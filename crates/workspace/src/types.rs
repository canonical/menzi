use menzi_common::ids::{ProjectId, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: menzi_common::ids::WorkspaceId,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub name: String,
    pub status: WorkspaceStatus,
    pub instance_name: Option<String>,
    pub endpoint: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Requested,
    Provisioning,
    Ready,
    Running,
    Idle,
    Archived,
    Deleted,
}

impl std::fmt::Display for WorkspaceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            WorkspaceStatus::Requested => "requested",
            WorkspaceStatus::Provisioning => "provisioning",
            WorkspaceStatus::Ready => "ready",
            WorkspaceStatus::Running => "running",
            WorkspaceStatus::Idle => "idle",
            WorkspaceStatus::Archived => "archived",
            WorkspaceStatus::Deleted => "deleted",
        };
        write!(f, "{name}")
    }
}

impl WorkspaceStatus {
    pub fn accepts_work(self) -> bool {
        matches!(
            self,
            WorkspaceStatus::Ready | WorkspaceStatus::Running | WorkspaceStatus::Idle
        )
    }

    pub fn is_live(self) -> bool {
        !matches!(self, WorkspaceStatus::Deleted)
    }

    pub fn is_provisioning(self) -> bool {
        matches!(
            self,
            WorkspaceStatus::Requested | WorkspaceStatus::Provisioning
        )
    }

