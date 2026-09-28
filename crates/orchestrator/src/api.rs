use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use menzi_common::ids::SessionId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::drivers::EnvDriver;
use crate::env_controller::EnvironmentController;
use crate::gate::{AlwaysFreeGate, WorkGate};
use crate::types::EnvironmentSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvLaunchRequest {
    pub session_id: SessionId,
    pub environment_name: String,
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvRelaunchRequest {
    pub session_id: SessionId,
    pub environment_name: String,
    pub reset_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvStatusRequest {
    pub session_id: SessionId,
    pub environment_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvLogsRequest {
    pub session_id: SessionId,
    pub environment_name: String,
    pub component: String,
    pub tail: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvExecRequest {
    pub session_id: SessionId,
    pub environment_name: String,
    pub component: String,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct OrchestratorState {
    pub driver: Arc<dyn EnvDriver>,
    pub controller: Arc<EnvironmentController>,
    pub specs: Arc<Mutex<HashMap<String, EnvironmentSpec>>>,
    pub gate: Arc<dyn WorkGate>,
}

impl OrchestratorState {
    pub fn new(driver: Arc<dyn EnvDriver>) -> Self {
        Self::with_gate(driver, Arc::new(AlwaysFreeGate))
    }

    pub fn with_gate(driver: Arc<dyn EnvDriver>, gate: Arc<dyn WorkGate>) -> Self {
        Self {
            driver,
            controller: Arc::new(EnvironmentController::new()),
            specs: Arc::new(Mutex::new(HashMap::new())),
            gate,
        }
    }

    pub fn register_spec(&self, name: &str, spec: EnvironmentSpec) {
        self.specs
            .lock()
            .expect("specs lock")
            .insert(name.to_string(), spec);
    }
}

pub fn create_router(state: OrchestratorState) -> Router {
    Router::new()
        .route("/api/env/specs", post(register_spec_handler))
        .route("/api/env/launch", post(launch_handler))
        .route("/api/env/relaunch", post(relaunch_handler))
        .route("/api/env/status", get(status_handler))
        .route("/api/env/exec", post(exec_handler))
        .route("/api/env/logs", post(logs_handler))
        .with_state(state)
}

fn lookup_spec(
    state: &OrchestratorState,
    environment_name: &str,
    variant: Option<&str>,
) -> Option<EnvironmentSpec> {
    let specs = state.specs.lock().expect("specs lock");
    match variant {
        Some(variant) => specs
            .get(&format!("{environment_name}-{variant}"))
            .cloned()
            .or_else(|| specs.get(environment_name).cloned()),
        None => specs.get(environment_name).cloned(),
    }
}

fn spec_not_found(environment_name: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(EnvResponse {
            success: false,
            message: format!("environment spec '{environment_name}' not found"),
            data: None,
        }),
    )
        .into_response()
}

fn internal_error(error: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(EnvResponse {
            success: false,
            message: error.to_string(),
            data: None,
        }),
    )
        .into_response()
}

async fn register_spec_handler(
    State(state): State<OrchestratorState>,
    Json(spec): Json<EnvironmentSpec>,
) -> Response {
    let name = spec.name.clone();
    state.register_spec(&name, spec);
    (
        StatusCode::OK,
        Json(EnvResponse {
            success: true,
            message: "environment spec registered".to_string(),
            data: None,
        }),
    )
        .into_response()
}

async fn launch_handler(
    State(state): State<OrchestratorState>,
    Json(request): Json<EnvLaunchRequest>,
) -> Response {
    let spec = match lookup_spec(
        &state,
        &request.environment_name,
        request.variant.as_deref(),
    ) {
        Some(spec) => spec,
        None => return spec_not_found(&request.environment_name),
    };
    let work = format!("env:{}", request.session_id);
    let acquired = match state.gate.try_acquire(&work).await {
        Ok(acquired) => acquired,
        Err(error) => return internal_error(error),
    };
    if !acquired {
        return (
            StatusCode::CONFLICT,
            Json(EnvResponse {
                success: false,
                message: "environment operation already in progress".to_string(),
                data: None,
            }),
        )
            .into_response();
    }
    let result = state.controller.launch(&spec, state.driver.as_ref()).await;
    let _ = state.gate.release(&work).await;
    match result {
        Ok(result) => (
            StatusCode::OK,
            Json(EnvResponse {
                success: result.success,
                message: "launch completed".to_string(),
                data: serde_json::to_value(&result).ok(),
            }),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn relaunch_handler(
    State(state): State<OrchestratorState>,
    Json(request): Json<EnvRelaunchRequest>,
) -> Response {
    let spec = match lookup_spec(&state, &request.environment_name, None) {
        Some(spec) => spec,
        None => return spec_not_found(&request.environment_name),
    };
    let changes = crate::types::ChangeSet {
        changed_paths: vec![],
        changed_inputs: vec![spec.name.clone()],
    };
    match state
        .controller
        .relaunch(&spec, &changes, state.driver.as_ref())
        .await
    {
        Ok(diff) => (
            StatusCode::OK,
            Json(EnvResponse {
                success: true,
                message: "relaunch completed".to_string(),
                data: Some(serde_json::to_value(&diff).unwrap_or_default()),
            }),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn status_handler(
    State(state): State<OrchestratorState>,
    Json(request): Json<EnvStatusRequest>,
) -> Response {
    let spec = match lookup_spec(&state, &request.environment_name, None) {
        Some(spec) => spec,
        None => return spec_not_found(&request.environment_name),
    };
    match state.controller.status(&spec, state.driver.as_ref()).await {
        Ok(status) => (
            StatusCode::OK,
            Json(EnvResponse {
                success: true,
                message: "status retrieved".to_string(),
                data: Some(serde_json::to_value(&status).unwrap_or_default()),
            }),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn exec_handler(
    State(state): State<OrchestratorState>,
    Json(request): Json<EnvExecRequest>,
) -> Response {
    let spec = match lookup_spec(&state, &request.environment_name, None) {
        Some(spec) => spec,
        None => return spec_not_found(&request.environment_name),
    };
    match state
        .controller
        .exec(
            &spec,
            &request.component,
            &request.command,
            state.driver.as_ref(),
        )
        .await
    {
        Ok(result) => (
            StatusCode::OK,
            Json(EnvResponse {
                success: true,
                message: "exec completed".to_string(),
                data: Some(serde_json::to_value(&result).unwrap_or_default()),
            }),
        )
            .into_response(),
        Err(error) => internal_error(error),
    }
}

async fn logs_handler(
    State(state): State<OrchestratorState>,
    Json(request): Json<EnvLogsRequest>,
) -> Response {
    let spec = match lookup_spec(&state, &request.environment_name, None) {
        Some(spec) => spec,
        None => return spec_not_found(&request.environment_name),
    };
    let component = request.component.trim();
    let data = Some(serde_json::json!({
        "component": component,
        "lines": [],
        "truncated": false,
    }));
    (
        StatusCode::OK,
        Json(EnvResponse {
            success: true,
            message: format!(
                "logs for component '{component}' of environment '{}'",
                spec.name
            ),
            data,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::EnvDriver;
    use crate::types::{ComponentKind, ComponentSpec, ExposeSpec, ExposeType, Placement};
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use menzi_lxd::ExecResult;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[derive(Default)]
    struct MockDriver {
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl EnvDriver for MockDriver {
        async fn create_instance(
            &self,
            name: &str,
            _image: &str,
            _profiles: &[String],
        ) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("create:{name}"));
            Ok(())
        }

        async fn start_instance(&self, name: &str) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            Ok(())
        }

        async fn stop_instance(&self, name: &str, force: bool) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{name}:{force}"));
            Ok(())
        }

        async fn delete_instance(&self, name: &str) -> menzi_common::Result<()> {
            self.calls.lock().unwrap().push(format!("delete:{name}"));
            Ok(())
        }

        async fn exec(&self, name: &str, command: &[String]) -> menzi_common::Result<ExecResult> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("exec:{name}:{}", command.join(" ")));
            Ok(ExecResult {
                exit_code: 0,
                stdout: "ok".to_string(),
                stderr: String::new(),
            })
        }

        async fn instance_status(&self, _name: &str) -> menzi_common::Result<String> {
            Ok("running".to_string())
        }

        async fn snapshot_instance(&self, name: &str, snapshot: &str) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("snapshot:{name}:{snapshot}"));
            Ok(())
        }

        async fn copy_instance(&self, source: &str, dest: &str) -> menzi_common::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("copy:{source}:{dest}"));
            Ok(())
        }
    }

    fn dev_spec() -> EnvironmentSpec {
        EnvironmentSpec {
            name: "dev".to_string(),
            components: vec![
                ComponentSpec {
                    name: "db".to_string(),
                    kind: ComponentKind::Service,
                    image: Some("postgres-16".to_string()),
                    run: None,
                    ready: None,
                    expose: vec![ExposeSpec {
                        port: 5432,
                        as_type: ExposeType::Tcp,
                        name: "pg".to_string(),
                    }],
                    after: vec![],
                    placement: Placement::OwnInstance,
                },
                ComponentSpec {
                    name: "daemon".to_string(),
                    kind: ComponentKind::Service,
                    image: None,
                    run: Some("daemon serve".to_string()),
                    ready: None,
                    expose: vec![],
                    after: vec!["db".to_string()],
                    placement: Placement::OwnInstance,
                },
            ],
        }
    }

    async fn body_string(response: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[tokio::test]
    async fn register_spec_then_launch_wires_driver() {
        let state = OrchestratorState::new(Arc::new(MockDriver::default()));
        let app = create_router(state.clone());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/specs")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&dev_spec()).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            variant: None,
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/launch")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("\"success\":true"));
    }

    #[tokio::test]
    async fn launch_without_spec_returns_not_found() {
        let state = OrchestratorState::new(Arc::new(MockDriver::default()));
        let app = create_router(state);
        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "missing".to_string(),
            variant: None,
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/launch")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    struct BusyGate;

    #[async_trait]
    impl WorkGate for BusyGate {
        async fn try_acquire(&self, _work: &str) -> menzi_common::Result<bool> {
            Ok(false)
        }

        async fn release(&self, _work: &str) -> menzi_common::Result<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingGate {
        events: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WorkGate for RecordingGate {
        async fn try_acquire(&self, work: &str) -> menzi_common::Result<bool> {
            self.events.lock().unwrap().push(format!("acquire:{work}"));
            Ok(true)
        }

        async fn release(&self, work: &str) -> menzi_common::Result<()> {
            self.events.lock().unwrap().push(format!("release:{work}"));
            Ok(())
        }
    }

    #[tokio::test]
    async fn launch_conflicts_when_gate_busy() {
        let state =
            OrchestratorState::with_gate(Arc::new(MockDriver::default()), Arc::new(BusyGate));
        state.register_spec("dev", dev_spec());
        let app = create_router(state);
        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            variant: None,
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/launch")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn launch_releases_gate_after_success() {
        let gate = Arc::new(RecordingGate::default());
        let state = OrchestratorState::with_gate(Arc::new(MockDriver::default()), gate.clone());
        state.register_spec("dev", dev_spec());
        let app = create_router(state);
        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            variant: None,
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/launch")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let events = gate.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].starts_with("acquire:env:"));
        assert_eq!(events[0], events[1].replace("release", "acquire"));
    }

    #[tokio::test]
    async fn status_returns_environment_state() {
        let state = OrchestratorState::new(Arc::new(MockDriver::default()));
        state.register_spec("dev", dev_spec());
        let app = create_router(state);
        let request = EnvStatusRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/env/status")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("\"status\":\"ready\""));
    }

    #[tokio::test]
    async fn exec_returns_command_result() {
        let state = OrchestratorState::new(Arc::new(MockDriver::default()));
        state.register_spec("dev", dev_spec());
        let app = create_router(state);
        let request = EnvExecRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            component: "daemon".to_string(),
            command: vec!["ls".to_string(), "-la".to_string()],
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/exec")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("\"exit_code\":0"));
    }

    #[tokio::test]
    async fn relaunch_restarts_environment() {
        let state = OrchestratorState::new(Arc::new(MockDriver::default()));
        state.register_spec("dev", dev_spec());
        let app = create_router(state);
        let request = EnvRelaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            reset_data: false,
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/env/relaunch")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn dto_shapes_match_supervisor_wire_contract() {
        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            variant: Some("e2e".to_string()),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("session_id").is_some());
        assert!(json.get("environment_name").is_some());
        assert!(json.get("variant").is_some());
    }
}
