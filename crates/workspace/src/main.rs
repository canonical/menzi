use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use menzi_common::{MenziError, Result};
use menzi_workspace::bootstrap::{DevelopmentScriptImport, WorkspaceBootstrapData, WorkspaceBootstrapStore};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::info;
use uuid::Uuid;

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
    bootstrap: Arc<dyn WorkspaceBootstrapStore>,
}

#[derive(Clone)]
struct SshKeyCipher {
    key: [u8; 32],
}

impl SshKeyCipher {
    fn from_env() -> Self {
        let secret = std::env::var("MENZI_SSH_KEY_ENCRYPTION_KEY")
            .unwrap_or_else(|_| "menzi-dev-ssh-key-encryption-key".to_string());
        let digest = Sha256::digest(secret.as_bytes());
        let mut key = [0u8; 32];
        key.copy_from_slice(&digest[..32]);
        Self { key }
    }

    fn decrypt(&self, payload: &str) -> Result<String> {
        use aes_gcm::aead::Aead;
        use aes_gcm::aead::KeyInit;
        use aes_gcm::{Aes256Gcm, Key, Nonce};

        let decoded = BASE64
            .decode(payload)
            .map_err(|error| MenziError::Validation(format!("invalid credential payload: {error}")))?;
        if decoded.len() < 13 {
            return Err(MenziError::Validation(
                "invalid credential payload length".to_string(),
            ));
        }
        let (nonce, ciphertext) = decoded.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|error| MenziError::Validation(format!("credential decrypt failed: {error}")))?;
        String::from_utf8(plaintext)
            .map_err(|error| MenziError::Validation(format!("credential is not utf8: {error}")))
    }
}

struct DbBootstrapStore {
    pool: PgPool,
    cipher: SshKeyCipher,
}

impl DbBootstrapStore {
    fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cipher: SshKeyCipher::from_env(),
        }
    }
}

#[async_trait]
impl WorkspaceBootstrapStore for DbBootstrapStore {
    async fn load(
        &self,
        user_id: menzi_common::ids::UserId,
        project_id: menzi_common::ids::ProjectId,
    ) -> Result<WorkspaceBootstrapData> {
        let repository_url: Option<String> = sqlx::query_scalar(
            "SELECT repository_url FROM projects WHERE id = $1::uuid",
        )
        .bind(project_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?
        .flatten();
        let key_row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT private_key_encrypted, passphrase_encrypted
             FROM user_ssh_keys
             WHERE user_id = $1::uuid
             ORDER BY is_default DESC, created_at DESC
             LIMIT 1",
        )
        .bind(user_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        let (ssh_private_key, ssh_passphrase) = match key_row {
            Some((private, passphrase)) => {
                let decrypted = self.cipher.decrypt(&private).ok();
                let decrypted_passphrase = match passphrase {
                    Some(payload) => self.cipher.decrypt(&payload).ok(),
                    None => None,
                };
                (decrypted, decrypted_passphrase)
            }
            None => (None, None),
        };
        Ok(WorkspaceBootstrapData {
            repository_url,
            ssh_private_key,
            ssh_passphrase,
        })
    }

    async fn has_scripts(&self, project_id: menzi_common::ids::ProjectId) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM project_development_scripts WHERE project_id = $1::uuid",
        )
        .bind(project_id.as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| MenziError::Database(e.to_string()))?;
        Ok(count > 0)
    }

    async fn import_scripts(
        &self,
        user_id: menzi_common::ids::UserId,
        project_id: menzi_common::ids::ProjectId,
        scripts: Vec<DevelopmentScriptImport>,
    ) -> Result<()> {
        for script in scripts {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO project_development_scripts
                    (id, project_id, name, slug, relative_path, body, source, created_by)
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, 'imported', $7::uuid)
                 ON CONFLICT (project_id, slug) DO NOTHING",
            )
            .bind(id)
            .bind(project_id.as_uuid())
            .bind(script.name)
            .bind(script.slug)
            .bind(script.relative_path)
            .bind(script.body)
            .bind(user_id.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(|e| MenziError::Database(e.to_string()))?;
        }
        Ok(())
    }
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
                bootstrap: Arc::new(DbBootstrapStore::new(pool.clone())),
            }
        }
        None => Wiring {
            store: Arc::new(menzi_workspace::InMemoryWorkspaceStore::new()),
            sessions: Arc::new(menzi_workspace::registry::registry_noop()),
            authorizer: Arc::new(menzi_workspace::PermissiveAuthorizer),
            audit: None,
            reconcile_gate: Arc::new(FreeReconcileGate),
            bootstrap: Arc::new(menzi_workspace::NoopWorkspaceBootstrapStore),
        },
    };

    let mut manager = menzi_workspace::WorkspaceManager::new(
        driver,
        gateway,
        wiring.store,
        source_instance.clone(),
    )
    .with_sessions(wiring.sessions);
    manager = manager.with_bootstrap(wiring.bootstrap);
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
