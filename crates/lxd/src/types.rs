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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceState {
    pub status: String,
    pub addresses: Vec<String>,
}

impl InstanceState {
    pub fn is_running(&self) -> bool {
        self.status.eq_ignore_ascii_case("running")
    }

    pub fn global_ipv4(&self) -> Option<&str> {
        self.addresses
            .iter()
            .filter_map(|entry| entry.strip_prefix("global:"))
            .find(|address| !address.contains(':'))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub status: String,
    pub status_code: i32,
    pub error: String,
    pub location: Option<String>,
}

impl Operation {
    pub fn is_running(&self) -> bool {
        self.status_code == 103
    }

    pub fn succeeded(&self) -> bool {
        self.status_code == 200
    }

    pub fn failed(&self) -> bool {
        self.status_code >= 400
    }
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
    fn instance_state_reads_the_global_ipv4() {
        let state = InstanceState {
            status: "Running".to_string(),
            addresses: vec![
                "global:fe80::1".to_string(),
                "global:10.10.10.251".to_string(),
                "local:127.0.0.1".to_string(),
            ],
        };
        assert!(state.is_running());
        assert_eq!(state.global_ipv4(), Some("10.10.10.251"));
    }

    #[test]
    fn instance_state_ignores_local_and_ipv6_addresses() {
        let state = InstanceState {
            status: "Running".to_string(),
            addresses: vec!["local:127.0.0.1".to_string(), "global:fd42::5".to_string()],
        };
        assert_eq!(state.global_ipv4(), None);
    }

    #[test]
    fn instance_state_reports_a_stopped_container() {
        let state = InstanceState {
            status: "Stopped".to_string(),
            addresses: vec![],
        };
        assert!(!state.is_running());
        assert_eq!(state.global_ipv4(), None);
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
