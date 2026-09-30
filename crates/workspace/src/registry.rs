use async_trait::async_trait;
use menzi_common::ids::WorkspaceId;
use menzi_common::{MenziError, Result};
use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Interactive,
    Autonomous,
}

impl std::fmt::Display for SessionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SessionKind::Interactive => "interactive",
            SessionKind::Autonomous => "autonomous",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionBinding {
    pub opencode_session: String,
    pub workspace_id: WorkspaceId,
    pub user_id: menzi_common::ids::UserId,
    pub project_id: menzi_common::ids::ProjectId,
    pub endpoint: String,
    pub instance_name: Option<String>,
    pub kind: SessionKind,
    pub principal: String,
    pub created_at: String,
}

#[async_trait]
pub trait SessionRegistry: Send + Sync {
    async fn bind(&self, binding: &SessionBinding) -> Result<()>;
    async fn binding(&self, opencode_session: &str) -> Result<Option<SessionBinding>>;
    async fn bindings_for_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<SessionBinding>>;
}

pub struct NoopRegistry;

#[async_trait]
impl SessionRegistry for NoopRegistry {
    async fn bind(&self, _binding: &SessionBinding) -> Result<()> {
        Ok(())
    }

    async fn binding(&self, _opencode_session: &str) -> Result<Option<SessionBinding>> {
        Ok(None)
    }

    async fn bindings_for_workspace(
        &self,
        _workspace_id: WorkspaceId,
    ) -> Result<Vec<SessionBinding>> {
        Ok(Vec::new())
    }
}

pub fn registry_noop() -> NoopRegistry {
    NoopRegistry
}

pub struct PostgresSessionRegistry {
    pool: PgPool,
}

