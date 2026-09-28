use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSpec {
    pub name: String,
    pub image: String,
    pub profiles: Vec<String>,
    pub config: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub name: String,
    pub status: String,
    pub status_code: i32,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub created_at: String,
    pub stateful: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_spec_serializes() {
        let mut config = std::collections::HashMap::new();
        config.insert("limits.cpu".to_string(), "2".to_string());
        let spec = InstanceSpec {
            name: "test".to_string(),
            image: "ubuntu/24.04".to_string(),
            profiles: vec!["default".to_string()],
            config,
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"image\":\"ubuntu/24.04\""));
    }

    #[test]
    fn instance_deserializes() {
        let json = r#"{"name":"test","status":"Running","status_code":103,"profiles":["default"]}"#;
        let instance: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(instance.name, "test");
        assert_eq!(instance.status, "Running");
    }

    #[test]
    fn exec_result_serializes() {
        let result = ExecResult {
            exit_code: 0,
            stdout: "hello".to_string(),
            stderr: "".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"exit_code\":0"));
        assert!(json.contains("\"stdout\":\"hello\""));
    }

    #[test]
    fn snapshot_deserializes() {
        let json = r#"{"name":"snap1","created_at":"2026-01-01T00:00:00Z","stateful":false}"#;
        let snapshot: Snapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.name, "snap1");
        assert!(!snapshot.stateful);
    }
}
