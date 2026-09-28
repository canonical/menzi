use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::manager::PreviewManager;
use crate::types::{Preview, PreviewSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePreviewRequest {
    pub project_id: ProjectId,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub mode: Option<String>,
}

#[derive(Clone)]
pub struct PreviewApiState {
    pub manager: Arc<PreviewManager>,
    pub source_instance: String,
}

impl PreviewApiState {
    pub fn new(manager: Arc<PreviewManager>, source_instance: impl Into<String>) -> Self {
        Self {
            manager,
            source_instance: source_instance.into(),
        }
    }
}

pub fn create_router(state: PreviewApiState) -> Router {
    Router::new()
        .route("/api/v1/projects/{project_id}/previews", get(list_previews))
        .route("/api/v1/previews", post(create_preview))
        .route(
            "/api/v1/previews/{id}",
            get(get_preview).delete(teardown_preview),
        )
        .route("/api/v1/previews/{id}/reset", post(reset_preview))
        .route("/api/v1/previews/{id}/restart", post(restart_preview))
        .with_state(state)
}

fn not_found(id: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": format!("preview {id} not found")})),
    )
        .into_response()
}

async fn list_previews(
    State(state): State<PreviewApiState>,
    Path(project_id): Path<ProjectId>,
) -> Json<Vec<Preview>> {
    Json(state.manager.list(project_id))
}

async fn create_preview(
    State(state): State<PreviewApiState>,
    Json(request): Json<CreatePreviewRequest>,
) -> Response {
    let spec = PreviewSpec {
        project_id: request.project_id,
        source_instance: state.source_instance.clone(),
        commit_sha: request.commit_sha,
        branch: request.branch,
        mode: request.mode.unwrap_or_else(|| "pinned".to_string()),
    };
    match state.manager.create(spec).await {
        Ok(preview) => (StatusCode::CREATED, Json(preview)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn get_preview(State(state): State<PreviewApiState>, Path(id): Path<String>) -> Response {
    match state.manager.get(&id) {
        Some(preview) => Json(preview).into_response(),
        None => not_found(&id),
    }
}

async fn reset_preview(State(state): State<PreviewApiState>, Path(id): Path<String>) -> Response {
    match state.manager.reset(&id).await {
        Ok(preview) => Json(preview).into_response(),
        Err(_) => not_found(&id),
    }
}

async fn restart_preview(State(state): State<PreviewApiState>, Path(id): Path<String>) -> Response {
    match state.manager.restart(&id).await {
        Ok(preview) => Json(preview).into_response(),
        Err(_) => not_found(&id),
    }
}

async fn teardown_preview(
    State(state): State<PreviewApiState>,
    Path(id): Path<String>,
) -> Response {
    match state.manager.teardown(&id).await {
        Ok(()) => Json(json!({"success": true})).into_response(),
        Err(_) => not_found(&id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::testbed::RecordingPreviewDriver;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn state() -> (PreviewApiState, Arc<RecordingPreviewDriver>) {
        let driver = Arc::new(RecordingPreviewDriver::default());
        let manager = Arc::new(PreviewManager::new(driver.clone()));
        (PreviewApiState::new(manager, "mz-workspace"), driver)
    }

    async fn body_string(response: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[tokio::test]
    async fn create_preview_returns_created() {
        let (state, driver) = state();
        let app = create_router(state);
        let project_id = ProjectId::new();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "project_id": project_id.to_string(),
                            "commit_sha": "abc123",
                            "mode": "pinned"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_string(response).await;
        assert!(body.contains("\"status\":\"ready\""));
        assert!(body.contains("preview.dev.local"));
        let calls = driver.calls.lock().unwrap();
        assert!(calls[0].starts_with("copy:mz-workspace:prv-"));
        assert!(calls[1].starts_with("start:prv-"));
    }

    #[tokio::test]
    async fn list_previews_matches_project() {
        let (state, _driver) = state();
        let app = create_router(state);
        let project_id = ProjectId::new();
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "project_id": project_id.to_string(),
                            "mode": "pinned"
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/projects/{project_id}/previews"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        let previews: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(previews.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_preview_returns_record() {
        let (state, _driver) = state();
        let app = create_router(state);
        let project_id = ProjectId::new();
        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"project_id": project_id.to_string()})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created_body = body_string(created).await;
        let created_json: serde_json::Value = serde_json::from_str(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap().to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/previews/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_missing_preview_returns_not_found() {
        let (state, _driver) = state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/previews/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn reset_preview_cycles_instance() {
        let (state, driver) = state();
        let app = create_router(state);
        let project_id = ProjectId::new();
        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"project_id": project_id.to_string()})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created_body = body_string(created).await;
        let created_json: serde_json::Value = serde_json::from_str(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap().to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/previews/{id}/reset"))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let calls = driver.calls.lock().unwrap();
        assert!(calls
            .iter()
            .any(|call| call.starts_with("stop:prv-") && call.ends_with(":true")));
    }

    #[tokio::test]
    async fn teardown_preview_removes_record() {
        let (state, driver) = state();
        let app = create_router(state);
        let project_id = ProjectId::new();
        let created = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"project_id": project_id.to_string()})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let created_body = body_string(created).await;
        let created_json: serde_json::Value = serde_json::from_str(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap().to_string();
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/previews/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let calls = driver.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call.starts_with("delete:prv-")));
    }

    #[tokio::test]
    async fn restart_missing_preview_returns_not_found() {
        let (state, _driver) = state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews/nope/restart")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
