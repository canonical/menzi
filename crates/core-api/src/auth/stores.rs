use async_trait::async_trait;
use chrono::{DateTime, Utc};
use menzi_auth::record::{IdentityLink, NewSession, OidcState, SessionRecord, UserRecord};
use menzi_auth::role_store::{PostgresRoleStore, RoleStore};
use menzi_auth::store::{AccountStore, OidcStateStore, SessionStore, StoreError, ThrottleStore};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct PostgresSessionStore {
    pub pool: PgPool,
}

impl PostgresSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const SESSION_COLUMNS: &str =
    "id, user_id, provider_id, created_at, expires_at, last_seen_at, user_agent, ip";

type SessionRow = (
    Uuid,
    Uuid,
    String,
    DateTime<Utc>,
    DateTime<Utc>,
    DateTime<Utc>,
    Option<String>,
    Option<String>,
);

fn session_of(row: SessionRow) -> SessionRecord {
    SessionRecord {
        id: row.0,
        user_id: row.1,
        provider_id: row.2,
        created_at: row.3,
        expires_at: row.4,
        last_seen_at: row.5,
        user_agent: row.6,
        ip: row.7,
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn create(&self, session: NewSession) -> Result<SessionRecord, StoreError> {
        let row: (Uuid, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as(
            "INSERT INTO auth_sessions
                (user_id, token_hash, csrf_hash, provider_id, expires_at, user_agent, ip)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, created_at, last_seen_at",
        )
        .bind(session.user_id)
        .bind(&session.token_hash)
        .bind(&session.csrf_hash)
        .bind(&session.provider_id)
        .bind(session.expires_at)
        .bind(&session.user_agent)
        .bind(&session.ip)
        .fetch_one(&self.pool)
        .await?;
        Ok(SessionRecord {
            id: row.0,
            user_id: session.user_id,
            provider_id: session.provider_id,
            created_at: row.1,
            expires_at: session.expires_at,
            last_seen_at: row.2,
            user_agent: session.user_agent,
            ip: session.ip,
        })
    }

    async fn find_active(&self, token_hash: &str) -> Result<Option<SessionRecord>, StoreError> {
        let row = sqlx::query_as::<_, SessionRow>(&format!(
            "SELECT {SESSION_COLUMNS} FROM auth_sessions
             WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()"
        ))
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(session_of))
    }

    async fn touch(&self, id: Uuid) -> Result<(), StoreError> {
        sqlx::query("UPDATE auth_sessions SET last_seen_at = now() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn revoke(&self, id: Uuid) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE auth_sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn revoke_all_for(&self, user_id: Uuid, keep: Option<Uuid>) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "UPDATE auth_sessions SET revoked_at = now()
             WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id <> $2)",
        )
        .bind(user_id)
        .bind(keep)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn list_for(&self, user_id: Uuid) -> Result<Vec<SessionRecord>, StoreError> {
        let rows = sqlx::query_as::<_, SessionRow>(&format!(
            "SELECT {SESSION_COLUMNS} FROM auth_sessions
             WHERE user_id = $1 ORDER BY created_at DESC"
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(session_of).collect())
    }

    async fn purge_expired(&self, now: DateTime<Utc>) -> Result<u64, StoreError> {
        let result = sqlx::query("DELETE FROM auth_sessions WHERE expires_at <= $1")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

pub struct PostgresAccountStore {
    pub pool: PgPool,
}

impl PostgresAccountStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const ACCOUNT_COLUMNS: &str =
    "id, org_id, email, name, avatar_url, password_hash, email_verified_at, created_at";

type AccountRow = (
    Uuid,
    Option<Uuid>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<DateTime<Utc>>,
    DateTime<Utc>,
);

fn user_of(row: AccountRow) -> UserRecord {
    UserRecord {
        id: row.0,
        org_id: row.1,
        email: row.2,
        name: row.3,
        avatar_url: row.4,
        password_hash: row.5,
        email_verified_at: row.6,
        created_at: row.7,
    }
}

fn classify(error: sqlx::Error) -> StoreError {
    if let sqlx::Error::Database(ref inner) = error {
        if inner.code().as_deref() == Some("23505") {
            return StoreError::Duplicate("unique constraint".to_string());
        }
    }
    StoreError::Database(error)
}

#[async_trait]
impl AccountStore for PostgresAccountStore {
    async fn find_by_email(&self, email: &str) -> Result<Option<UserRecord>, StoreError> {
        let row = sqlx::query_as::<_, AccountRow>(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM users WHERE lower(email) = lower($1)"
        ))
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(user_of))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<UserRecord>, StoreError> {
        let row = sqlx::query_as::<_, AccountRow>(&format!(
            "SELECT {ACCOUNT_COLUMNS} FROM users WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(user_of))
    }

    async fn create(
        &self,
        email: &str,
        name: &str,
        password_hash: Option<&str>,
        verified: bool,
    ) -> Result<UserRecord, StoreError> {
        let row = sqlx::query_as::<_, AccountRow>(&format!(
            "INSERT INTO users (email, name, password_hash, email_verified_at, password_changed_at)
             VALUES (lower(btrim($1)), btrim($2), $3, CASE WHEN $4 THEN now() END, now())
             RETURNING {ACCOUNT_COLUMNS}"
        ))
        .bind(email)
        .bind(name)
        .bind(password_hash)
        .bind(verified)
        .fetch_one(&self.pool)
        .await
        .map_err(classify)?;
        Ok(user_of(row))
    }

    async fn set_password_hash(&self, id: Uuid, hash: &str) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE users SET password_hash = $2, password_changed_at = now() WHERE id = $1",
        )
        .bind(id)
        .bind(hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn set_org(&self, id: Uuid, org_id: Uuid) -> Result<(), StoreError> {
        sqlx::query("UPDATE users SET org_id = $2 WHERE id = $1 AND org_id IS NULL")
            .bind(id)
            .bind(org_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find_by_identity(
        &self,
        provider_id: &str,
        subject: &str,
    ) -> Result<Option<UserRecord>, StoreError> {
        let row = sqlx::query_as::<_, AccountRow>(&format!(
            "SELECT {} FROM users u
             JOIN auth_identities i ON i.user_id = u.id
             WHERE i.provider_id = $1 AND i.subject = $2",
            ACCOUNT_COLUMNS
                .split(", ")
                .map(|column| format!("u.{column}"))
                .collect::<Vec<String>>()
                .join(", ")
        ))
        .bind(provider_id)
        .bind(subject)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(user_of))
    }

    async fn link_identity(&self, link: IdentityLink) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO auth_identities (provider_id, subject, user_id, email_at_link)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (provider_id, subject) DO NOTHING",
        )
        .bind(&link.provider_id)
        .bind(&link.subject)
        .bind(link.user_id)
        .bind(&link.email_at_link)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_identities(&self, user_id: Uuid) -> Result<Vec<IdentityLink>, StoreError> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT provider_id, subject, email_at_link FROM auth_identities
             WHERE user_id = $1 ORDER BY provider_id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(provider_id, subject, email_at_link)| IdentityLink {
                user_id,
                provider_id,
                subject,
                email_at_link,
            })
            .collect())
    }

    async fn unlink_identity(&self, user_id: Uuid, provider_id: &str) -> Result<bool, StoreError> {
        let result =
            sqlx::query("DELETE FROM auth_identities WHERE user_id = $1 AND provider_id = $2")
                .bind(user_id)
                .bind(provider_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

pub struct PostgresOidcStateStore {
    pub pool: PgPool,
}

impl PostgresOidcStateStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OidcStateStore for PostgresOidcStateStore {
    async fn put(&self, state: OidcState) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO auth_oidc_states
                (state, provider_id, nonce, code_verifier, redirect_to, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&state.state)
        .bind(&state.provider_id)
        .bind(&state.nonce)
        .bind(&state.code_verifier)
        .bind(&state.redirect_to)
        .bind(state.expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn take(&self, state: &str) -> Result<Option<OidcState>, StoreError> {
        let row = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "DELETE FROM auth_oidc_states
             WHERE state = $1 AND expires_at > now()
             RETURNING state, provider_id, nonce, code_verifier, redirect_to",
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(state, provider_id, nonce, code_verifier, redirect_to)| OidcState {
                state,
                provider_id,
                nonce,
                code_verifier,
                redirect_to,
                expires_at: Utc::now(),
            },
        ))
    }
}

pub struct PostgresThrottleStore {
    pub pool: PgPool,
}

impl PostgresThrottleStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ThrottleStore for PostgresThrottleStore {
    async fn failures_since(&self, email: &str, since: DateTime<Utc>) -> Result<i64, StoreError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM auth_login_attempts
             WHERE lower(email) = lower($1) AND succeeded = false AND created_at > $2",
        )
        .bind(email)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    async fn record(&self, email: &str, succeeded: bool) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO auth_login_attempts (email, succeeded) VALUES (lower($1), $2)")
            .bind(email)
            .bind(succeeded)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub fn postgres_roles(pool: PgPool) -> Arc<dyn RoleStore> {
    Arc::new(PostgresRoleStore::new(pool))
}
