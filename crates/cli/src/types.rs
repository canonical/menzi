use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub server_url: String,
    pub token: String,
    pub default_format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Table,
    Json,
    Plain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchRequest {
    pub project_id: String,
    pub branch: Option<String>,
    pub task: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResponse {
    pub session_id: String,
    pub status: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardRequest {
    pub session_id: String,
    pub exposure_name: String,
    pub local_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardResponse {
    pub local_port: u16,
    pub remote_target: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCheckRequest {
    pub project_id: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCheckResponse {
    pub revision: String,
    pub checks: Vec<DesignCheckResult>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignCheckResult {
    pub clause_id: String,
    pub verdict: String,
    pub file: Option<String>,
    pub line: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewCreateRequest {
    pub project_id: String,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub mode: PreviewMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMode {
    Pinned,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewCreateResponse {
    pub preview_id: String,
    pub status: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_format_roundtrips() {
        assert_eq!(
            serde_json::to_string(&OutputFormat::Table).unwrap(),
            "\"table\""
        );
        assert_eq!(
            serde_json::to_string(&OutputFormat::Json).unwrap(),
            "\"json\""
        );
        assert_eq!(
            serde_json::to_string(&OutputFormat::Plain).unwrap(),
            "\"plain\""
        );
    }

    #[test]
    fn preview_mode_roundtrips() {
        assert_eq!(
            serde_json::to_string(&PreviewMode::Pinned).unwrap(),
            "\"pinned\""
        );
        assert_eq!(
            serde_json::to_string(&PreviewMode::Live).unwrap(),
            "\"live\""
        );
    }

    #[test]
    fn cli_config_serializes() {
        let config = CliConfig {
            server_url: "https://menzi.example.com".to_string(),
            token: "token-123".to_string(),
            default_format: OutputFormat::Table,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("https://menzi.example.com"));
        assert!(json.contains("\"default_format\":\"table\""));
    }

    #[test]
    fn launch_request_serializes() {
        let request = LaunchRequest {
            project_id: "proj-1".to_string(),
            branch: Some("main".to_string()),
            task: Some("Add feature".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("proj-1"));
        assert!(json.contains("Add feature"));
    }

    #[test]
    fn launch_response_deserializes() {
        let json = r#"{"session_id":"sess-1","status":"ready","url":"https://menzi.example.com/s/sess-1"}"#;
        let response: LaunchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.session_id, "sess-1");
        assert_eq!(response.status, "ready");
    }

    #[test]
    fn forward_request_serializes() {
        let request = ForwardRequest {
            session_id: "sess-1".to_string(),
            exposure_name: "daemon-api".to_string(),
            local_port: 7000,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("daemon-api"));
        assert!(json.contains("\"local_port\":7000"));
    }

    #[test]
    fn forward_response_deserializes() {
        let json = r#"{"local_port":7000,"remote_target":"daemon:7000","status":"active"}"#;
        let response: ForwardResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.local_port, 7000);
        assert_eq!(response.status, "active");
    }

    #[test]
    fn design_check_request_serializes() {
        let request = DesignCheckRequest {
            project_id: "proj-1".to_string(),
            revision: Some("latest".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("proj-1"));
        assert!(json.contains("latest"));
    }

    #[test]
    fn design_check_response_deserializes() {
        let json = r#"{"revision":"R42","checks":[{"clause_id":"DC-12","verdict":"pass","file":null,"line":null,"message":"OK"}],"passed":true}"#;
        let response: DesignCheckResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.revision, "R42");
        assert!(response.passed);
    }

    #[test]
    fn preview_create_request_serializes() {
        let request = PreviewCreateRequest {
            project_id: "proj-1".to_string(),
            commit_sha: Some("abc123".to_string()),
            branch: Some("feature/x".to_string()),
            mode: PreviewMode::Pinned,
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("\"mode\":\"pinned\""));
    }

    #[test]
    fn preview_create_response_deserializes() {
        let json = r#"{"preview_id":"prev-1","status":"building","url":"https://menzi.example.com/previews/prev-1"}"#;
        let response: PreviewCreateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.preview_id, "prev-1");
        assert_eq!(response.status, "building");
    }

    #[test]
    fn auth_token_serializes() {
        let token = AuthToken {
            access_token: "access-123".to_string(),
            refresh_token: Some("refresh-456".to_string()),
            expires_at: "2026-12-31T23:59:59Z".to_string(),
        };
        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("access-123"));
        assert!(json.contains("refresh-456"));
    }

    #[test]
    fn design_check_result_serializes() {
        let result = DesignCheckResult {
            clause_id: "DC-12".to_string(),
            verdict: "violation".to_string(),
            file: Some("cmd/cli/list.go".to_string()),
            line: Some(42),
            message: "Direct database access".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("Direct database access"));
    }
}
