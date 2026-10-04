use async_trait::async_trait;
use menzi_common::ids::{ProjectId, WorkspaceId};
use menzi_common::Result;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::{Workspace, WorkspaceKey};

#[async_trait]
pub trait WorkspaceStore: Send + Sync {
    async fn get(&self, key: &WorkspaceKey) -> Result<Option<Workspace>>;
    async fn get_by_id(&self, id: WorkspaceId) -> Result<Option<Workspace>>;
    async fn save(&self, workspace: &Workspace) -> Result<()>;
    async fn list_for_project(&self, project_id: ProjectId) -> Result<Vec<Workspace>>;
    async fn list_stale(
        &self,
        statuses: &[crate::types::WorkspaceStatus],
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Workspace>>;
    async fn claim(&self, key: &WorkspaceKey) -> Result<Option<Workspace>>;
}

#[derive(Default)]
pub struct InMemoryWorkspaceStore {
    entries: Mutex<HashMap<WorkspaceKey, Workspace>>,
}

impl InMemoryWorkspaceStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkspaceStore for InMemoryWorkspaceStore {
    async fn get(&self, key: &WorkspaceKey) -> Result<Option<Workspace>> {
        Ok(self
            .entries
            .lock()
            .expect("workspace store lock")
            .get(key)
            .cloned())
    }

    async fn get_by_id(&self, id: WorkspaceId) -> Result<Option<Workspace>> {
        Ok(self
            .entries
            .lock()
            .expect("workspace store lock")
            .values()
            .find(|workspace| workspace.id == id)
            .cloned())
    }

    async fn save(&self, workspace: &Workspace) -> Result<()> {
        let key = WorkspaceKey::new(workspace.user_id, workspace.project_id);
        self.entries
            .lock()
            .expect("workspace store lock")
            .insert(key, workspace.clone());
        Ok(())
    }

    async fn list_for_project(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
        let entries = self.entries.lock().expect("workspace store lock");
        let mut found: Vec<Workspace> = entries
            .values()
            .filter(|workspace| workspace.project_id == project_id)
            .cloned()
            .collect();
        found.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(found)
    }

