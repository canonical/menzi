use async_trait::async_trait;
use menzi_common::{MenziError, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::info;

use menzi_workspace::identity::{Authorizer, Principal};
use menzi_workspace::manager::WorkspaceAudit;
use menzi_workspace::registry::SessionRegistry;
use menzi_workspace::types::WorkspaceKey;

struct DbAuthorizer {
    pool: PgPool,
}

#[async_trait]
impl Authorizer for DbAuthorizer {
    async fn may_use_workspace(&self, principal: &Principal, key: &WorkspaceKey) -> Result<bool> {
        let user_id = match principal {
            Principal::User(user_id) => *user_id,
            Principal::Service(name) => {
                let allowed = std::env::var("MENZI_WORKSPACE_SERVICE").unwrap_or_default();
                return Ok(*name == allowed);
            }
        };
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM project_members WHERE project_id = $1 AND user_id = $2")
                .bind(key.project_id.as_uuid())
                .bind(user_id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(row.is_some())
    }
}

struct DbAudit {
    pool: PgPool,
}

impl WorkspaceAudit for DbAudit {
    fn record(&self, action: &str, principal: &str, key: &WorkspaceKey, detail: &str) {
        let pool = self.pool.clone();
        let action = action.to_string();
        let principal = principal.to_string();
        let detail = detail.to_string();
        let project_id = key.project_id;
        let instance_name = key.instance_name();
        tokio::spawn(async move {
            let result = sqlx::query(
                "INSERT INTO workspace_audit
                    (project_id, instance_name, action, principal, detail)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(project_id.as_uuid())
            .bind(&instance_name)
            .bind(&action)
            .bind(&principal)
            .bind(&detail)
            .execute(&pool)
            .await;
            if let Err(error) = result {
                tracing::warn!("workspace audit write failed: {error}");
            }
        });
    }
}

struct Wiring {
    store: Arc<dyn menzi_workspace::WorkspaceStore>,
    sessions: Arc<dyn SessionRegistry>,
    authorizer: Arc<dyn Authorizer>,
    audit: Option<Arc<DbAudit>>,
    reconcile_gate: Arc<dyn ReconcileGate>,
}

#[async_trait]
trait ReconcileGate: Send + Sync {
    async fn try_acquire(&self) -> Result<bool>;
    async fn release(&self) -> Result<()>;
}

struct FreeReconcileGate;

#[async_trait]
impl ReconcileGate for FreeReconcileGate {
    async fn try_acquire(&self) -> Result<bool> {
        Ok(true)
    }

    async fn release(&self) -> Result<()> {
        Ok(())
    }
}

struct PostgresReconcileGate {
    pool: PgPool,
    key: i64,
}

impl PostgresReconcileGate {
    fn new(pool: PgPool, key: i64) -> Self {
        Self { pool, key }
    }
}

#[async_trait]
impl ReconcileGate for PostgresReconcileGate {
    async fn try_acquire(&self) -> Result<bool> {
        let row: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock($1)")
            .bind(self.key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(row.0)
    }

    async fn release(&self) -> Result<()> {
        let _: (bool,) = sqlx::query_as("SELECT pg_advisory_unlock($1)")
            .bind(self.key)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(())
    }
}

async fn pool_from_env() -> Option<PgPool> {
    let url = std::env::var("MENZI_DATABASE_URL").ok()?;
    match PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&url)
        .await
    {
        Ok(pool) => Some(pool),
        Err(error) => {
            tracing::warn!("database unavailable, workspaces stay in memory: {error}");
            None
        }
    }
}

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let lxd = Arc::new(
        menzi_lxd::HttpLxdClient::connect(
            &config.lxd_url,
            "default",
            config.lxd_cert_path.as_deref(),
            config.lxd_key_path.as_deref(),
        )
        .expect("Failed to connect to LXD"),
    );

    let source_instance =
        std::env::var("MENZI_SOURCE_INSTANCE").unwrap_or_else(|_| "mz-workspace".to_string());
    let opencode_port = std::env::var("MENZI_OPENCODE_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(17999);
    let domain =
        std::env::var("MENZI_WORKSPACE_DOMAIN").unwrap_or_else(|_| "dev.local".to_string());
    let bind = std::env::var("MENZI_WORKSPACE_BIND").unwrap_or_else(|_| "0.0.0.0:8096".to_string());

    let driver = Arc::new(menzi_workspace::LxdWorkspaceDriver::with_endpoint(
        lxd,
        opencode_port,
        domain,
    ));
    let gateway = Arc::new(menzi_workspace::HttpOpencodeGateway::new());

    let pool = pool_from_env().await;
    let wiring = match pool {
        Some(pool) => {
            if let Err(error) = menzi_workspace::PostgresWorkspaceStore::new(pool.clone())
                .migrate()
                .await
            {
                tracing::warn!("workspace migration failed: {error}");
            }
            let lock_key = std::env::var("MENZI_WORKSPACE_RECONCILE_LOCK_KEY")
                .ok()
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(81096);
            Wiring {
                store: Arc::new(menzi_workspace::PostgresWorkspaceStore::new(pool.clone())),
                sessions: Arc::new(menzi_workspace::PostgresSessionRegistry::new(pool.clone())),
                authorizer: Arc::new(DbAuthorizer { pool: pool.clone() }),
                audit: Some(Arc::new(DbAudit { pool: pool.clone() })),
                reconcile_gate: Arc::new(PostgresReconcileGate::new(pool.clone(), lock_key)),
            }
        }
        None => Wiring {
            store: Arc::new(menzi_workspace::InMemoryWorkspaceStore::new()),
            sessions: Arc::new(menzi_workspace::registry::registry_noop()),
            authorizer: Arc::new(menzi_workspace::PermissiveAuthorizer),
            audit: None,
            reconcile_gate: Arc::new(FreeReconcileGate),
        },
    };

    let mut manager = menzi_workspace::WorkspaceManager::new(
        driver,
        gateway,
        wiring.store,
        source_instance.clone(),
    )
    .with_sessions(wiring.sessions);
    if let Some(audit) = wiring.audit {
        manager = manager.with_audit(audit);
    }
    let manager = Arc::new(manager);

    let state =
        menzi_workspace::api::WorkspaceApiState::new(manager.clone(), source_instance.clone())
            .with_authorizer(wiring.authorizer);
    let app = menzi_workspace::api::create_router(state);

    if let Some(seconds) = std::env::var("MENZI_WORKSPACE_RECONCILE_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        spawn_reconciler(manager.clone(), wiring.reconcile_gate, seconds);
    }

    info!("Listening on {bind}, provisioning from {source_instance}");

    let listener = TcpListener::bind(&bind).await.expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server failed");
}

fn spawn_reconciler(
    manager: Arc<menzi_workspace::WorkspaceManager>,
    gate: Arc<dyn ReconcileGate>,
    seconds: u64,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            let acquired = match gate.try_acquire().await {
                Ok(acquired) => acquired,
                Err(error) => {
                    tracing::warn!("workspace reconcile lock failed: {error}");
                    continue;
                }
            };
            if !acquired {
                continue;
            }
            match manager.reconcile().await {
                Ok(report) => {
                    if !report.adopted.is_empty()
                        || !report.released.is_empty()
                        || !report.reaped.is_empty()
                        || !report.failed.is_empty()
                    {
                        info!(
                            adopted = report.adopted.len(),
                            released = report.released.len(),
                            reaped = report.reaped.len(),
                            failed = report.failed.len(),
                            "workspace reconcile"
                        );
                    }
                }
                Err(error) => tracing::warn!("workspace reconcile failed: {error}"),
            }
            if let Err(error) = gate.release().await {
                tracing::warn!("workspace reconcile unlock failed: {error}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedGate {
        acquire: bool,
    }

    #[async_trait]
    impl ReconcileGate for FixedGate {
        async fn try_acquire(&self) -> Result<bool> {
            Ok(self.acquire)
        }

        async fn release(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn free_gate_allows_reconcile() {
        let gate = FreeReconcileGate;
        assert!(gate.try_acquire().await.unwrap());
        gate.release().await.unwrap();
    }

    #[tokio::test]
    async fn fixed_gate_can_block_reconcile() {
        let gate = FixedGate { acquire: false };
        assert!(!gate.try_acquire().await.unwrap());
    }
}
