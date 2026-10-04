use async_trait::async_trait;
use menzi_common::ids::ProjectId;
use menzi_common::{MenziError, Result};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::Preview;
use crate::types::PreviewStatus;

#[async_trait]
pub trait PreviewStore: Send + Sync {
    async fn save(&self, preview: &Preview, source_instance: &str) -> Result<()>;
    async fn list(&self, project_id: ProjectId) -> Result<Vec<Preview>>;
    async fn get(&self, id: &str) -> Result<Option<Preview>>;
    async fn delete(&self, id: &str) -> Result<()>;
}

#[derive(Default)]
pub struct InMemoryPreviewStore {
    entries: Mutex<HashMap<String, Preview>>,
}

impl InMemoryPreviewStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PreviewStore for InMemoryPreviewStore {
    async fn save(&self, preview: &Preview, _source_instance: &str) -> Result<()> {
        self.entries
            .lock()
            .expect("preview store lock")
            .insert(preview.id.clone(), preview.clone());
        Ok(())
    }

    async fn list(&self, project_id: ProjectId) -> Result<Vec<Preview>> {
        let mut previews: Vec<Preview> = self
            .entries
            .lock()
            .expect("preview store lock")
            .values()
            .filter(|preview| preview.project_id == project_id)
            .cloned()
            .collect();
        previews.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(previews)
    }

    async fn get(&self, id: &str) -> Result<Option<Preview>> {
        Ok(self
            .entries
            .lock()
            .expect("preview store lock")
            .get(id)
            .cloned())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.entries.lock().expect("preview store lock").remove(id);
        Ok(())
    }
}

pub struct PostgresPreviewStore {
    pool: PgPool,
}

impl PostgresPreviewStore {
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
impl PreviewStore for PostgresPreviewStore {
    async fn save(&self, preview: &Preview, source_instance: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO preview_registry
                (id, project_id, commit_sha, branch, status, mode, url, source_instance)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (id) DO UPDATE SET
                commit_sha = EXCLUDED.commit_sha,
                branch = EXCLUDED.branch,
                status = EXCLUDED.status,
                mode = EXCLUDED.mode,
                url = EXCLUDED.url,
                source_instance = EXCLUDED.source_instance,
                deleted_at = NULL,
                updated_at = now()",
        )
        .bind(&preview.id)
        .bind(preview.project_id.as_uuid())
        .bind(&preview.commit_sha)
        .bind(&preview.branch)
        .bind(status_name(preview.status))
        .bind(&preview.mode)
        .bind(&preview.url)
        .bind(source_instance)
        .execute(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }

    async fn list(&self, project_id: ProjectId) -> Result<Vec<Preview>> {
        let rows = sqlx::query_as::<_, (String, uuid::Uuid, Option<String>, Option<String>, String, String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, project_id, commit_sha, branch, status, mode, url, created_at
             FROM preview_registry
             WHERE project_id = $1 AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(project_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;

        rows.into_iter().map(row_to_preview).collect()
    }

    async fn get(&self, id: &str) -> Result<Option<Preview>> {
        let row = sqlx::query_as::<_, (String, uuid::Uuid, Option<String>, Option<String>, String, String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, project_id, commit_sha, branch, status, mode, url, created_at
             FROM preview_registry
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        row.map(row_to_preview).transpose()
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE preview_registry SET deleted_at = now(), updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }
}

fn row_to_preview(
    row: (
        String,
        uuid::Uuid,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        chrono::DateTime<chrono::Utc>,
    ),
) -> Result<Preview> {
    let status = serde_json::from_value(serde_json::Value::String(row.4))
        .map_err(|_| MenziError::Database("invalid preview status".to_string()))?;
    Ok(Preview {
        id: row.0,
        project_id: ProjectId::from_uuid(row.1),
        commit_sha: row.2,
        branch: row.3,
        status,
        mode: row.5,
        url: row.6,
        created_at: row.7.to_rfc3339(),
    })
}

fn status_name(status: PreviewStatus) -> &'static str {
    match status {
        PreviewStatus::Pending => "pending",
        PreviewStatus::Ready => "ready",
        PreviewStatus::Stopped => "stopped",
        PreviewStatus::Failed => "failed",
    }
}