    async fn list_stale(
        &self,
        statuses: &[crate::types::WorkspaceStatus],
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Workspace>> {
        let entries = self.entries.lock().expect("workspace store lock");
        Ok(entries
            .values()
            .filter(|workspace| statuses.contains(&workspace.status))
            .filter(|workspace| {
                chrono::DateTime::parse_from_rfc3339(&workspace.updated_at)
                    .map(|updated| updated < older_than)
                    .unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    async fn claim(&self, key: &WorkspaceKey) -> Result<Option<Workspace>> {
        self.get(key).await
    }
}

#[cfg(test)]
pub(crate) mod testbed {
    use super::*;
    use crate::types::{WorkspaceSpec, WorkspaceStatus};
    use menzi_common::ids::UserId;
    use menzi_common::MenziError;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    pub struct RecordingStore {
        pub saved: Mutex<Vec<Workspace>>,
        pub fail_save: AtomicBool,
        pub shared: Option<Arc<InMemoryWorkspaceStore>>,
    }

    impl Default for RecordingStore {
        fn default() -> Self {
            Self {
                saved: Mutex::new(Vec::new()),
                fail_save: AtomicBool::new(false),
                shared: None,
            }
        }
    }

    impl RecordingStore {
        pub fn backed_by(inner: Arc<InMemoryWorkspaceStore>) -> Self {
            Self {
                saved: Mutex::new(Vec::new()),
                fail_save: AtomicBool::new(false),
                shared: Some(inner),
            }
        }
    }

    #[async_trait]
    impl WorkspaceStore for RecordingStore {
        async fn get(&self, key: &WorkspaceKey) -> Result<Option<Workspace>> {
            match &self.shared {
                Some(inner) => inner.get(key).await,
                None => Ok(self
                    .saved
                    .lock()
                    .unwrap()
                    .iter()
                    .rev()
                    .find(|workspace| {
                        WorkspaceKey::new(workspace.user_id, workspace.project_id) == *key
                    })
                    .cloned()),
            }
        }

        async fn get_by_id(&self, id: WorkspaceId) -> Result<Option<Workspace>> {
            match &self.shared {
                Some(inner) => inner.get_by_id(id).await,
                None => Ok(self
                    .saved
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .cloned()),
            }
        }

        async fn save(&self, workspace: &Workspace) -> Result<()> {
            if self.fail_save.load(Ordering::SeqCst) {
                return Err(MenziError::Database("save refused".to_string()));
            }
            self.saved.lock().unwrap().push(workspace.clone());
            match &self.shared {
                Some(inner) => inner.save(workspace).await,
                None => Ok(()),
            }
        }

        async fn list_for_project(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
            match &self.shared {
                Some(inner) => inner.list_for_project(project_id).await,
                None => {
                    let mut found: Vec<Workspace> = self
                        .saved
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|workspace| workspace.project_id == project_id)
                        .cloned()
                        .collect();
                    found.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                    Ok(found)
                }
            }
        }

        async fn list_stale(
            &self,
            statuses: &[WorkspaceStatus],
            older_than: chrono::DateTime<chrono::Utc>,
        ) -> Result<Vec<Workspace>> {
            let found = self
                .saved
                .lock()
                .unwrap()
                .iter()
                .filter(|workspace| statuses.contains(&workspace.status))
                .filter(|workspace| {
                    chrono::DateTime::parse_from_rfc3339(&workspace.updated_at)
                        .map(|updated| updated < older_than)
                        .unwrap_or(true)
                })
                .cloned()
                .collect::<Vec<Workspace>>();
            Ok(found)
        }

        async fn claim(&self, key: &WorkspaceKey) -> Result<Option<Workspace>> {
            self.get(key).await
        }
    }

    pub fn spec(user: UserId, project: ProjectId) -> WorkspaceSpec {
        WorkspaceSpec {
            user_id: user,
            project_id: project,
            source_instance: "mz-workspace".to_string(),
            name: None,
            branch: Some("main".to_string()),
            commit_sha: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testbed::{spec, RecordingStore};
    use super::*;
    use crate::types::WorkspaceStatus;
    use menzi_common::ids::UserId;
    use std::sync::Arc;

    fn workspace(user: UserId, project: ProjectId) -> Workspace {
        Workspace {
            id: WorkspaceId::new(),
            user_id: user,
            project_id: project,
            name: "workspace".to_string(),
            status: WorkspaceStatus::Ready,
            instance_name: Some("wsp-a-b".to_string()),
            endpoint: None,
            branch: None,
            commit_sha: None,
            last_error: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            opencode_username: None,
            opencode_password_encrypted: None,
        }
    }

    #[tokio::test]
    async fn saves_and_reads_back_by_key() {
        let store = InMemoryWorkspaceStore::new();
        let user = UserId::new();
        let project = ProjectId::new();
        assert!(store
            .get(&WorkspaceKey::new(user, project))
            .await
            .unwrap()
            .is_none());
        let saved = workspace(user, project);
        store.save(&saved).await.unwrap();
        let found = store.get(&WorkspaceKey::new(user, project)).await.unwrap();
        assert_eq!(found.map(|entry| entry.id), Some(saved.id));
    }

    #[tokio::test]
    async fn reads_back_by_id() {
        let store = InMemoryWorkspaceStore::new();
        let saved = workspace(UserId::new(), ProjectId::new());
        let id = saved.id;
        store.save(&saved).await.unwrap();
        assert_eq!(store.get_by_id(id).await.unwrap().map(|w| w.id), Some(id));
        assert!(store.get_by_id(WorkspaceId::new()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn save_overwrites_the_same_key() {
        let store = InMemoryWorkspaceStore::new();
        let user = UserId::new();
        let project = ProjectId::new();
        let mut first = workspace(user, project);
        first.status = WorkspaceStatus::Provisioning;
        store.save(&first).await.unwrap();
        let mut second = workspace(user, project);
        second.status = WorkspaceStatus::Ready;
        store.save(&second).await.unwrap();
        let found = store
            .get(&WorkspaceKey::new(user, project))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.status, WorkspaceStatus::Ready);
        assert_eq!(found.id, second.id);
    }

    #[tokio::test]
    async fn lists_only_the_requested_project_newest_first() {
        let store = InMemoryWorkspaceStore::new();
        let project = ProjectId::new();
        let mut older = workspace(UserId::new(), project);
        older.created_at = "2026-01-01T00:00:00Z".to_string();
        let mut newer = workspace(UserId::new(), project);
        newer.created_at = "2026-02-01T00:00:00Z".to_string();
        store.save(&older).await.unwrap();
        store.save(&newer).await.unwrap();
        store
            .save(&workspace(UserId::new(), ProjectId::new()))
            .await
            .unwrap();

        let listed = store.list_for_project(project).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].created_at, "2026-02-01T00:00:00Z");
    }

    #[tokio::test]
    async fn lists_stale_entries_for_reaping() {
        let store = InMemoryWorkspaceStore::new();
        let mut old = workspace(UserId::new(), ProjectId::new());
        old.status = WorkspaceStatus::Provisioning;
        old.updated_at = "2020-01-01T00:00:00Z".to_string();
        store.save(&old).await.unwrap();
        let mut fresh = workspace(UserId::new(), ProjectId::new());
        fresh.status = WorkspaceStatus::Provisioning;
        fresh.updated_at = chrono::Utc::now().to_rfc3339();
        store.save(&fresh).await.unwrap();

        let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
        let stale = store
            .list_stale(&[WorkspaceStatus::Provisioning], cutoff)
            .await
            .unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].updated_at, "2020-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn a_recording_store_keeps_what_it_saved() {
        let store = RecordingStore::default();
        let user = UserId::new();
        let project = ProjectId::new();
        let spec = spec(user, project);
        let saved = workspace(user, project);
        store.save(&saved).await.unwrap();
        assert_eq!(
            store
                .get(&WorkspaceKey::new(user, project))
                .await
                .unwrap()
                .map(|entry| entry.id),
            Some(saved.id)
        );
        assert_eq!(spec.source_instance, "mz-workspace");
    }

    #[tokio::test]
    async fn a_recording_store_can_refuse_a_save() {
        let store = RecordingStore::default();
        store
            .fail_save
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(store
            .save(&workspace(UserId::new(), ProjectId::new()))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_recording_store_can_share_a_backing_store() {
        let inner = Arc::new(InMemoryWorkspaceStore::new());
        let store = RecordingStore::backed_by(inner.clone());
        let user = UserId::new();
        let project = ProjectId::new();
        let saved = workspace(user, project);
        store.save(&saved).await.unwrap();
        assert_eq!(
            inner
                .get(&WorkspaceKey::new(user, project))
                .await
                .unwrap()
                .map(|entry| entry.id),
            Some(saved.id)
        );
    }
}
