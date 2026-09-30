use menzi_common::ids::{ProjectId, UserId, WorkspaceId};
use menzi_common::{MenziError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

use crate::driver::{ProvisionOutcome, WorkspaceDriver};
use crate::gateway::OpencodeGateway;
use crate::registry::{SessionBinding, SessionKind, SessionRegistry};
use crate::store::WorkspaceStore;
use crate::types::{
    AgentSession, PromptOutcome, ReconcileReport, TerminalRequest, TerminalResult, Workspace,
    WorkspaceKey, WorkspaceSpec, WorkspaceStatus,
};

pub trait WorkspaceAudit: Send + Sync {
    fn record(&self, action: &str, principal: &str, key: &WorkspaceKey, detail: &str);
}

pub struct NoopAudit;

impl WorkspaceAudit for NoopAudit {
    fn record(&self, _action: &str, _principal: &str, _key: &WorkspaceKey, _detail: &str) {}
}

#[derive(Clone)]
pub struct WorkspaceManager {
    driver: Arc<dyn WorkspaceDriver>,
    gateway: Arc<dyn OpencodeGateway>,
    store: Arc<dyn WorkspaceStore>,
    audit: Arc<dyn WorkspaceAudit>,
    sessions: Arc<dyn SessionRegistry>,
    source_instance: String,
    locks: Arc<std::sync::Mutex<HashMap<WorkspaceKey, Arc<AsyncMutex<()>>>>>,
    health_attempts: u32,
    health_delay: std::time::Duration,
}

impl WorkspaceManager {
    pub fn new(
        driver: Arc<dyn WorkspaceDriver>,
        gateway: Arc<dyn OpencodeGateway>,
        store: Arc<dyn WorkspaceStore>,
        source_instance: impl Into<String>,
    ) -> Self {
        Self {
            driver,
            gateway,
            store,
            audit: Arc::new(NoopAudit),
            sessions: Arc::new(crate::registry::registry_noop()),
            source_instance: source_instance.into(),
            locks: Arc::new(std::sync::Mutex::new(HashMap::new())),
            health_attempts: 15,
            health_delay: std::time::Duration::from_secs(2),
        }
    }

    pub fn with_audit(mut self, audit: Arc<dyn WorkspaceAudit>) -> Self {
        self.audit = audit;
        self
    }

    pub fn with_sessions(mut self, sessions: Arc<dyn SessionRegistry>) -> Self {
        self.sessions = sessions;
        self
    }

    pub fn with_health_wait(mut self, attempts: u32, delay: std::time::Duration) -> Self {
        self.health_attempts = attempts;
        self.health_delay = delay;
        self
    }

    pub fn source_instance(&self) -> &str {
        &self.source_instance
    }

    async fn wait_healthy(&self, endpoint: &str) -> Result<()> {
        let mut last = None;
        for attempt in 0..self.health_attempts {
            match self.gateway.health(endpoint).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last = Some(error);
                    if attempt + 1 < self.health_attempts {
                        tokio::time::sleep(self.health_delay).await;
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| MenziError::Gateway("opencode unreachable".to_string())))
    }

    pub async fn ensure_workspace(
        &self,
        principal: &str,
        spec: WorkspaceSpec,
    ) -> Result<Workspace> {
        let key = WorkspaceKey::new(spec.user_id, spec.project_id);
        let lock = self.lock_for(key);
        let _guard = lock.lock().await;

        if let Some(existing) = self.store.get(&key).await? {
            if existing.status.accepts_work() || existing.status == WorkspaceStatus::Archived {
                self.audit.record("ensure:reuse", principal, &key, "");
                return Ok(existing);
            }
        }
        self.provision(&key, &spec, principal).await
    }

    async fn provision(
        &self,
        key: &WorkspaceKey,
        spec: &WorkspaceSpec,
        principal: &str,
    ) -> Result<Workspace> {
        let instance = key.instance_name();
        let mut workspace = Workspace {
            id: WorkspaceId::new(),
            user_id: key.user_id,
            project_id: key.project_id,
            name: spec.name.clone().unwrap_or_else(|| instance.clone()),
            status: WorkspaceStatus::Provisioning,
            instance_name: Some(instance.clone()),
            endpoint: None,
            branch: spec.branch.clone(),
            commit_sha: spec.commit_sha.clone(),
            last_error: None,
            created_at: now(),
            updated_at: now(),
        };
        self.store.save(&workspace).await?;
        self.audit
            .record("provision:start", principal, key, &workspace.name);

        let outcome = match self
            .driver
            .provision(&instance, &spec.source_instance)
            .await
        {
            Ok(outcome) => outcome,
            Err(error) => {
                workspace.status = WorkspaceStatus::Requested;
                workspace.last_error = Some(error.to_string());
                workspace.updated_at = now();
                self.store.save(&workspace).await?;
                self.audit
                    .record("provision:failed", principal, key, &error.to_string());
                return Err(error);
            }
        };

        match self.driver.resolve_endpoint(&instance).await {
            Ok(Some(endpoint)) => workspace.endpoint = Some(endpoint),
            Ok(None) => workspace.endpoint = Some(self.driver.endpoint_template(&instance)),
            Err(error) => {
                workspace.status = WorkspaceStatus::Requested;
                workspace.last_error = Some(error.to_string());
                workspace.updated_at = now();
                self.store.save(&workspace).await?;
                return Err(error);
            }
        }

        workspace.status = WorkspaceStatus::Ready;
        workspace.last_error = None;
        workspace.updated_at = now();
        self.store.save(&workspace).await?;
        self.audit.record(
            match outcome {
                ProvisionOutcome::Created => "provision:created",
                ProvisionOutcome::Adopted => "provision:adopted",
            },
            principal,
            key,
            &workspace.name,
        );
        Ok(workspace)
    }

    pub async fn get_workspace(&self, key: &WorkspaceKey) -> Result<Workspace> {
        self.store
            .get(key)
            .await?
            .ok_or_else(|| MenziError::NotFound(format!("workspace for {}", key.instance_name())))
    }

    pub async fn get_workspace_by_id(&self, id: WorkspaceId) -> Result<Workspace> {
        self.store
            .get_by_id(id)
            .await?
            .ok_or_else(|| MenziError::NotFound(format!("workspace {id}")))
    }

    pub async fn list_workspaces(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
        self.store.list_for_project(project_id).await
    }

    pub async fn find(&self, user_id: UserId, project_id: ProjectId) -> Result<Option<Workspace>> {
        self.store
            .get(&WorkspaceKey::new(user_id, project_id))
            .await
    }

    pub async fn connect(&self, key: &WorkspaceKey) -> Result<String> {
        let workspace = self.get_workspace(key).await?;
        if !workspace.status.accepts_work() {
            return Err(MenziError::Conflict(format!(
                "workspace {} is {}",
                key.instance_name(),
                workspace.status
            )));
        }
        let instance = workspace
            .instance_name
            .ok_or_else(|| MenziError::Conflict("workspace has no instance".to_string()))?;
        if !self.driver.is_running(&instance).await? {
            self.start(key).await?;
        }
        let endpoint = match self.driver.resolve_endpoint(&instance).await? {
            Some(endpoint) => endpoint,
            None => self.driver.endpoint_template(&instance),
        };
        self.wait_healthy(&endpoint).await?;
        Ok(endpoint)
    }

    pub async fn start(&self, key: &WorkspaceKey) -> Result<Workspace> {
        let mut workspace = self.get_workspace(key).await?;
        if let Some(instance) = workspace.instance_name.clone() {
            self.driver.start(&instance).await?;
            match self.driver.resolve_endpoint(&instance).await? {
                Some(endpoint) => workspace.endpoint = Some(endpoint),
                None => workspace.endpoint = Some(self.driver.endpoint_template(&instance)),
            }
        }
        if workspace.status == WorkspaceStatus::Idle
            || workspace.status == WorkspaceStatus::Archived
        {
            workspace.status = WorkspaceStatus::Ready;
        }
        workspace.updated_at = now();
        self.store.save(&workspace).await?;
        Ok(workspace)
    }

    pub async fn open_session(
        &self,
        principal: &str,
        key: &WorkspaceKey,
        title: Option<&str>,
    ) -> Result<AgentSession> {
        let session = self.open_session_untracked(key, title).await?;
        if let Err(error) = self
            .record_binding(principal, key, &session, SessionKind::Interactive)
            .await
        {
            tracing::warn!("session {} was not recorded: {error}", session.id);
        }
        Ok(session)
    }

    pub async fn open_session_untracked(
        &self,
        key: &WorkspaceKey,
        title: Option<&str>,
    ) -> Result<AgentSession> {
        let endpoint = self.connect(key).await?;
        self.gateway.create_session(&endpoint, title).await
    }

    /// Where a session runs, so the proxy can send its traffic to the right
    /// opencode after a restart rather than falling back to the shared one.
    pub async fn record_binding(
        &self,
        principal: &str,
        key: &WorkspaceKey,
        session: &AgentSession,
        kind: SessionKind,
    ) -> Result<SessionBinding> {
        let workspace = self.get_workspace(key).await?;
        let endpoint = self.connect(key).await.or_else(|_| {
            workspace.endpoint.clone().ok_or_else(|| {
                MenziError::Conflict("workspace has no reachable endpoint".to_string())
            })
        })?;
        let binding = SessionBinding {
            opencode_session: session.id.clone(),
            workspace_id: workspace.id,
            user_id: key.user_id,
            project_id: key.project_id,
            endpoint,
            instance_name: workspace.instance_name.clone(),
            kind,
            principal: principal.to_string(),
            created_at: now(),
        };
        self.sessions.bind(&binding).await?;
        Ok(binding)
    }

    pub async fn session_binding(&self, opencode_session: &str) -> Result<Option<SessionBinding>> {
        self.sessions.binding(opencode_session).await
    }

    pub async fn workspace_sessions(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<SessionBinding>> {
        self.sessions.bindings_for_workspace(workspace_id).await
    }

    pub async fn list_sessions(&self, key: &WorkspaceKey) -> Result<Vec<AgentSession>> {
        let endpoint = self.connect(key).await?;
        self.gateway.sessions(&endpoint).await
    }

    pub async fn prompt(
        &self,
        key: &WorkspaceKey,
        session_id: &str,
        text: &str,
    ) -> Result<PromptOutcome> {
        self.prompt_as("unknown", key, session_id, text, SessionKind::Interactive)
            .await
    }

    pub async fn prompt_as(
        &self,
        principal: &str,
        key: &WorkspaceKey,
        session_id: &str,
        text: &str,
        kind: SessionKind,
    ) -> Result<PromptOutcome> {
        if self.sessions.binding(session_id).await?.is_none() {
            let session = AgentSession {
                id: session_id.to_string(),
                title: None,
            };
            self.record_binding(principal, key, &session, kind).await?;
        }
        let endpoint = self.connect(key).await?;
        let mut workspace = self.get_workspace(key).await?;
        let outcome = self.gateway.prompt(&endpoint, session_id, text).await?;
        if workspace.status == WorkspaceStatus::Ready || workspace.status == WorkspaceStatus::Idle {
            workspace.status = WorkspaceStatus::Running;
            workspace.updated_at = now();
            self.store.save(&workspace).await?;
        }
        Ok(outcome)
    }

    pub async fn interrupt(&self, key: &WorkspaceKey, session_id: &str) -> Result<()> {
        let endpoint = self.connect(key).await?;
        self.gateway.interrupt(&endpoint, session_id).await
    }

    pub async fn terminal(
        &self,
        key: &WorkspaceKey,
        request: &TerminalRequest,
    ) -> Result<TerminalResult> {
        let workspace = self.get_workspace(key).await?;
        if !workspace.status.accepts_work() {
            return Err(MenziError::Conflict(format!(
                "workspace {} is {}",
                key.instance_name(),
                workspace.status
            )));
        }
        let instance = workspace
            .instance_name
            .ok_or_else(|| MenziError::Conflict("workspace has no instance".to_string()))?;
        if !self.driver.is_running(&instance).await? {
            self.start(key).await?;
        }
        self.driver.exec(&instance, request).await
    }

    pub async fn destroy_workspace(&self, principal: &str, key: &WorkspaceKey) -> Result<()> {
        let lock = self.lock_for(*key);
        let _guard = lock.lock().await;
        let mut workspace = self.get_workspace(key).await?;
        if let Some(instance) = workspace.instance_name.clone() {
            self.driver.destroy(&instance).await?;
        }
        workspace.status = WorkspaceStatus::Deleted;
        workspace.updated_at = now();
        self.store.save(&workspace).await?;
        self.audit
            .record("destroy", principal, key, workspace.name.as_str());
        Ok(())
    }

    pub async fn suspend_workspace(&self, key: &WorkspaceKey) -> Result<Workspace> {
        let mut workspace = self.get_workspace(key).await?;
        if let Some(instance) = workspace.instance_name.clone() {
            self.driver.stop(&instance, false).await?;
        }
        workspace.status = WorkspaceStatus::Idle;
        workspace.updated_at = now();
        self.store.save(&workspace).await?;
        Ok(workspace)
    }

    pub async fn archive_workspace(&self, key: &WorkspaceKey) -> Result<Workspace> {
        let mut workspace = self.get_workspace(key).await?;
        if let Some(instance) = workspace.instance_name.clone() {
            self.driver.stop(&instance, true).await?;
        }
        workspace.status = WorkspaceStatus::Archived;
        workspace.updated_at = now();
        self.store.save(&workspace).await?;
        Ok(workspace)
    }

    pub async fn reconcile(&self) -> Result<ReconcileReport> {
        let mut report = ReconcileReport::default();
        for project in self.known_projects().await? {
            for workspace in self.store.list_for_project(project).await? {
                let Some(instance) = workspace.instance_name.clone() else {
                    continue;
                };
                let key = WorkspaceKey::new(workspace.user_id, workspace.project_id);
                if workspace.status.is_provisioning() {
                    let exists = self.driver.exists(&instance).await.unwrap_or(false);
                    if exists {
                        self.adopt(&mut report, workspace, &key).await;
                    } else {
                        report.released.push(key.instance_name());
                    }
                    continue;
                }
                if !workspace.status.is_live() {
                    continue;
                }
                match self.driver.is_running(&instance).await {
                    Ok(true) => {}
                    Ok(false) => {
                        if self.recycle(&mut report, workspace, &key).await {
                            report.reaped.push(key.instance_name());
                        }
                    }
                    Err(_) => report.failed.push(key.instance_name()),
                }
            }
        }
        Ok(report)
    }

    /// A record whose container has gone is put back to `requested` so the next
    /// caller re-provisions instead of being told a workspace that no longer
    /// exists is ready.
    async fn recycle(
        &self,
        report: &mut ReconcileReport,
        workspace: Workspace,
        key: &WorkspaceKey,
    ) -> bool {
        let mut workspace = workspace;
        workspace.status = WorkspaceStatus::Requested;
        workspace.endpoint = None;
        workspace.last_error = Some("the container is gone".to_string());
        workspace.updated_at = now();
        if self.store.save(&workspace).await.is_ok() {
            report.released.push(key.instance_name());
            true
        } else {
            report.failed.push(key.instance_name());
            false
        }
    }

    async fn adopt(&self, report: &mut ReconcileReport, workspace: Workspace, key: &WorkspaceKey) {
        let Some(instance) = workspace.instance_name.clone() else {
            return;
        };
        match self.driver.resolve_endpoint(&instance).await {
            Ok(Some(endpoint)) => {
                let mut workspace = workspace;
                workspace.status = WorkspaceStatus::Ready;
                workspace.endpoint = Some(endpoint);
                workspace.last_error = None;
                workspace.updated_at = now();
                if self.store.save(&workspace).await.is_ok() {
                    report.adopted.push(key.instance_name());
                }
            }
            Ok(None) => report.reaped.push(key.instance_name()),
            Err(_) => report.failed.push(key.instance_name()),
        }
    }

    async fn known_projects(&self) -> Result<Vec<ProjectId>> {
        let mut seen = Vec::new();
        for status in [
            WorkspaceStatus::Requested,
            WorkspaceStatus::Provisioning,
            WorkspaceStatus::Ready,
            WorkspaceStatus::Running,
            WorkspaceStatus::Idle,
            WorkspaceStatus::Archived,
        ] {
            for workspace in self.store.list_stale(&[status], now_time()).await? {
                if !seen.contains(&workspace.project_id) {
                    seen.push(workspace.project_id);
                }
            }
        }
        Ok(seen)
    }

    pub async fn reap(&self, older_than: chrono::Duration) -> Result<Vec<String>> {
        let cutoff = chrono::Utc::now() - older_than;
        let mut reaped = Vec::new();
        for workspace in self
            .store
            .list_stale(&[WorkspaceStatus::Provisioning], cutoff)
            .await?
        {
            let key = WorkspaceKey::new(workspace.user_id, workspace.project_id);
            if let Some(instance) = workspace.instance_name.clone() {
                let _ = self.driver.destroy(&instance).await;
            }
            reaped.push(key.instance_name());
        }
        Ok(reaped)
    }

    fn lock_for(&self, key: WorkspaceKey) -> Arc<AsyncMutex<()>> {
        let mut locks = self.locks.lock().expect("workspace locks");
        locks
            .entry(key)
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn now_time() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

#[cfg(test)]
pub(crate) mod testbed {
    use super::*;
    use crate::driver::WorkspaceDriver;
    pub use crate::store::testbed::spec;
    use async_trait::async_trait;
    use menzi_common::Result;
    use std::sync::Mutex as StdMutex;

    #[derive(Clone, Default)]
    pub struct RecordingDriver {
        pub calls: Arc<StdMutex<Vec<String>>>,
        pub fail_provision: Arc<StdMutex<bool>>,
        pub fail_resolve: Arc<StdMutex<bool>>,
        pub running: Arc<StdMutex<bool>>,
    }

    #[async_trait]
    impl WorkspaceDriver for RecordingDriver {
        async fn provision(&self, instance: &str, source: &str) -> Result<ProvisionOutcome> {
            if *self.fail_provision.lock().unwrap() {
                return Err(MenziError::Lxd("provision refused".to_string()));
            }
            self.calls
                .lock()
                .unwrap()
                .push(format!("provision:{source}:{instance}"));
            *self.running.lock().unwrap() = true;
            Ok(ProvisionOutcome::Created)
        }

        async fn start(&self, instance: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("start:{instance}"));
            *self.running.lock().unwrap() = true;
            Ok(())
        }

        async fn stop(&self, instance: &str, force: bool) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{instance}:{force}"));
            *self.running.lock().unwrap() = false;
            Ok(())
        }

        async fn destroy(&self, instance: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("destroy:{instance}"));
            *self.running.lock().unwrap() = false;
            Ok(())
        }

        async fn exec(&self, instance: &str, request: &TerminalRequest) -> Result<TerminalResult> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("exec:{instance}:{request:?}"));
            Ok(TerminalResult {
                exit_code: 0,
                stdout: "ok".to_string(),
                stderr: String::new(),
                timed_out: false,
            })
        }

        async fn exists(&self, _instance: &str) -> Result<bool> {
            Ok(*self.running.lock().unwrap())
        }

        async fn is_running(&self, _instance: &str) -> Result<bool> {
            Ok(*self.running.lock().unwrap())
        }

        async fn resolve_endpoint(&self, _instance: &str) -> Result<Option<String>> {
            if *self.fail_resolve.lock().unwrap() {
                return Err(MenziError::Lxd("state unavailable".to_string()));
            }
            if *self.running.lock().unwrap() {
                Ok(Some("http://10.0.0.9:17999".to_string()))
            } else {
                Ok(None)
            }
        }

        fn endpoint_template(&self, instance: &str) -> String {
            format!("http://{instance}.dev.local:17999")
        }
    }

    #[derive(Clone, Default)]
    pub struct RecordingGateway {
        pub calls: Arc<StdMutex<Vec<String>>>,
        pub healthy: Arc<StdMutex<bool>>,
    }

    #[async_trait]
    impl OpencodeGateway for RecordingGateway {
        async fn health(&self, endpoint: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("health:{endpoint}"));
            if *self.healthy.lock().unwrap() {
                Ok(())
            } else {
                Err(MenziError::Gateway("unhealthy".to_string()))
            }
        }

        async fn create_session(
            &self,
            endpoint: &str,
            title: Option<&str>,
        ) -> Result<AgentSession> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("session:{endpoint}:{title:?}"));
            Ok(AgentSession {
                id: "ses_1".to_string(),
                title: title.map(str::to_string),
            })
        }

        async fn prompt(
            &self,
            endpoint: &str,
            session_id: &str,
            text: &str,
        ) -> Result<PromptOutcome> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("prompt:{endpoint}:{session_id}:{text}"));
            Ok(PromptOutcome {
                session_id: session_id.to_string(),
                message_id: Some("msg_1".to_string()),
                finish_reason: Some("stop".to_string()),
                error: None,
            })
        }

        async fn interrupt(&self, endpoint: &str, session_id: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("interrupt:{endpoint}:{session_id}"));
            Ok(())
        }

        async fn sessions(&self, endpoint: &str) -> Result<Vec<AgentSession>> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("sessions:{endpoint}"));
            Ok(vec![AgentSession {
                id: "ses_1".to_string(),
                title: None,
            }])
        }
    }

    #[derive(Clone, Default)]
    pub struct RecordingAudit {
        pub entries: Arc<StdMutex<Vec<String>>>,
    }

    impl WorkspaceAudit for RecordingAudit {
        fn record(&self, action: &str, principal: &str, key: &WorkspaceKey, detail: &str) {
            self.entries.lock().unwrap().push(format!(
                "{action}:{principal}:{}:{detail}",
                key.instance_name()
            ));
        }
    }

    pub struct Harness {
        pub manager: WorkspaceManager,
        pub driver: RecordingDriver,
        pub gateway: RecordingGateway,
        pub audit: RecordingAudit,
        pub store: Arc<crate::store::InMemoryWorkspaceStore>,
        pub registry: Arc<crate::registry::testbed::RecordingRegistry>,
    }

    pub fn harness() -> Harness {
        let driver = RecordingDriver {
            running: Arc::new(StdMutex::new(false)),
            ..Default::default()
        };
        let gateway = RecordingGateway {
            healthy: Arc::new(StdMutex::new(true)),
            ..Default::default()
        };
        let audit = RecordingAudit::default();
        let store = Arc::new(crate::store::InMemoryWorkspaceStore::new());
        let registry = Arc::new(crate::registry::testbed::RecordingRegistry::new());
        let manager = WorkspaceManager::new(
            Arc::new(driver.clone()),
            Arc::new(gateway.clone()),
            store.clone(),
            "mz-workspace",
        )
        .with_health_wait(3, std::time::Duration::from_millis(5))
        .with_sessions(registry.clone())
        .with_audit(Arc::new(audit.clone()));
        Harness {
            manager,
            driver,
            gateway,
            audit,
            store,
            registry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testbed::*;
    use super::*;

    fn key(user: UserId, project: ProjectId) -> WorkspaceKey {
        WorkspaceKey::new(user, project)
    }

    #[tokio::test]
    async fn ensure_provisions_a_container_from_the_template() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        let workspace = h
            .manager
            .ensure_workspace("principal", spec(k.user_id, k.project_id))
            .await
            .unwrap();

        assert_eq!(workspace.status, WorkspaceStatus::Ready);
        assert_eq!(
            workspace.instance_name.as_deref(),
            Some(k.instance_name().as_str())
        );
        assert_eq!(workspace.endpoint.as_deref(), Some("http://10.0.0.9:17999"));
        assert!(h
            .driver
            .calls
            .lock()
            .unwrap()
            .contains(&format!("provision:mz-workspace:{}", k.instance_name())));
    }

    #[tokio::test]
    async fn ensure_is_idempotent_for_the_same_user_and_project() {
        let h = harness();
        let user = UserId::new();
        let project = ProjectId::new();
        let first = h
            .manager
            .ensure_workspace("p", spec(user, project))
            .await
            .unwrap();
        let second = h
            .manager
            .ensure_workspace("p", spec(user, project))
            .await
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(h.driver.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn different_users_get_different_workspaces_for_one_project() {
        let h = harness();
        let project = ProjectId::new();
        let mine = h
            .manager
            .ensure_workspace("p", spec(UserId::new(), project))
            .await
            .unwrap();
        let theirs = h
            .manager
            .ensure_workspace("p", spec(UserId::new(), project))
            .await
            .unwrap();
        assert_ne!(mine.instance_name, theirs.instance_name);
    }

    #[tokio::test]
    async fn one_user_gets_one_workspace_per_project() {
        let h = harness();
        let user = UserId::new();
        let project = ProjectId::new();
        let first = h
            .manager
            .ensure_workspace("p", spec(user, project))
            .await
            .unwrap();
        let second = h
            .manager
            .ensure_workspace("p", spec(user, project))
            .await
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(h.driver.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_failed_provision_leaves_the_workspace_retryable_and_records_why() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        *h.driver.fail_provision.lock().unwrap() = true;

        assert!(h
            .manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .is_err());

        let stored = h.store.get(&k).await.unwrap().unwrap();
        assert_eq!(stored.status, WorkspaceStatus::Requested);
        assert!(stored.last_error.is_some());

        *h.driver.fail_provision.lock().unwrap() = false;
        let retried = h
            .manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        assert_eq!(retried.status, WorkspaceStatus::Ready);
        assert!(retried.last_error.is_none());
    }

    #[tokio::test]
    async fn a_failed_endpoint_resolve_does_not_mark_a_workspace_ready() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        *h.driver.fail_resolve.lock().unwrap() = true;

        assert!(h
            .manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .is_err());
        let stored = h.store.get(&k).await.unwrap().unwrap();
        assert_eq!(stored.status, WorkspaceStatus::Requested);
    }

    #[tokio::test]
    async fn connect_resolves_the_workspace_endpoint() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        assert_eq!(
            h.manager.connect(&k).await.unwrap(),
            "http://10.0.0.9:17999"
        );
    }

    #[tokio::test]
    async fn connect_reports_a_workspace_that_does_not_exist() {
        let h = harness();
        assert!(matches!(
            h.manager
                .connect(&key(UserId::new(), ProjectId::new()))
                .await,
            Err(MenziError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn connect_surfaces_an_unhealthy_opencode() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        *h.gateway.healthy.lock().unwrap() = false;
        assert!(matches!(
            h.manager.connect(&k).await,
            Err(MenziError::Gateway(_))
        ));
    }

    #[tokio::test]
    async fn connect_waits_for_an_opencode_that_is_still_starting() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        let gateway = h.gateway.clone();
        let warmer = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            *gateway.healthy.lock().unwrap() = true;
        });
        let endpoint = h.manager.connect(&k).await.unwrap();
        warmer.await.unwrap();
        assert_eq!(endpoint, "http://10.0.0.9:17999");
    }

    #[tokio::test]
    async fn connect_starts_a_stopped_container() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        *h.driver.running.lock().unwrap() = false;
        h.manager.suspend_workspace(&k).await.unwrap();

        let endpoint = h.manager.connect(&k).await.unwrap();
        assert_eq!(endpoint, "http://10.0.0.9:17999");
        assert!(h
            .driver
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call == &format!("start:{}", k.instance_name())));
    }

    #[tokio::test]
    async fn opening_a_session_records_where_it_runs() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();

        let session = h.manager.open_session("alice", &k, None).await.unwrap();
        let binding = h
            .manager
            .session_binding(&session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(binding.endpoint, "http://10.0.0.9:17999");
        assert_eq!(
            binding.workspace_id,
            h.store.get(&k).await.unwrap().unwrap().id
        );
        assert_eq!(binding.principal, "alice");
        assert_eq!(binding.kind, SessionKind::Interactive);
        assert_eq!(
            binding.instance_name.as_deref(),
            Some(k.instance_name().as_str())
        );
    }

    #[tokio::test]
    async fn a_prompt_binds_the_session_it_uses() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();

        h.manager
            .prompt_as(
                "autonomous",
                &k,
                "ses_autonomous",
                "review the diff",
                SessionKind::Autonomous,
            )
            .await
            .unwrap();

        let binding = h
            .manager
            .session_binding("ses_autonomous")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(binding.kind, SessionKind::Autonomous);
        assert_eq!(binding.principal, "autonomous");
    }

    #[tokio::test]
    async fn a_session_already_bound_is_not_bound_again() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        let session = h.manager.open_session("alice", &k, None).await.unwrap();

        h.manager.prompt(&k, &session.id, "again").await.unwrap();
        let bound = h.registry.bound.lock().unwrap();
        assert_eq!(bound.len(), 1, "a bound session should not be rebound");
    }

    #[tokio::test]
    async fn sessions_can_be_listed_for_a_workspace() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        let session = h.manager.open_session("alice", &k, None).await.unwrap();
        let workspace = h.store.get(&k).await.unwrap().unwrap();

        let listed = h.manager.workspace_sessions(workspace.id).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].opencode_session, session.id);
    }

    #[tokio::test]
    async fn a_registry_that_refuses_does_not_stop_a_session_opening() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        *h.registry.fail_bind.lock().unwrap() = true;
        let session = h.manager.open_session("alice", &k, None).await.unwrap();
        assert_eq!(session.id, "ses_1");
        assert!(h.manager.session_binding("ses_1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn open_session_and_prompt_run_in_that_workspace() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();

        let session = h
            .manager
            .open_session("alice", &k, Some("refactor"))
            .await
            .unwrap();
        let outcome = h
            .manager
            .prompt(&k, &session.id, "do the thing")
            .await
            .unwrap();

        assert_eq!(session.id, "ses_1");
        assert_eq!(outcome.session_id, "ses_1");
        let calls = h.gateway.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call.contains("session:")));
        assert!(calls.iter().any(|call| call.ends_with(":do the thing")));
    }

    #[tokio::test]
    async fn prompting_marks_the_workspace_running() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager.prompt(&k, "ses_1", "hello").await.unwrap();
        assert_eq!(
            h.store.get(&k).await.unwrap().unwrap().status,
            WorkspaceStatus::Running
        );
    }

    #[tokio::test]
    async fn interrupt_reaches_the_workspace_opencode() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager.interrupt(&k, "ses_1").await.unwrap();
        assert!(h
            .gateway
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.starts_with("interrupt:")));
    }

    #[tokio::test]
    async fn terminal_runs_in_the_workspace_container() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        let result = h
            .manager
            .terminal(&k, &TerminalRequest::new("ls").arg("-la"))
            .await
            .unwrap();
        assert!(result.succeeded());
        assert!(h
            .driver
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| { call.starts_with(&format!("exec:{}", k.instance_name())) }));
    }

    #[tokio::test]
    async fn destroy_removes_the_container_and_keeps_the_record() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager.destroy_workspace("p", &k).await.unwrap();

        assert!(h
            .driver
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call == &format!("destroy:{}", k.instance_name())));
        assert_eq!(
            h.store.get(&k).await.unwrap().unwrap().status,
            WorkspaceStatus::Deleted
        );
    }

    #[tokio::test]
    async fn a_destroyed_workspace_is_replaced_on_the_next_ensure() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager.destroy_workspace("p", &k).await.unwrap();
        let fresh = h
            .manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        assert_eq!(fresh.status, WorkspaceStatus::Ready);
    }

    #[tokio::test]
    async fn a_deleted_workspace_refuses_new_work() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager.destroy_workspace("p", &k).await.unwrap();
        assert!(matches!(
            h.manager.connect(&k).await,
            Err(MenziError::Conflict(_))
        ));
        assert!(h.manager.prompt(&k, "ses_1", "hi").await.is_err());
        assert!(h
            .manager
            .terminal(&k, &TerminalRequest::new("ls"))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn concurrent_ensures_provision_one_container() {
        let h = Arc::new(harness());
        let user = UserId::new();
        let project = ProjectId::new();
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let manager = h.manager.clone();
            tasks.push(tokio::spawn(async move {
                manager.ensure_workspace("p", spec(user, project)).await
            }));
        }
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(
            h.driver
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| call.starts_with("provision:"))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn suspend_leaves_the_workspace_reusable() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        let suspended = h.manager.suspend_workspace(&k).await.unwrap();
        assert_eq!(suspended.status, WorkspaceStatus::Idle);
        assert!(h.manager.prompt(&k, "ses_1", "still there").await.is_ok());
    }

    #[tokio::test]
    async fn archive_stops_the_container_and_can_be_revived() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        let archived = h.manager.archive_workspace(&k).await.unwrap();
        assert_eq!(archived.status, WorkspaceStatus::Archived);
        let revived = h.manager.start(&k).await.unwrap();
        assert_eq!(revived.status, WorkspaceStatus::Ready);
    }

    #[tokio::test]
    async fn an_autonomous_run_uses_the_same_operations() {
        let h = harness();
        let project = ProjectId::new();
        let service = UserId::new();
        let k = key(service, project);
        h.manager
            .ensure_workspace("autonomous", spec(service, project))
            .await
            .unwrap();
        let session = h
            .manager
            .open_session("autonomous", &k, Some("nightly"))
            .await
            .unwrap();
        let outcome = h
            .manager
            .prompt(&k, &session.id, "review the open diff")
            .await
            .unwrap();
        assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
        assert_eq!(
            h.store.get(&k).await.unwrap().unwrap().status,
            WorkspaceStatus::Running
        );
    }

    #[tokio::test]
    async fn every_lifecycle_step_is_audited() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.manager
            .ensure_workspace("alice", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager
            .ensure_workspace("alice", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        h.manager.destroy_workspace("alice", &k).await.unwrap();

        let entries = h.audit.entries.lock().unwrap().clone();
        assert!(entries
            .iter()
            .any(|e| e.starts_with("provision:created:alice:")));
        assert!(entries.iter().any(|e| e.starts_with("ensure:reuse:alice:")));
        assert!(entries.iter().any(|e| e.starts_with("destroy:alice:")));
    }

    #[tokio::test]
    async fn reconcile_adopts_a_provisioning_workspace_whose_container_exists() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        let mut workspace = Workspace {
            id: WorkspaceId::new(),
            user_id: k.user_id,
            project_id: k.project_id,
            name: k.instance_name(),
            status: WorkspaceStatus::Provisioning,
            instance_name: Some(k.instance_name()),
            endpoint: None,
            branch: None,
            commit_sha: None,
            last_error: Some("interrupted".to_string()),
            created_at: now(),
            updated_at: now(),
        };
        h.store.save(&workspace).await.unwrap();
        *h.driver.running.lock().unwrap() = true;

        let report = h.manager.reconcile().await.unwrap();
        assert!(report.adopted.contains(&k.instance_name()));
        workspace.status = WorkspaceStatus::Ready;
        let stored = h.store.get(&k).await.unwrap().unwrap();
        assert_eq!(stored.status, WorkspaceStatus::Ready);
        assert!(stored.last_error.is_none());
    }

    #[tokio::test]
    async fn reconcile_reports_a_container_that_vanished() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.store
            .save(&Workspace {
                id: WorkspaceId::new(),
                user_id: k.user_id,
                project_id: k.project_id,
                name: k.instance_name(),
                status: WorkspaceStatus::Ready,
                instance_name: Some(k.instance_name()),
                endpoint: Some("http://10.0.0.9:17999".to_string()),
                branch: None,
                commit_sha: None,
                last_error: None,
                created_at: now(),
                updated_at: now(),
            })
            .await
            .unwrap();
        *h.driver.running.lock().unwrap() = false;

        let report = h.manager.reconcile().await.unwrap();
        assert!(report.reaped.contains(&k.instance_name()));

        let stored = h.store.get(&k).await.unwrap().unwrap();
        assert_eq!(stored.status, WorkspaceStatus::Requested);
        assert_eq!(stored.endpoint, None);
        assert!(stored.last_error.is_some());
    }

    #[tokio::test]
    async fn a_recycled_workspace_re_provisions_on_the_next_call() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        h.store
            .save(&Workspace {
                id: WorkspaceId::new(),
                user_id: k.user_id,
                project_id: k.project_id,
                name: k.instance_name(),
                status: WorkspaceStatus::Ready,
                instance_name: Some(k.instance_name()),
                endpoint: Some("http://10.0.0.9:17999".to_string()),
                branch: None,
                commit_sha: None,
                last_error: None,
                created_at: now(),
                updated_at: now(),
            })
            .await
            .unwrap();
        *h.driver.running.lock().unwrap() = false;
        h.manager.reconcile().await.unwrap();

        let revived = h
            .manager
            .ensure_workspace("p", spec(k.user_id, k.project_id))
            .await
            .unwrap();
        assert_eq!(revived.status, WorkspaceStatus::Ready);
        assert_eq!(revived.endpoint.as_deref(), Some("http://10.0.0.9:17999"));
    }

    #[tokio::test]
    async fn reap_destroys_stuck_provisioning_containers() {
        let h = harness();
        let k = key(UserId::new(), ProjectId::new());
        let workspace = Workspace {
            id: WorkspaceId::new(),
            user_id: k.user_id,
            project_id: k.project_id,
            name: k.instance_name(),
            status: WorkspaceStatus::Provisioning,
            instance_name: Some(k.instance_name()),
            endpoint: None,
            branch: None,
            commit_sha: None,
            last_error: None,
            created_at: "2020-01-01T00:00:00Z".to_string(),
            updated_at: "2020-01-01T00:00:00Z".to_string(),
        };
        h.store.save(&workspace).await.unwrap();
        *h.driver.running.lock().unwrap() = true;

        let reaped = h.manager.reap(chrono::Duration::hours(1)).await.unwrap();
        assert_eq!(reaped, vec![k.instance_name()]);
        assert!(h
            .driver
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call == &format!("destroy:{}", k.instance_name())));
    }
}
