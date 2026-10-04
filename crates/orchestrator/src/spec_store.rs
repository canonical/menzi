use async_trait::async_trait;
use menzi_common::{MenziError, Result};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::EnvironmentSpec;

#[async_trait]
pub trait SpecStore: Send + Sync {
    async fn register(&self, name: &str, spec: &EnvironmentSpec) -> Result<()>;
    async fn names(&self) -> Result<Vec<String>>;
    async fn get(&self, name: &str) -> Result<Option<EnvironmentSpec>>;
}

#[derive(Default)]
pub struct InMemorySpecStore {
    specs: Mutex<HashMap<String, EnvironmentSpec>>,
}

impl InMemorySpecStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SpecStore for InMemorySpecStore {
    async fn register(&self, name: &str, spec: &EnvironmentSpec) -> Result<()> {
        self.specs
            .lock()
            .expect("spec lock")
            .insert(name.to_string(), spec.clone());
        Ok(())
    }

    async fn names(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = self
            .specs
            .lock()
            .expect("spec lock")
            .keys()
            .cloned()
            .collect();
        names.sort();
        Ok(names)
    }

    async fn get(&self, name: &str) -> Result<Option<EnvironmentSpec>> {
        Ok(self.specs.lock().expect("spec lock").get(name).cloned())
    }
}

pub struct PostgresSpecStore {
    pool: PgPool,
}

impl PostgresSpecStore {
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
impl SpecStore for PostgresSpecStore {
    async fn register(&self, name: &str, spec: &EnvironmentSpec) -> Result<()> {
        let payload = serde_json::to_value(spec)
            .map_err(|e| MenziError::Internal(anyhow::Error::msg(e.to_string())))?;
        sqlx::query(
            "INSERT INTO orchestrator_env_specs (name, spec)
             VALUES ($1, $2)
             ON CONFLICT (name) DO UPDATE SET
                spec = EXCLUDED.spec,
                updated_at = now()",
        )
        .bind(name)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }

    async fn names(&self) -> Result<Vec<String>> {
        let mut names: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM orchestrator_env_specs ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?
        .into_iter()
        .map(|row| row.0)
        .collect();
        names.sort();
        Ok(names)
    }

    async fn get(&self, name: &str) -> Result<Option<EnvironmentSpec>> {
        let row = sqlx::query_as::<_, (serde_json::Value,)>(
            "SELECT spec FROM orchestrator_env_specs WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        row.map(|row| {
            serde_json::from_value(row.0)
                .map_err(|e| MenziError::Internal(anyhow::Error::msg(e.to_string())))
        })
        .transpose()
    }
}
