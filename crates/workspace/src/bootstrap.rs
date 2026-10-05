use async_trait::async_trait;
use menzi_common::ids::{ProjectId, UserId};
use menzi_common::Result;

#[derive(Debug, Clone, Default)]
pub struct DevelopmentScriptImport {
    pub name: String,
    pub slug: String,
    pub relative_path: String,
    pub body: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceBootstrapData {
    pub repository_url: Option<String>,
    pub ssh_private_key: Option<String>,
    pub ssh_passphrase: Option<String>,
}

#[async_trait]
pub trait WorkspaceBootstrapStore: Send + Sync {
    async fn load(&self, user_id: UserId, project_id: ProjectId) -> Result<WorkspaceBootstrapData>;
    async fn has_scripts(&self, project_id: ProjectId) -> Result<bool>;
    async fn import_scripts(
        &self,
        user_id: UserId,
        project_id: ProjectId,
        scripts: Vec<DevelopmentScriptImport>,
    ) -> Result<()>;
}

#[derive(Default)]
pub struct NoopWorkspaceBootstrapStore;

#[async_trait]
impl WorkspaceBootstrapStore for NoopWorkspaceBootstrapStore {
    async fn load(
        &self,
        _user_id: UserId,
        _project_id: ProjectId,
    ) -> Result<WorkspaceBootstrapData> {
        Ok(WorkspaceBootstrapData::default())
    }

    async fn has_scripts(&self, _project_id: ProjectId) -> Result<bool> {
        Ok(true)
    }

    async fn import_scripts(
        &self,
        _user_id: UserId,
        _project_id: ProjectId,
        _scripts: Vec<DevelopmentScriptImport>,
    ) -> Result<()> {
        Ok(())
    }
}
