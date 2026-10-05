use sqlx::postgres::{PgPool, PgPoolOptions};

use menzi_common::Result;

#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20_u32)
            .connect(database_url)
            .await
            .map_err(|e| menzi_common::MenziError::Database(e.to_string()))?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| menzi_common::MenziError::Database(e.to_string()))?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| menzi_common::MenziError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn database_connect_fails_with_invalid_url() {
        let result = Database::connect("postgres://invalid:5432/nonexistent").await;
        assert!(result.is_err());
    }
}
