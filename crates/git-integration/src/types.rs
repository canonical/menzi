use menzi_common::ids::ProjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepo {
    pub id: String,
    pub project_id: ProjectId,
    pub provider: GitProvider,
    pub external_id: String,
    pub name: String,
    pub default_branch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitProvider {
    Github,
    Gitlab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitPR {
    pub id: String,
    pub repo_id: String,
    pub external_id: String,
    pub number: i32,
    pub title: String,
    pub branch: String,
    pub head_sha: String,
    pub status: PRStatus,
    pub preview_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PRStatus {
    Open,
    Closed,
    Merged,
    Draft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRCheck {
    pub pr_id: String,
    pub revision_id: String,
    pub findings: Vec<PRFinding>,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRFinding {
    pub clause_id: String,
    pub severity: FindingSeverity,
    pub message: String,
    pub file: String,
    pub line: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_type: String,
    pub delivery_id: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInstallation {
    pub id: String,
    pub provider: GitProvider,
    pub account_login: String,
    pub repositories: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_provider_roundtrips() {
        assert_eq!(
            serde_json::to_string(&GitProvider::Github).unwrap(),
            "\"github\""
        );
        assert_eq!(
            serde_json::to_string(&GitProvider::Gitlab).unwrap(),
            "\"gitlab\""
        );
    }

    #[test]
    fn pr_status_roundtrips() {
        assert_eq!(serde_json::to_string(&PRStatus::Open).unwrap(), "\"open\"");
        assert_eq!(
            serde_json::to_string(&PRStatus::Closed).unwrap(),
            "\"closed\""
        );
        assert_eq!(
            serde_json::to_string(&PRStatus::Merged).unwrap(),
            "\"merged\""
        );
        assert_eq!(
            serde_json::to_string(&PRStatus::Draft).unwrap(),
            "\"draft\""
        );
    }

    #[test]
    fn check_status_roundtrips() {
        assert_eq!(
            serde_json::to_string(&CheckStatus::Pass).unwrap(),
            "\"pass\""
        );
        assert_eq!(
            serde_json::to_string(&CheckStatus::Fail).unwrap(),
            "\"fail\""
        );
        assert_eq!(
            serde_json::to_string(&CheckStatus::Warning).unwrap(),
            "\"warning\""
        );
    }

    #[test]
    fn finding_severity_roundtrips() {
        assert_eq!(
            serde_json::to_string(&FindingSeverity::Info).unwrap(),
            "\"info\""
        );
        assert_eq!(
            serde_json::to_string(&FindingSeverity::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(
            serde_json::to_string(&FindingSeverity::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn git_repo_serializes() {
        let repo = GitRepo {
            id: "repo-1".to_string(),
            project_id: ProjectId::new(),
            provider: GitProvider::Github,
            external_id: "12345".to_string(),
            name: "my-project".to_string(),
            default_branch: "main".to_string(),
        };
        let json = serde_json::to_string(&repo).unwrap();
        assert!(json.contains("\"provider\":\"github\""));
        assert!(json.contains("my-project"));
    }

    #[test]
    fn git_pr_serializes() {
        let pr = GitPR {
            id: "pr-1".to_string(),
            repo_id: "repo-1".to_string(),
            external_id: "67890".to_string(),
            number: 412,
            title: "Add feature".to_string(),
            branch: "feature/add-feature".to_string(),
            head_sha: "abc123".to_string(),
            status: PRStatus::Open,
            preview_id: Some("preview-1".to_string()),
        };
        let json = serde_json::to_string(&pr).unwrap();
        assert!(json.contains("\"number\":412"));
        assert!(json.contains("\"status\":\"open\""));
    }

    #[test]
    fn pr_check_serializes() {
        let check = PRCheck {
            pr_id: "pr-1".to_string(),
            revision_id: "rev-42".to_string(),
            findings: vec![PRFinding {
                clause_id: "DC-12".to_string(),
                severity: FindingSeverity::Error,
                message: "CLI accesses database directly".to_string(),
                file: "cmd/cli/list.go".to_string(),
                line: 42,
            }],
            status: CheckStatus::Fail,
        };
        let json = serde_json::to_string(&check).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("\"status\":\"fail\""));
    }

    #[test]
    fn webhook_event_serializes() {
        let event = WebhookEvent {
            event_type: "pull_request".to_string(),
            delivery_id: "del-1".to_string(),
            payload: serde_json::json!({"action": "opened"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("pull_request"));
    }

    #[test]
    fn app_installation_serializes() {
        let installation = AppInstallation {
            id: "inst-1".to_string(),
            provider: GitProvider::Github,
            account_login: "my-org".to_string(),
            repositories: vec!["repo-1".to_string(), "repo-2".to_string()],
        };
        let json = serde_json::to_string(&installation).unwrap();
        assert!(json.contains("my-org"));
        assert!(json.contains("repo-1"));
    }

    #[test]
    fn pr_finding_serializes() {
        let finding = PRFinding {
            clause_id: "DC-12".to_string(),
            severity: FindingSeverity::Warning,
            message: "Possible design violation".to_string(),
            file: "src/main.rs".to_string(),
            line: 10,
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(json.contains("DC-12"));
        assert!(json.contains("\"severity\":\"warning\""));
    }
}
