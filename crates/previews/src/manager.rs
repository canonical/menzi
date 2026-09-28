use async_trait::async_trait;
use chrono::Utc;
use menzi_common::ids::ProjectId;
use menzi_common::{MenziError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::types::{Preview, PreviewSpec, PreviewStatus};

#[async_trait]
pub trait PreviewDriver: Send + Sync {
    async fn copy_instance(&self, source: &str, dest: &str) -> Result<()>;
    async fn start_instance(&self, name: &str) -> Result<()>;
    async fn stop_instance(&self, name: &str, force: bool) -> Result<()>;
    async fn delete_instance(&self, name: &str) -> Result<()>;
}

pub struct LxdPreviewDriver {
    client: Arc<dyn menzi_lxd::LxdClient>,
}

impl LxdPreviewDriver {
    pub fn new(client: Arc<dyn menzi_lxd::LxdClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl PreviewDriver for LxdPreviewDriver {
    async fn copy_instance(&self, source: &str, dest: &str) -> Result<()> {
        self.client.copy_instance(source, dest).await.map(|_| ())
    }

    async fn start_instance(&self, name: &str) -> Result<()> {
        self.client.start_instance(name).await
    }

    async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
        self.client.stop_instance(name, force).await
    }

    async fn delete_instance(&self, name: &str) -> Result<()> {
        self.client.delete_instance(name).await
    }
}

pub struct PreviewManager {
    driver: Arc<dyn PreviewDriver>,
    registry: Mutex<HashMap<String, Preview>>,
}

impl PreviewManager {
    pub fn new(driver: Arc<dyn PreviewDriver>) -> Self {
        Self {
            driver,
            registry: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create(&self, spec: PreviewSpec) -> Result<Preview> {
        let id = Uuid::new_v4().to_string();
        let short = &id[..8];
        let instance = format!("prv-{short}");
        self.driver
            .copy_instance(&spec.source_instance, &instance)
            .await?;
        self.driver.start_instance(&instance).await?;
        let preview = Preview {
            id,
            project_id: spec.project_id,
            commit_sha: spec.commit_sha,
            branch: spec.branch,
            status: PreviewStatus::Ready,
            mode: spec.mode,
            url: format!("http://{instance}.preview.dev.local"),
            created_at: Utc::now().to_rfc3339(),
        };
        self.registry
            .lock()
            .expect("registry lock")
            .insert(preview.id.clone(), preview.clone());
        Ok(preview)
    }

    pub fn list(&self, project_id: ProjectId) -> Vec<Preview> {
        let registry = self.registry.lock().expect("registry lock");
        let mut previews: Vec<Preview> = registry
            .values()
            .filter(|preview| preview.project_id == project_id)
            .cloned()
            .collect();
        previews.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        previews
    }

    pub fn get(&self, id: &str) -> Option<Preview> {
        self.registry
            .lock()
            .expect("registry lock")
            .get(id)
            .cloned()
    }

    pub async fn reset(&self, id: &str) -> Result<Preview> {
        let preview = self
            .get(id)
            .ok_or_else(|| MenziError::NotFound(format!("preview {id}")))?;
        let instance = self.instance_for(&preview);
        self.driver.stop_instance(&instance, true).await?;
        self.driver.start_instance(&instance).await?;
        Ok(preview)
    }

    pub async fn restart(&self, id: &str) -> Result<Preview> {
        let preview = self
            .get(id)
            .ok_or_else(|| MenziError::NotFound(format!("preview {id}")))?;
        let instance = self.instance_for(&preview);
        self.driver.stop_instance(&instance, false).await?;
        self.driver.start_instance(&instance).await?;
        Ok(preview)
    }

    pub async fn teardown(&self, id: &str) -> Result<()> {
        let preview = self
            .get(id)
            .ok_or_else(|| MenziError::NotFound(format!("preview {id}")))?;
        let instance = self.instance_for(&preview);
        let _ = self.driver.stop_instance(&instance, true).await;
        self.driver.delete_instance(&instance).await?;
        self.registry.lock().expect("registry lock").remove(id);
        Ok(())
    }

    fn instance_for(&self, preview: &Preview) -> String {
        let short = &preview.id[..8];
        format!("prv-{short}")
    }
}

#[cfg(test)]
pub(crate) mod testbed {
    use super::*;
    use crate::types::PreviewSpec;

    pub(crate) fn spec() -> PreviewSpec {
        PreviewSpec {
            project_id: ProjectId::new(),
            source_instance: "mz-workspace".to_string(),
            commit_sha: Some("deadbeef".to_string()),
            branch: Some("main".to_string()),
            mode: "pinned".to_string(),
        }
    }

    #[derive(Clone, Default)]
    pub(crate) struct RecordingPreviewDriver {
        pub(crate) calls: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl PreviewDriver for RecordingPreviewDriver {
        async fn copy_instance(&self, source: &str, dest: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("copy:{source}:{dest}"));
            Ok(())
        }

        async fn start_instance(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("start:{name}"));
            Ok(())
        }

        async fn stop_instance(&self, name: &str, force: bool) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("stop:{name}:{force}"));
            Ok(())
        }

        async fn delete_instance(&self, name: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("delete:{name}"));
            Ok(())
        }
    }

    pub(crate) fn manager() -> (PreviewManager, RecordingPreviewDriver) {
        let driver = RecordingPreviewDriver::default();
        (PreviewManager::new(Arc::new(driver.clone())), driver)
    }
}