impl PostgresSessionRegistry {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn map_binding(row: &sqlx::postgres::PgRow) -> Result<SessionBinding> {
    let kind: String = row
        .try_get("kind")
        .map_err(|e| MenziError::Database(e.to_string()))?;
    let kind = serde_json::from_value(serde_json::Value::String(kind.clone()))
        .map_err(|_| MenziError::Database(format!("unknown session kind {kind}")))?;
    Ok(SessionBinding {
        opencode_session: row
            .try_get("opencode_session")
            .map_err(|e| MenziError::Database(e.to_string()))?,
        workspace_id: WorkspaceId::from_uuid(
            row.try_get::<uuid::Uuid, _>("workspace_id")
                .map_err(|e| MenziError::Database(e.to_string()))?,
        ),
        user_id: menzi_common::ids::UserId::from_uuid(
            row.try_get::<uuid::Uuid, _>("user_id")
                .map_err(|e| MenziError::Database(e.to_string()))?,
        ),
        project_id: menzi_common::ids::ProjectId::from_uuid(
            row.try_get::<uuid::Uuid, _>("project_id")
                .map_err(|e| MenziError::Database(e.to_string()))?,
        ),
        endpoint: row
            .try_get("endpoint")
            .map_err(|e| MenziError::Database(e.to_string()))?,
        instance_name: row.try_get("instance_name").ok(),
        kind,
        principal: row
            .try_get("principal")
            .map_err(|e| MenziError::Database(e.to_string()))?,
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .map_err(|e| MenziError::Database(e.to_string()))?
            .to_rfc3339(),
    })
}

#[async_trait]
impl SessionRegistry for PostgresSessionRegistry {
    async fn bind(&self, binding: &SessionBinding) -> Result<()> {
        sqlx::query(
            "INSERT INTO workspace_sessions
                (workspace_id, opencode_session, user_id, project_id, endpoint,
                 instance_name, kind, principal)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (opencode_session) DO UPDATE SET
                endpoint = EXCLUDED.endpoint,
                instance_name = EXCLUDED.instance_name,
                status = 'active'",
        )
        .bind(binding.workspace_id.as_uuid())
        .bind(&binding.opencode_session)
        .bind(binding.user_id.as_uuid())
        .bind(binding.project_id.as_uuid())
        .bind(&binding.endpoint)
        .bind(&binding.instance_name)
        .bind(binding.kind.to_string())
        .bind(&binding.principal)
        .execute(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }

    async fn binding(&self, opencode_session: &str) -> Result<Option<SessionBinding>> {
        let row = sqlx::query(
            "SELECT workspace_id, opencode_session, user_id, project_id, endpoint,
                    instance_name, kind, principal, created_at
             FROM workspace_sessions WHERE opencode_session = $1",
        )
        .bind(opencode_session)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        match row {
            Some(row) => Ok(Some(map_binding(&row)?)),
            None => Ok(None),
        }
    }

    async fn bindings_for_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<SessionBinding>> {
        let rows = sqlx::query(
            "SELECT workspace_id, opencode_session, user_id, project_id, endpoint,
                    instance_name, kind, principal, created_at
             FROM workspace_sessions WHERE workspace_id = $1 ORDER BY created_at DESC",
        )
        .bind(workspace_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        rows.iter().map(map_binding).collect()
    }
}

#[cfg(test)]
pub(crate) mod testbed {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct RecordingRegistry {
        pub bound: Mutex<Vec<SessionBinding>>,
        pub fail_bind: Mutex<bool>,
    }

    impl RecordingRegistry {
        pub fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait]
    impl SessionRegistry for RecordingRegistry {
        async fn bind(&self, binding: &SessionBinding) -> Result<()> {
            if *self.fail_bind.lock().expect("registry lock") {
                return Err(MenziError::Database("bind refused".to_string()));
            }
            self.bound
                .lock()
                .expect("registry lock")
                .push(binding.clone());
            Ok(())
        }

        async fn binding(&self, opencode_session: &str) -> Result<Option<SessionBinding>> {
            Ok(self
                .bound
                .lock()
                .expect("registry lock")
                .iter()
                .rev()
                .find(|entry| entry.opencode_session == opencode_session)
                .cloned())
        }

        async fn bindings_for_workspace(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<Vec<SessionBinding>> {
            Ok(self
                .bound
                .lock()
                .expect("registry lock")
                .iter()
                .filter(|entry| entry.workspace_id == workspace_id)
                .cloned()
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testbed::RecordingRegistry;
    use super::*;
    use menzi_common::ids::{ProjectId, UserId};

    fn binding(session: &str) -> SessionBinding {
        SessionBinding {
            opencode_session: session.to_string(),
            workspace_id: WorkspaceId::new(),
            user_id: UserId::new(),
            project_id: ProjectId::new(),
            endpoint: "http://10.0.0.1:17999".to_string(),
            instance_name: Some("wsp-a-b".to_string()),
            kind: SessionKind::Interactive,
            principal: "user-1".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn the_kind_is_named_the_way_the_database_stores_it() {
        assert_eq!(SessionKind::Interactive.to_string(), "interactive");
        assert_eq!(SessionKind::Autonomous.to_string(), "autonomous");
    }

    #[test]
    fn a_binding_serializes_for_the_proxy_to_read() {
        let json = serde_json::to_string(&binding("ses_1")).unwrap();
        assert!(json.contains("\"opencode_session\":\"ses_1\""));
        assert!(json.contains("\"endpoint\":\"http://10.0.0.1:17999\""));
        assert!(json.contains("\"kind\":\"interactive\""));
    }

    #[tokio::test]
    async fn a_binding_is_found_by_its_session() {
        let registry = RecordingRegistry::new();
        registry.bind(&binding("ses_1")).await.unwrap();
        let found = registry.binding("ses_1").await.unwrap().unwrap();
        assert_eq!(found.endpoint, "http://10.0.0.1:17999");
        assert!(registry.binding("ses_other").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn the_last_binding_for_a_session_wins() {
        let registry = RecordingRegistry::new();
        registry.bind(&binding("ses_1")).await.unwrap();
        let mut second = binding("ses_1");
        second.endpoint = "http://10.0.0.2:17999".to_string();
        registry.bind(&second).await.unwrap();
        let found = registry.binding("ses_1").await.unwrap().unwrap();
        assert_eq!(found.endpoint, "http://10.0.0.2:17999");
    }

    #[tokio::test]
    async fn bindings_are_listed_per_workspace() {
        let registry = RecordingRegistry::new();
        let mut in_workspace = binding("ses_1");
        in_workspace.workspace_id = WorkspaceId::new();
        let other = binding("ses_2");
        registry.bind(&in_workspace).await.unwrap();
        registry.bind(&other).await.unwrap();

        let listed = registry
            .bindings_for_workspace(in_workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].opencode_session, "ses_1");
    }

    #[tokio::test]
    async fn a_registry_can_refuse_a_binding() {
        let registry = RecordingRegistry::new();
        *registry.fail_bind.lock().unwrap() = true;
        assert!(registry.bind(&binding("ses_1")).await.is_err());
    }
}
