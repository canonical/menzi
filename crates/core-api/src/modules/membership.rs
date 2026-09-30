//! Verifies that the org and project routes only ever answer for a caller.
//!
//! The behaviour lives here rather than in each handler so the SQL cannot drift
//! back to an unfiltered read, which is what made `list_orgs` leak every org in
//! the database.

use async_trait::async_trait;
use axum::http::StatusCode;
use menzi_auth::role_store::RoleStore;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::identity::Caller;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Caller,
    Org,
    Project,
}

#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("who is calling is not established")]
    NoCaller,
    #[error("not your organisation")]
    WrongOrg,
    #[error("not your project")]
    WrongProject,
    #[error("{0}")]
    Database(#[from] sqlx::Error),
}

impl ScopeError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NoCaller => StatusCode::UNAUTHORIZED,
            Self::WrongOrg | Self::WrongProject => StatusCode::FORBIDDEN,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Database(error) => error.to_string(),
            other => other.to_string(),
        }
    }
}

#[async_trait]
pub trait ScopedRead: Send + Sync {
    async fn caller_id(&self, caller: &Caller) -> Result<Uuid, ScopeError>;
    async fn assert_org(&self, caller: &Caller, org_id: Uuid) -> Result<(), ScopeError>;
    async fn assert_project(&self, caller: &Caller, project_id: Uuid) -> Result<(), ScopeError>;
    async fn list_orgs(&self, caller: &Caller) -> Result<Vec<Uuid>, ScopeError>;
    async fn list_projects(&self, caller: &Caller) -> Result<Vec<Uuid>, ScopeError>;
}

pub struct PostgresScope {
    pool: PgPool,
    roles: Arc<dyn RoleStore>,
}

impl PostgresScope {
    pub fn new(pool: PgPool, roles: Arc<dyn RoleStore>) -> Self {
        Self { pool, roles }
    }
}

#[async_trait]
impl ScopedRead for PostgresScope {
    async fn caller_id(&self, caller: &Caller) -> Result<Uuid, ScopeError> {
        caller
            .user_id()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(ScopeError::NoCaller)
    }

    async fn assert_org(&self, caller: &Caller, org_id: Uuid) -> Result<(), ScopeError> {
        let id = self.caller_id(caller).await?;
        match self.roles.org_of(id).await {
            Some(owned) if owned == org_id => Ok(()),
            Some(_) => Err(ScopeError::WrongOrg),
            None => Err(ScopeError::WrongOrg),
        }
    }

    async fn assert_project(&self, caller: &Caller, project_id: Uuid) -> Result<(), ScopeError> {
        let id = self.caller_id(caller).await?;
        if self.roles.is_member(id, project_id).await {
            Ok(())
        } else {
            Err(ScopeError::WrongProject)
        }
    }

    async fn list_orgs(&self, caller: &Caller) -> Result<Vec<Uuid>, ScopeError> {
        let id = self.caller_id(caller).await?;
        let rows = sqlx::query_as::<_, (Uuid,)>("SELECT org_id FROM users WHERE id = $1")
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|row| row.0).collect())
    }

    async fn list_projects(&self, caller: &Caller) -> Result<Vec<Uuid>, ScopeError> {
        let id = self.caller_id(caller).await?;
        let rows = sqlx::query_as::<_, (Uuid,)>(
            "SELECT project_id FROM project_members WHERE user_id = $1",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| row.0).collect())
    }
}

pub fn denied(error: ScopeError) -> axum::response::Response {
    (
        error.status(),
        axum::Json(serde_json::json!({ "error": error.message() })),
    )
        .into_response()
}

use axum::response::IntoResponse;