#[cfg(test)]
mod tests {
    use super::testbed::*;
    use super::*;

    #[tokio::test]
    async fn create_copies_and_starts_instance() {
        let (manager, driver) = manager();
        let preview = manager.create(spec()).await.unwrap();
        assert_eq!(preview.status, PreviewStatus::Ready);
        assert!(preview.url.starts_with("http://prv-"));
        let calls = driver.calls.lock().unwrap();
        assert_eq!(
            calls[0],
            "copy:mz-workspace:prv-".to_string() + &preview.id[..8]
        );
        assert_eq!(calls[1], "start:prv-".to_string() + &preview.id[..8]);
    }

    #[tokio::test]
    async fn list_filters_by_project() {
        let (manager, _driver) = manager();
        let first = manager.create(spec()).await.unwrap();
        let other = PreviewSpec {
            project_id: ProjectId::new(),
            source_instance: "mz-workspace".to_string(),
            commit_sha: None,
            branch: None,
            mode: "pinned".to_string(),
        };
        manager.create(other).await.unwrap();
        let project = manager.list(first.project_id);
        assert_eq!(project.len(), 1);
        assert_eq!(project[0].id, first.id);
    }

    #[tokio::test]
    async fn reset_cycles_instance() {
        let (manager, driver) = manager();
        let preview = manager.create(spec()).await.unwrap();
        manager.reset(&preview.id).await.unwrap();
        let calls = driver.calls.lock().unwrap();
        assert_eq!(calls[2], format!("stop:prv-{}:true", &preview.id[..8]));
        assert_eq!(calls[3], format!("start:prv-{}", &preview.id[..8]));
    }

    #[tokio::test]
    async fn restart_cycles_instance_soft() {
        let (manager, driver) = manager();
        let preview = manager.create(spec()).await.unwrap();
        manager.restart(&preview.id).await.unwrap();
        let calls = driver.calls.lock().unwrap();
        assert_eq!(calls[2], format!("stop:prv-{}:false", &preview.id[..8]));
        assert_eq!(calls[3], format!("start:prv-{}", &preview.id[..8]));
    }

    #[tokio::test]
    async fn teardown_deletes_and_removes() {
        let (manager, driver) = manager();
        let preview = manager.create(spec()).await.unwrap();
        manager.teardown(&preview.id).await.unwrap();
        assert!(manager.get(&preview.id).is_none());
        let calls = driver.calls.lock().unwrap();
        assert!(calls
            .iter()
            .any(|call| call == &format!("delete:prv-{}", &preview.id[..8])));
    }

    #[tokio::test]
    async fn missing_preview_is_not_found() {
        let (manager, _driver) = manager();
        assert!(manager.reset("nope").await.is_err());
        assert!(manager.restart("nope").await.is_err());
        assert!(manager.teardown("nope").await.is_err());
    }

    #[test]
    fn instance_for_is_preview_prefixed_short_id() {
        let (manager, _driver) = manager();
        let preview = Preview {
            id: "12345678-1111-1111-1111-111111111111".to_string(),
            project_id: ProjectId::new(),
            commit_sha: None,
            branch: None,
            status: PreviewStatus::Ready,
            mode: "pinned".to_string(),
            url: "http://prv-12345678.preview.dev.local".to_string(),
            created_at: "now".to_string(),
        };
        assert_eq!(manager.instance_for(&preview), "prv-12345678");
    }
}
