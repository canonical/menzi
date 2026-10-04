use async_trait::async_trait;
use menzi_common::ids::{ProjectId, UserId, WorkspaceId};
use menzi_common::{MenziError, Result};
use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::store::WorkspaceStore;
use crate::types::{Workspace, WorkspaceKey, WorkspaceStatus};

struct Row0 {
    id: WorkspaceId,
    user_id: UserId,
    project_id: ProjectId,
    name: String,
    status: WorkspaceStatus,
    lxd_instance_name: Option<String>,
    endpoint: Option<String>,
    branch: Option<String>,
    commit_sha: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
    opencode_username: Option<String>,
    opencode_password_encrypted: Option<String>,
}

fn map(row: &sqlx::postgres::PgRow) -> Result<Row0> {
    let status: String = row
        .try_get("status")
        .map_err(|e| MenziError::Database(e.to_string()))?;
    let status: WorkspaceStatus = serde_json::from_value(serde_json::Value::String(status.clone()))
        .map_err(|_| MenziError::Database(format!("unknown workspace status {status}")))?;
    Ok(Row0 {
        id: row
            .try_get::<uuid::Uuid, _>("id")
            .map_err(|e| MenziError::Database(e.to_string()))
            .map(WorkspaceId::from_uuid)?,
        user_id: row
            .try_get::<uuid::Uuid, _>("user_id")
            .map_err(|e| MenziError::Database(e.to_string()))
            .map(UserId::from_uuid)?,
        project_id: row
            .try_get::<uuid::Uuid, _>("project_id")
            .map_err(|e| MenziError::Database(e.to_string()))
            .map(ProjectId::from_uuid)?,
        name: row
            .try_get("name")
            .map_err(|e| MenziError::Database(e.to_string()))?,
        status,
        lxd_instance_name: row.try_get("lxd_instance_name").ok(),
        endpoint: row
            .try_get::<serde_json::Value, _>("metadata")
            .ok()
            .and_then(|value| {
                value
                    .get("opencode_endpoint")
                    .and_then(|endpoint| endpoint.as_str())
                    .map(str::to_string)
            }),
        branch: row.try_get("branch").ok(),
        commit_sha: row.try_get("commit_sha").ok(),
        last_error: row
            .try_get::<serde_json::Value, _>("metadata")
            .ok()
            .and_then(|value| {
                value
                    .get("last_error")
                    .and_then(|error| error.as_str())
                    .map(str::to_string)
            }),
        created_at: row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .map_err(|e| MenziError::Database(e.to_string()))?
            .to_rfc3339(),
        updated_at: row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
            .map_err(|e| MenziError::Database(e.to_string()))?
            .to_rfc3339(),
        opencode_username: row
            .try_get::<serde_json::Value, _>("metadata")
            .ok()
            .and_then(|value| {
                value
                    .get("opencode_username")
                    .and_then(|username| username.as_str())
                    .map(str::to_string)
            }),
        opencode_password_encrypted: row
            .try_get::<serde_json::Value, _>("metadata")
            .ok()
            .and_then(|value| {
                value
                    .get("opencode_password_encrypted")
                    .and_then(|password| password.as_str())
                    .map(str::to_string)
            }),
    })
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

fn optional(row: Option<sqlx::postgres::PgRow>) -> Result<Option<Workspace>> {
    match row.as_ref() {
        Some(row) => map(row).map(|row| Some(into_workspace(row))),
        None => Ok(None),
    }
}

fn into_workspace(row: Row0) -> Workspace {
    Workspace {
        id: row.id,
        user_id: row.user_id,
        project_id: row.project_id,
        name: row.name,
        status: row.status,
        instance_name: row.lxd_instance_name,
        endpoint: row.endpoint,
        branch: row.branch,
        commit_sha: row.commit_sha,
        last_error: row.last_error,
        created_at: row.created_at,
        updated_at: row.updated_at,
        opencode_username: row.opencode_username,
        opencode_password_encrypted: row.opencode_password_encrypted,
    }
}

pub struct PostgresWorkspaceStore {
    pool: PgPool,
}

impl PostgresWorkspaceStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("../db/migrations")
            .run(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))
    }
}

#[async_trait]
impl WorkspaceStore for PostgresWorkspaceStore {
    async fn get(&self, key: &WorkspaceKey) -> Result<Option<Workspace>> {
        let row = sqlx::query(
            "SELECT * FROM workspaces
             WHERE project_id = $1 AND user_id = $2 AND deleted_at IS NULL",
        )
        .bind(key.project_id.as_uuid())
        .bind(key.user_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        optional(row)
    }

    async fn get_by_id(&self, id: WorkspaceId) -> Result<Option<Workspace>> {
        let row = sqlx::query("SELECT * FROM workspaces WHERE id = $1 AND deleted_at IS NULL")
            .bind(id.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))?;
        optional(row)
    }

