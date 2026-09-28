use crate::types::*;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvComponentStatus {
    pub name: String,
    pub status: String,
    pub health: String,
    pub uptime_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvStatus {
    pub environment_name: String,
    pub status: String,
    pub components: Vec<EnvComponentStatus>,
    pub exposures: Vec<ExposureInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureInfo {
    pub name: String,
    pub url: String,
    pub as_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStream {
    pub component: String,
    pub lines: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct EnvToolHandler {
    pub control_plane_url: String,
    pub session_token: String,
}

impl EnvToolHandler {
    pub fn new(control_plane_url: impl Into<String>, session_token: impl Into<String>) -> Self {
        Self {
            control_plane_url: control_plane_url.into(),
            session_token: session_token.into(),
        }
    }

    pub fn validate_session(&self, session_id: &SessionId) -> bool {
        !session_id.to_string().is_empty()
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.control_plane_url.trim_end_matches('/'), path)
    }

    async fn post<T: Serialize, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> menzi_common::Result<R> {
        let response = reqwest::Client::new()
            .post(self.endpoint(path))
            .bearer_auth(&self.session_token)
            .json(body)
            .send()
            .await
            .map_err(|error| {
                menzi_common::MenziError::Gateway(format!("control plane request failed: {error}"))
            })?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|error| {
            menzi_common::MenziError::Gateway(format!("control plane response failed: {error}"))
        })?;
        if !status.is_success() {
            return Err(menzi_common::MenziError::Gateway(format!(
                "control plane returned {status}: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        serde_json::from_slice(&bytes).map_err(|error| {
            menzi_common::MenziError::Gateway(format!("invalid control plane response: {error}"))
        })
    }

    pub async fn launch(&self, request: &EnvLaunchRequest) -> menzi_common::Result<EnvResponse> {
        self.post("/api/env/launch", request).await
    }

    pub async fn relaunch(
        &self,
        request: &EnvRelaunchRequest,
    ) -> menzi_common::Result<EnvResponse> {
        self.post("/api/env/relaunch", request).await
    }

    pub async fn status(&self, request: &EnvStatusRequest) -> menzi_common::Result<EnvResponse> {
        self.post("/api/env/status", request).await
    }

    pub async fn logs(&self, request: &EnvLogsRequest) -> menzi_common::Result<EnvResponse> {
        self.post("/api/env/logs", request).await
    }

    pub async fn exec(&self, request: &EnvExecRequest) -> menzi_common::Result<EnvResponse> {
        self.post("/api/env/exec", request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_launch_request_serializes() {
        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            variant: Some("e2e".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("dev"));
    }

    #[test]
    fn env_response_serializes() {
        let response = EnvResponse {
            success: true,
            message: "OK".to_string(),
            data: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn env_status_serializes() {
        let status = EnvStatus {
            environment_name: "dev".to_string(),
            status: "ready".to_string(),
            components: vec![EnvComponentStatus {
                name: "daemon".to_string(),
                status: "running".to_string(),
                health: "healthy".to_string(),
                uptime_secs: 3600,
            }],
            exposures: vec![],
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("daemon"));
    }

    #[test]
    fn env_tool_handler_validates_session() {
        let handler = EnvToolHandler::new("https://control.example.com", "token-123");
        assert!(handler.validate_session(&SessionId::new()));
    }

    #[test]
    fn log_stream_serializes() {
        let stream = LogStream {
            component: "daemon".to_string(),
            lines: vec!["line1".to_string(), "line2".to_string()],
            truncated: false,
        };
        let json = serde_json::to_string(&stream).unwrap();
        assert!(json.contains("daemon"));
    }

    #[test]
    fn exec_result_serializes() {
        let result = ExecResult {
            exit_code: 0,
            stdout: "OK".to_string(),
            stderr: "".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"exit_code\":0"));
    }

    #[test]
    fn endpoint_joins_control_plane_url() {
        let handler = EnvToolHandler::new("https://control.example.com", "token-123");
        assert_eq!(
            handler.endpoint("/api/env/launch"),
            "https://control.example.com/api/env/launch"
        );
    }

    #[test]
    fn endpoint_trims_trailing_slash() {
        let handler = EnvToolHandler::new("https://control.example.com/", "token-123");
        assert_eq!(
            handler.endpoint("/api/env/status"),
            "https://control.example.com/api/env/status"
        );
    }

    #[tokio::test]
    async fn launch_posts_to_control_plane_and_parses_response() {
        let app = axum::Router::new().route(
            "/api/env/launch",
            axum::routing::post(
                |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    assert_eq!(body["environment_name"], "dev");
                    axum::Json(EnvResponse {
                        success: true,
                        message: "launch completed".to_string(),
                        data: None,
                    })
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let handler = EnvToolHandler::new(format!("http://{address}"), "token-123");
        let request = EnvLaunchRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            variant: None,
        };
        let response = handler.launch(&request).await.unwrap();
        assert!(response.success);
        assert_eq!(response.message, "launch completed");
    }

    #[tokio::test]
    async fn status_returns_env_response() {
        let app = axum::Router::new().route(
            "/api/env/status",
            axum::routing::post(
                |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    assert_eq!(body["environment_name"], "dev");
                    axum::Json(EnvResponse {
                        success: true,
                        message: "status retrieved".to_string(),
                        data: Some(serde_json::json!({"status": "ready"})),
                    })
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let handler = EnvToolHandler::new(format!("http://{address}"), "token-123");
        let request = EnvStatusRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
        };
        let response = handler.status(&request).await.unwrap();
        assert!(response.success);
        assert_eq!(response.data.as_ref().unwrap()["status"], "ready");
    }

    #[tokio::test]
    async fn exec_returns_command_output() {
        let app = axum::Router::new().route(
            "/api/env/exec",
            axum::routing::post(
                |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    assert_eq!(body["command"][0], "ls");
                    axum::Json(EnvResponse {
                        success: true,
                        message: "exec completed".to_string(),
                        data: Some(serde_json::json!({"exit_code": 0, "stdout": "ok"})),
                    })
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let handler = EnvToolHandler::new(format!("http://{address}"), "token-123");
        let request = EnvExecRequest {
            session_id: SessionId::new(),
            environment_name: "dev".to_string(),
            component: "daemon".to_string(),
            command: vec!["ls".to_string(), "-la".to_string()],
        };
        let response = handler.exec(&request).await.unwrap();
        assert!(response.success);
        assert_eq!(response.data.as_ref().unwrap()["exit_code"], 0);
    }
}
