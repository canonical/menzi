use menzi_common::ids::{OrgId, ProjectId, SessionId, UserId};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRequest {
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub user_id: UserId,
    pub org_id: OrgId,
    pub template_id: Option<String>,
    pub trust_level: TrustLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Container,
    Vm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementDecision {
    pub node: String,
    pub project: String,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub active_workspaces: i32,
    pub max_workspaces: i32,
    pub previews_count: i32,
    pub max_previews: i32,
}

impl QuotaStatus {
    pub fn can_start_workspace(&self) -> bool {
        self.active_workspaces < self.max_workspaces
    }

    pub fn can_create_preview(&self) -> bool {
        self.previews_count < self.max_previews
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmPool {
    pub template_id: String,
    pub available: i32,
    pub target: i32,
}

impl WarmPool {
    pub fn needs_replenishment(&self) -> bool {
        self.available < self.target
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    pub name: String,
    pub components: Vec<ComponentSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub name: String,
    pub kind: ComponentKind,
    pub image: Option<String>,
    pub run: Option<String>,
    pub ready: Option<ReadyCheck>,
    pub expose: Vec<ExposeSpec>,
    pub after: Vec<String>,
    pub placement: Placement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Service,
    Job,
    Console,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadyCheck {
    pub tcp: Option<u16>,
    pub http: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposeSpec {
    pub port: u16,
    pub as_type: ExposeType,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposeType {
    Http,
    Tcp,
    Terminal,
    Logs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    Same,
    OwnInstance,
    Vm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelaunchPlan {
    pub components_to_restart: Vec<String>,
    pub components_to_build: Vec<String>,
    pub components_to_skip: Vec<String>,
}

impl RelaunchPlan {
    pub fn is_empty(&self) -> bool {
        self.components_to_restart.is_empty() && self.components_to_build.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    pub changed_paths: Vec<String>,
    pub changed_inputs: Vec<String>,
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
    }

    #[test]
    fn instance_deserializes() {
        let json = r#"{"name":"test","status":"Running","status_code":103}"#;
        let instance: Instance = serde_json::from_str(json).unwrap();
        assert_eq!(instance.name, "test");
        assert_eq!(instance.status, "Running");
    }

    #[test]
    fn quota_status_allows_workspace_under_limit() {
        let quota = QuotaStatus {
            active_workspaces: 2,
            max_workspaces: 5,
            previews_count: 0,
            max_previews: 10,
        };
        assert!(quota.can_start_workspace());
    }

    #[test]
    fn quota_status_denies_workspace_at_limit() {
        let quota = QuotaStatus {
            active_workspaces: 5,
            max_workspaces: 5,
            previews_count: 0,
            max_previews: 10,
        };
        assert!(!quota.can_start_workspace());
    }

    #[test]
    fn quota_status_allows_preview_under_limit() {
        let quota = QuotaStatus {
            active_workspaces: 0,
            max_workspaces: 5,
            previews_count: 3,
            max_previews: 10,
        };
        assert!(quota.can_create_preview());
    }

    #[test]
    fn quota_status_denies_preview_at_limit() {
        let quota = QuotaStatus {
            active_workspaces: 0,
            max_workspaces: 5,
            previews_count: 10,
            max_previews: 10,
        };
        assert!(!quota.can_create_preview());
    }

    #[test]
    fn warm_pool_needs_replenishment_when_low() {
        let pool = WarmPool {
            template_id: "tpl-1".to_string(),
            available: 1,
            target: 5,
        };
        assert!(pool.needs_replenishment());
    }

    #[test]
    fn warm_pool_does_not_need_replenishment_when_full() {
        let pool = WarmPool {
            template_id: "tpl-1".to_string(),
            available: 5,
            target: 5,
        };
        assert!(!pool.needs_replenishment());
    }

    #[test]
    fn relaunch_plan_is_empty_when_no_changes() {
        let plan = RelaunchPlan {
            components_to_restart: vec![],
            components_to_build: vec![],
            components_to_skip: vec!["comp-1".to_string()],
        };
        assert!(plan.is_empty());
    }

    #[test]
    fn relaunch_plan_is_not_empty_when_restart_needed() {
        let plan = RelaunchPlan {
            components_to_restart: vec!["comp-1".to_string()],
            components_to_build: vec![],
            components_to_skip: vec![],
        };
        assert!(!plan.is_empty());
    }

    #[test]
    fn environment_spec_serializes() {
        let spec = EnvironmentSpec {
            name: "dev".to_string(),
            components: vec![ComponentSpec {
                name: "db".to_string(),
                kind: ComponentKind::Service,
                image: Some("postgres-16".to_string()),
                run: None,
                ready: Some(ReadyCheck {
                    tcp: Some(5432),
                    http: None,
                    timeout_secs: 30,
                }),
                expose: vec![],
                after: vec![],
                placement: Placement::OwnInstance,
            }],
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"name\":\"dev\""));
        assert!(json.contains("\"kind\":\"service\""));
    }

    #[test]
    fn component_kind_roundtrips() {
        assert_eq!(
            serde_json::to_string(&ComponentKind::Service).unwrap(),
            "\"service\""
        );
        assert_eq!(
            serde_json::to_string(&ComponentKind::Job).unwrap(),
            "\"job\""
        );
        assert_eq!(
            serde_json::to_string(&ComponentKind::Console).unwrap(),
            "\"console\""
        );
    }

    #[test]
    fn expose_type_roundtrips() {
        assert_eq!(
            serde_json::to_string(&ExposeType::Http).unwrap(),
            "\"http\""
        );
        assert_eq!(serde_json::to_string(&ExposeType::Tcp).unwrap(), "\"tcp\"");
        assert_eq!(
            serde_json::to_string(&ExposeType::Terminal).unwrap(),
            "\"terminal\""
        );
        assert_eq!(
            serde_json::to_string(&ExposeType::Logs).unwrap(),
            "\"logs\""
        );
    }

    #[test]
    fn placement_roundtrips() {
        assert_eq!(serde_json::to_string(&Placement::Same).unwrap(), "\"same\"");
        assert_eq!(
            serde_json::to_string(&Placement::OwnInstance).unwrap(),
            "\"own_instance\""
        );
        assert_eq!(serde_json::to_string(&Placement::Vm).unwrap(), "\"vm\"");
    }

    #[test]
    fn trust_level_roundtrips() {
        assert_eq!(
            serde_json::to_string(&TrustLevel::Container).unwrap(),
            "\"container\""
        );
        assert_eq!(serde_json::to_string(&TrustLevel::Vm).unwrap(), "\"vm\"");
    }

    #[test]
    fn change_set_serializes() {
        let changes = ChangeSet {
            changed_paths: vec!["src/main.rs".to_string()],
            changed_inputs: vec!["cmd/daemon".to_string()],
        };
        let json = serde_json::to_string(&changes).unwrap();
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("cmd/daemon"));
    }

    #[test]
    fn placement_decision_serializes() {
        let decision = PlacementDecision {
            node: "node-1".to_string(),
            project: "org-acme-ws".to_string(),
            network: "sess-123".to_string(),
        };
        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("node-1"));
        assert!(json.contains("org-acme-ws"));
    }
}
