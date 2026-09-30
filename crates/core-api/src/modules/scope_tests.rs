//! The org and project routes only ever answer for a caller.

use crate::identity::Caller;
use crate::modules::membership::{denied, PostgresScope, ScopeError, ScopedRead};
use async_trait::async_trait;
use axum::http::StatusCode;
use menzi_auth::role::Role;
use menzi_auth::role_store::InMemoryRoleStore;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

struct Nothing {
    _pool: PgPool,
}

fn pool() -> PgPool {
    PgPool::connect_lazy("postgres://unused:unused@127.0.0.1:1/unused").expect("lazy pool")
}

#[async_trait]
impl ScopedRead for Nothing {
    async fn caller_id(&self, caller: &Caller) -> Result<Uuid, ScopeError> {
        caller
            .user_id()
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(ScopeError::NoCaller)
    }
    async fn assert_org(&self, _: &Caller, _: Uuid) -> Result<(), ScopeError> {
        Ok(())
    }
    async fn assert_project(&self, _: &Caller, _: Uuid) -> Result<(), ScopeError> {
        Ok(())
    }
    async fn list_orgs(&self, _: &Caller) -> Result<Vec<Uuid>, ScopeError> {
        Ok(Vec::new())
    }
    async fn list_projects(&self, _: &Caller) -> Result<Vec<Uuid>, ScopeError> {
        Ok(Vec::new())
    }
}

fn scope() -> PostgresScope {
    PostgresScope::new(pool(), Arc::new(InMemoryRoleStore::new()))
}

fn caller(id: &str) -> Caller {
    Caller::User(id.to_string())
}

fn roles() -> (Arc<InMemoryRoleStore>, PostgresScope) {
    let store = Arc::new(InMemoryRoleStore::new());
    let scope = PostgresScope::new(pool(), store.clone());
    (store, scope)
}

#[tokio::test]
async fn a_service_caller_has_no_user_id() {
    let empty = Nothing { _pool: pool() };
    assert!(matches!(
        empty
            .caller_id(&Caller::Service("session-proxy".into()))
            .await,
        Err(ScopeError::NoCaller)
    ));
}

#[tokio::test]
async fn a_caller_that_is_not_a_uuid_has_no_id() {
    let empty = Nothing { _pool: pool() };
    assert!(matches!(
        empty.caller_id(&caller("not-a-uuid")).await,
        Err(ScopeError::NoCaller)
    ));
}

#[tokio::test]
async fn a_user_caller_yields_its_uuid() {
    let empty = Nothing { _pool: pool() };
    let id = Uuid::new_v4();
    assert!(matches!(empty.caller_id(&caller(&id.to_string())).await, Ok(found) if found == id));
}

#[tokio::test]
async fn a_caller_may_only_read_the_org_it_owns() {
    let (store, scope) = roles();
    let user = Uuid::new_v4();
    let mine = Uuid::new_v4();
    let theirs = Uuid::new_v4();
    store.set_org_for(user, mine);

    assert!(scope
        .assert_org(&caller(&user.to_string()), mine)
        .await
        .is_ok());
    assert!(matches!(
        scope.assert_org(&caller(&user.to_string()), theirs).await,
        Err(ScopeError::WrongOrg)
    ));
}

#[tokio::test]
async fn a_caller_with_no_org_owns_none() {
    let (_store, scope) = roles();
    assert!(matches!(
        scope
            .assert_org(&caller(&Uuid::new_v4().to_string()), Uuid::new_v4())
            .await,
        Err(ScopeError::WrongOrg)
    ));
}

#[tokio::test]
async fn project_membership_is_what_gates_a_project() {
    let (store, scope) = roles();
    let user = Uuid::new_v4();
    let member = Uuid::new_v4();
    let other = Uuid::new_v4();
    store.grant(user, member, Role::Developer);

    assert!(scope
        .assert_project(&caller(&user.to_string()), member)
        .await
        .is_ok());
    assert!(matches!(
        scope
            .assert_project(&caller(&user.to_string()), other)
            .await,
        Err(ScopeError::WrongProject)
    ));
}

#[tokio::test]
async fn an_owner_may_read_the_project() {
    let (store, scope) = roles();
    let user = Uuid::new_v4();
    let project = Uuid::new_v4();
    store.grant(user, project, Role::Owner);
    assert!(scope
        .assert_project(&caller(&user.to_string()), project)
        .await
        .is_ok());
}

#[tokio::test]
async fn the_errors_map_onto_distinct_statuses() {
    assert_eq!(ScopeError::NoCaller.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(ScopeError::WrongOrg.status(), StatusCode::FORBIDDEN);
    assert_eq!(ScopeError::WrongProject.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_refusal_answers_with_its_reason() {
    assert_eq!(
        denied(ScopeError::WrongProject).status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        denied(ScopeError::NoCaller).status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn a_scope_can_be_shared_across_requests() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let scope = scope();
    assert_send_sync(&scope);
}

#[tokio::test]
async fn a_default_scope_reports_no_caller() {
    let scope = scope();
    assert!(matches!(
        scope.caller_id(&Caller::Service("x".into())).await,
        Err(ScopeError::NoCaller)
    ));
}