    async fn save(&self, workspace: &Workspace) -> Result<()> {
        let metadata = serde_json::json!({
            "opencode_endpoint": workspace.endpoint,
            "last_error": workspace.last_error,
            "opencode_username": workspace.opencode_username,
            "opencode_password_encrypted": workspace.opencode_password_encrypted,
        });
        let status = workspace.status.to_string();
        let deleted = status == "deleted";

        let updated = sqlx::query(
            "UPDATE workspaces SET
                name = $3,
                status = $4,
                lxd_instance_name = $5,
                branch = $6,
                commit_sha = $7,
                metadata = $8,
                updated_at = now(),
                deleted_at = CASE WHEN $9 THEN now() ELSE NULL END
             WHERE project_id = $1 AND user_id = $2 AND deleted_at IS NULL",
        )
        .bind(workspace.project_id.as_uuid())
        .bind(workspace.user_id.as_uuid())
        .bind(&workspace.name)
        .bind(&status)
        .bind(&workspace.instance_name)
        .bind(&workspace.branch)
        .bind(&workspace.commit_sha)
        .bind(metadata.clone())
        .bind(deleted)
        .execute(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        if updated.rows_affected() > 0 {
            return Ok(());
        }

        let insert = sqlx::query(
            "INSERT INTO workspaces
                (id, project_id, user_id, name, status, lxd_instance_name,
                 branch, commit_sha, metadata, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                 CASE WHEN $10 THEN now() ELSE NULL END)",
        )
        .bind(workspace.id.as_uuid())
        .bind(workspace.project_id.as_uuid())
        .bind(workspace.user_id.as_uuid())
        .bind(&workspace.name)
        .bind(&status)
        .bind(&workspace.instance_name)
        .bind(&workspace.branch)
        .bind(&workspace.commit_sha)
        .bind(metadata.clone())
        .bind(deleted)
        .execute(&self.pool)
        .await;

        match insert {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error) => {
                let retry = sqlx::query(
                    "UPDATE workspaces SET
                        name = $3,
                        status = $4,
                        lxd_instance_name = $5,
                        branch = $6,
                        commit_sha = $7,
                        metadata = $8,
                        updated_at = now(),
                        deleted_at = CASE WHEN $9 THEN now() ELSE NULL END
                     WHERE project_id = $1 AND user_id = $2 AND deleted_at IS NULL",
                )
                .bind(workspace.project_id.as_uuid())
                .bind(workspace.user_id.as_uuid())
                .bind(&workspace.name)
                .bind(&status)
                .bind(&workspace.instance_name)
                .bind(&workspace.branch)
                .bind(&workspace.commit_sha)
                .bind(metadata.clone())
                .bind(deleted)
                .execute(&self.pool)
                .await
                .map_err(|e| MenziError::Database(e.to_string()))?;
                if retry.rows_affected() == 0 {
                    return Err(MenziError::Database(format!(
                        "workspace {} is taken by another record",
                        workspace.id
                    )));
                }
                Ok(())
            }
            Err(error) => Err(MenziError::Database(error.to_string())),
        }
    }

    async fn list_for_project(&self, project_id: ProjectId) -> Result<Vec<Workspace>> {
        let rows = sqlx::query(
            "SELECT * FROM workspaces
             WHERE project_id = $1 AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(project_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        rows.iter()
            .map(map)
            .map(|row| row.map(into_workspace))
            .collect()
    }

    async fn list_stale(
        &self,
        statuses: &[WorkspaceStatus],
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Workspace>> {
        let names: Vec<String> = statuses.iter().map(|status| status.to_string()).collect();
        let rows = sqlx::query(
            "SELECT * FROM workspaces
             WHERE status = ANY($1) AND deleted_at IS NULL AND updated_at < $2
             ORDER BY updated_at ASC",
        )
        .bind(&names)
        .bind(older_than)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        rows.iter()
            .map(map)
            .map(|row| row.map(into_workspace))
            .collect()
    }

    async fn claim(&self, key: &WorkspaceKey) -> Result<Option<Workspace>> {
        sqlx::query(
            "UPDATE workspaces SET updated_at = now()
             WHERE project_id = $1 AND user_id = $2 AND deleted_at IS NULL
             RETURNING *",
        )
        .bind(key.project_id.as_uuid())
        .bind(key.user_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))
        .map(optional)?
    }
}