    pub fn can_transition_to(self, next: WorkspaceStatus) -> bool {
        use WorkspaceStatus::*;
        match (self, next) {
            (Requested, Provisioning) => true,
            (Provisioning, Ready) => true,
            (Provisioning, Requested) => true,
            (Ready, Running) | (Ready, Idle) => true,
            (Running, Idle) | (Idle, Running) => true,
            (Ready, Archived) | (Running, Archived) | (Idle, Archived) => true,
            (Archived, Ready) => true,
            (Requested, Deleted)
            | (Provisioning, Deleted)
            | (Ready, Deleted)
            | (Running, Deleted)
            | (Idle, Deleted)
            | (Archived, Deleted) => true,
            (Deleted, _) => false,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceKey {
    pub user_id: UserId,
    pub project_id: ProjectId,
}

impl WorkspaceKey {
    pub fn new(user_id: UserId, project_id: ProjectId) -> Self {
        Self {
            user_id,
            project_id,
        }
    }

    pub fn instance_name(&self) -> String {
        format!(
            "wsp-{}-{}",
            short(self.user_id.to_string().as_str()),
            short(self.project_id.to_string().as_str())
        )
    }
}

fn short(id: &str) -> String {
    id.replace('-', "").chars().take(8).collect()
}

#[derive(Debug, Clone)]
pub struct WorkspaceSpec {
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub source_instance: String,
    pub name: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub title: Option<String>,
    pub directory: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeChange {
    pub file: String,
    pub previous: Option<String>,
    pub additions: usize,
    pub deletions: usize,
    pub status: String,
    pub binary: bool,
    pub truncated: bool,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeDiff {
    pub head: String,
    pub version: String,
    pub changes: Vec<TreeChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptOutcome {
    pub session_id: String,
    pub message_id: Option<String>,
    pub finish_reason: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRequest {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub timeout_secs: Option<u64>,
}

impl TerminalRequest {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            timeout_secs: None,
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn timeout_secs(mut self, timeout_secs: Option<u64>) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    pub fn in_dir(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl TerminalResult {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSpec {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

impl From<TerminalSpec> for TerminalRequest {
    fn from(spec: TerminalSpec) -> Self {
        Self {
            command: spec.command,
            args: spec.args,
            cwd: spec.cwd,
            timeout_secs: spec.timeout_secs,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileReport {
    pub adopted: Vec<String>,
    pub released: Vec<String>,
    pub reaped: Vec<String>,
    pub failed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> ProjectId {
        ProjectId::new()
    }

    #[test]
    fn instance_name_is_derived_and_stable() {
        let key = WorkspaceKey::new(UserId::new(), ProjectId::new());
        assert_eq!(key.instance_name(), key.instance_name());
        assert!(key.instance_name().starts_with("wsp-"));
    }

    #[test]
    fn instance_name_differs_per_user_and_project() {
        let user = UserId::new();
        let by_user = WorkspaceKey::new(user, ProjectId::new()).instance_name();
        let by_project = WorkspaceKey::new(UserId::new(), project()).instance_name();
        assert_ne!(by_user, by_project);
    }

    #[test]
    fn instance_name_is_lxd_safe() {
        let name = WorkspaceKey::new(UserId::new(), ProjectId::new()).instance_name();
        assert!(name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert!(name.len() <= 63);
    }

    #[test]
    fn status_rejects_provisioning_in_a_hurry() {
        assert!(!WorkspaceStatus::Requested.can_transition_to(WorkspaceStatus::Running));
        assert!(!WorkspaceStatus::Ready.can_transition_to(WorkspaceStatus::Provisioning));
    }

    #[test]
    fn status_allows_the_happy_path() {
        assert!(WorkspaceStatus::Requested.can_transition_to(WorkspaceStatus::Provisioning));
        assert!(WorkspaceStatus::Provisioning.can_transition_to(WorkspaceStatus::Ready));
        assert!(WorkspaceStatus::Ready.can_transition_to(WorkspaceStatus::Running));
        assert!(WorkspaceStatus::Running.can_transition_to(WorkspaceStatus::Idle));
        assert!(WorkspaceStatus::Idle.can_transition_to(WorkspaceStatus::Running));
    }

    #[test]
    fn a_deleted_workspace_is_terminal() {
        for next in [
            WorkspaceStatus::Requested,
            WorkspaceStatus::Provisioning,
            WorkspaceStatus::Ready,
            WorkspaceStatus::Running,
        ] {
            assert!(!WorkspaceStatus::Deleted.can_transition_to(next));
        }
        assert!(!WorkspaceStatus::Deleted.is_live());
    }

    #[test]
    fn any_live_state_can_be_torn_down() {
        for from in [
            WorkspaceStatus::Requested,
            WorkspaceStatus::Provisioning,
            WorkspaceStatus::Ready,
            WorkspaceStatus::Running,
            WorkspaceStatus::Idle,
            WorkspaceStatus::Archived,
        ] {
            assert!(from.can_transition_to(WorkspaceStatus::Deleted), "{from:?}");
        }
    }

    #[test]
    fn work_is_only_accepted_once_provisioned() {
        assert!(!WorkspaceStatus::Requested.accepts_work());
        assert!(!WorkspaceStatus::Provisioning.accepts_work());
        assert!(WorkspaceStatus::Ready.accepts_work());
        assert!(WorkspaceStatus::Running.accepts_work());
        assert!(!WorkspaceStatus::Deleted.accepts_work());
    }

    #[test]
    fn provisioning_states_are_the_ones_a_retry_must_rebuild() {
        assert!(WorkspaceStatus::Requested.is_provisioning());
        assert!(WorkspaceStatus::Provisioning.is_provisioning());
        assert!(!WorkspaceStatus::Ready.is_provisioning());
    }

    #[test]
    fn status_serializes_as_the_database_stores_it() {
        assert_eq!(
            serde_json::to_string(&WorkspaceStatus::Provisioning).unwrap(),
            "\"provisioning\""
        );
        assert_eq!(
            serde_json::from_str::<WorkspaceStatus>("\"archived\"").unwrap(),
            WorkspaceStatus::Archived
        );
    }

    #[test]
    fn status_displays_as_the_database_stores_it() {
        assert_eq!(WorkspaceStatus::Provisioning.to_string(), "provisioning");
        assert_eq!(WorkspaceStatus::Deleted.to_string(), "deleted");
    }

    #[test]
    fn terminal_request_builds_a_command_line() {
        let request = TerminalRequest::new("ls").arg("-la").in_dir("/workspace");
        assert_eq!(request.command, "ls");
        assert_eq!(request.args, vec!["-la"]);
        assert_eq!(request.cwd.as_deref(), Some("/workspace"));
    }

    #[test]
    fn terminal_result_reports_success() {
        let ok = TerminalResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
        };
        assert!(ok.succeeded());
        let timed_out = TerminalResult {
            timed_out: true,
            ..ok.clone()
        };
        assert!(!timed_out.succeeded());
    }
}
