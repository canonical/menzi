use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestEvent {
    pub action: String,
    pub number: i32,
    pub pull_request: PullRequestData,
    pub repository: RepositoryData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestData {
    pub id: i64,
    pub number: i32,
    pub title: String,
    pub head: BranchData,
    pub base: BranchData,
    pub state: String,
    pub draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchData {
    pub sha: String,
    pub r#ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryData {
    pub id: i64,
    pub full_name: String,
    pub default_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushEvent {
    pub r#ref: String,
    pub before: String,
    pub after: String,
    pub repository: RepositoryData,
    pub pusher: PusherData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PusherData {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRunEvent {
    pub action: String,
    pub check_run: CheckRunData,
    pub repository: RepositoryData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRunData {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub output: Option<CheckRunOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRunOutput {
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookHandler {
    pub app_id: String,
    pub installation_id: String,
}

impl WebhookHandler {
    pub fn new(app_id: impl Into<String>, installation_id: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            installation_id: installation_id.into(),
        }
    }

    pub fn handle_pull_request(&self, event: &PullRequestEvent) -> Vec<String> {
        let mut actions = vec![];
        match event.action.as_str() {
            "opened" | "synchronize" => {
                actions.push("create_preview".to_string());
                actions.push("run_design_review".to_string());
            }
            "closed" => {
                actions.push("teardown_preview".to_string());
            }
            _ => {}
        }
        actions
    }

    pub fn handle_push(&self, event: &PushEvent) -> Vec<String> {
        let mut actions = vec![];
        if event.r#ref == format!("refs/heads/{}", event.repository.default_branch) {
            actions.push("trigger_prebuild".to_string());
        }
        actions
    }

    pub fn handle_check_run(&self, event: &CheckRunEvent) -> Vec<String> {
        let mut actions = vec![];
        if event.action == "rerequested" {
            actions.push("rerun_design_review".to_string());
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_handler_creates() {
        let handler = WebhookHandler::new("app-123", "inst-456");
        assert_eq!(handler.app_id, "app-123");
        assert_eq!(handler.installation_id, "inst-456");
    }

    #[test]
    fn webhook_handler_handles_pr_opened() {
        let handler = WebhookHandler::new("app-123", "inst-456");
        let event = PullRequestEvent {
            action: "opened".to_string(),
            number: 412,
            pull_request: PullRequestData {
                id: 123,
                number: 412,
                title: "Feature".to_string(),
                head: BranchData {
                    sha: "abc123".to_string(),
                    r#ref: "feature/x".to_string(),
                },
                base: BranchData {
                    sha: "def456".to_string(),
                    r#ref: "main".to_string(),
                },
                state: "open".to_string(),
                draft: false,
            },
            repository: RepositoryData {
                id: 789,
                full_name: "org/repo".to_string(),
                default_branch: "main".to_string(),
            },
        };
        let actions = handler.handle_pull_request(&event);
        assert!(actions.contains(&"create_preview".to_string()));
        assert!(actions.contains(&"run_design_review".to_string()));
    }

    #[test]
    fn webhook_handler_handles_pr_closed() {
        let handler = WebhookHandler::new("app-123", "inst-456");
        let event = PullRequestEvent {
            action: "closed".to_string(),
            number: 412,
            pull_request: PullRequestData {
                id: 123,
                number: 412,
                title: "Feature".to_string(),
                head: BranchData {
                    sha: "abc123".to_string(),
                    r#ref: "feature/x".to_string(),
                },
                base: BranchData {
                    sha: "def456".to_string(),
                    r#ref: "main".to_string(),
                },
                state: "closed".to_string(),
                draft: false,
            },
            repository: RepositoryData {
                id: 789,
                full_name: "org/repo".to_string(),
                default_branch: "main".to_string(),
            },
        };
        let actions = handler.handle_pull_request(&event);
        assert!(actions.contains(&"teardown_preview".to_string()));
    }

    #[test]
    fn webhook_handler_handles_push_to_main() {
        let handler = WebhookHandler::new("app-123", "inst-456");
        let event = PushEvent {
            r#ref: "refs/heads/main".to_string(),
            before: "abc123".to_string(),
            after: "def456".to_string(),
            repository: RepositoryData {
                id: 789,
                full_name: "org/repo".to_string(),
                default_branch: "main".to_string(),
            },
            pusher: PusherData {
                name: "user".to_string(),
                email: "user@example.com".to_string(),
            },
        };
        let actions = handler.handle_push(&event);
        assert!(actions.contains(&"trigger_prebuild".to_string()));
    }

    #[test]
    fn webhook_handler_handles_push_to_feature_branch() {
        let handler = WebhookHandler::new("app-123", "inst-456");
        let event = PushEvent {
            r#ref: "refs/heads/feature/x".to_string(),
            before: "abc123".to_string(),
            after: "def456".to_string(),
            repository: RepositoryData {
                id: 789,
                full_name: "org/repo".to_string(),
                default_branch: "main".to_string(),
            },
            pusher: PusherData {
                name: "user".to_string(),
                email: "user@example.com".to_string(),
            },
        };
        let actions = handler.handle_push(&event);
        assert!(!actions.contains(&"trigger_prebuild".to_string()));
    }

    #[test]
    fn webhook_handler_handles_check_run_rerequested() {
        let handler = WebhookHandler::new("app-123", "inst-456");
        let event = CheckRunEvent {
            action: "rerequested".to_string(),
            check_run: CheckRunData {
                id: 123,
                name: "design-review".to_string(),
                status: "queued".to_string(),
                conclusion: None,
                output: None,
            },
            repository: RepositoryData {
                id: 789,
                full_name: "org/repo".to_string(),
                default_branch: "main".to_string(),
            },
        };
        let actions = handler.handle_check_run(&event);
        assert!(actions.contains(&"rerun_design_review".to_string()));
    }

    #[test]
    fn pull_request_event_deserializes() {
        let json = r#"{"action":"opened","number":412,"pull_request":{"id":123,"number":412,"title":"Feature","head":{"sha":"abc123","ref":"feature/x"},"base":{"sha":"def456","ref":"main"},"state":"open","draft":false},"repository":{"id":789,"full_name":"org/repo","default_branch":"main"}}"#;
        let event: PullRequestEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.number, 412);
    }

    #[test]
    fn push_event_deserializes() {
        let json = r#"{"ref":"refs/heads/main","before":"abc123","after":"def456","repository":{"id":789,"full_name":"org/repo","default_branch":"main"},"pusher":{"name":"user","email":"user@example.com"}}"#;
        let event: PushEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.r#ref, "refs/heads/main");
    }

    #[test]
    fn check_run_event_deserializes() {
        let json = r#"{"action":"rerequested","check_run":{"id":123,"name":"design-review","status":"queued","conclusion":null,"output":null},"repository":{"id":789,"full_name":"org/repo","default_branch":"main"}}"#;
        let event: CheckRunEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.check_run.name, "design-review");
    }

    #[test]
    fn check_run_data_serializes() {
        let data = CheckRunData {
            id: 123,
            name: "design-review".to_string(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
            output: Some(CheckRunOutput {
                title: "Design Review".to_string(),
                summary: "All checks passed".to_string(),
            }),
        };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("design-review"));
    }
}
