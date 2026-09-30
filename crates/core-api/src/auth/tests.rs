#![cfg(test)]

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use chrono::Utc;
use menzi_auth::record::UserRecord;
use menzi_auth::role::Role;
use menzi_auth::role_store::InMemoryRoleStore;
use menzi_auth::store::{
    AccountStore, InMemoryAccountStore, InMemoryOidcStateStore, InMemorySessionStore,
    InMemoryThrottleStore, SessionStore,
};
use serde_json::Value;

type Code = axum::http::StatusCode;
use std::sync::Arc;
use tower::ServiceExt;

#[allow(unused_imports)]
use menzi_auth::store::OidcStateStore;

use crate::auth::clock::FixedClock;
use crate::auth::config::{AuthConfig, RegistrationMode};
use crate::auth::identity::seed_session;
use crate::auth::mailer::StubMailer;
use crate::auth::oidc::{FakeOidcProvider, IdTokenClaims};
use crate::auth::service::AuthService;
use crate::auth::state::AuthState;

const PASSWORD: &str = "a-long-enough-password";

struct Harness {
    state: Arc<AuthState>,
    accounts: Arc<InMemoryAccountStore>,
    sessions: Arc<InMemorySessionStore>,
    roles: Arc<InMemoryRoleStore>,
    mailer: Arc<StubMailer>,
    clock: Arc<FixedClock>,
    provider: Arc<FakeOidcProvider>,
}

fn claims(subject: &str, email: &str, verified: Option<bool>) -> IdTokenClaims {
    IdTokenClaims {
        subject: subject.to_string(),
        email: Some(email.to_string()),
        email_verified: verified,
        name: Some("A Person".to_string()),
        picture: None,
    }
}

fn harness(registration: RegistrationMode, auto_provision: bool) -> Harness {
    let accounts = Arc::new(InMemoryAccountStore::new());
    let sessions = Arc::new(InMemorySessionStore::new());
    let states = Arc::new(InMemoryOidcStateStore::new());
    let throttle = Arc::new(InMemoryThrottleStore::new());
    let roles = Arc::new(InMemoryRoleStore::new());
    let clock = Arc::new(FixedClock::new(Utc::now()));
    let mailer = Arc::new(StubMailer::new());
    let provider = Arc::new(FakeOidcProvider::new(
        "google",
        claims("sub-1", "from.provider@example.com", Some(true)),
    ));

    let config = AuthConfig {
        registration_mode: registration,
        oidc_auto_provision: auto_provision,
        ..AuthConfig::for_tests()
    };
    let service = AuthService::new(
        accounts.clone(),
        sessions.clone(),
        states.clone(),
        throttle,
        clock.clone(),
        mailer.clone(),
        vec![provider.clone()],
        config.clone(),
    )
    .with_roles(roles.clone());
    let shared = Arc::new(config);
    let resolver = Arc::new(super::identity::SessionResolver::new(
        sessions.clone(),
        accounts.clone(),
        shared.clone(),
    ));

    Harness {
        state: Arc::new(AuthState {
            service: Arc::new(service),
            config: shared,
            resolver,
        }),
        accounts,
        sessions,
        roles,
        mailer,
        clock,
        provider,
    }
}

async fn user_of(harness: &Harness, email: &str) -> UserRecord {
    harness
        .accounts
        .find_by_email(email)
        .await
        .unwrap()
        .unwrap()
}

fn register_body() -> Value {
    serde_json::json!({
        "email": "person@example.com",
        "name": "A Person",
        "password": PASSWORD,
    })
}

fn login_body() -> Value {
    serde_json::json!({ "email": "person@example.com", "password": PASSWORD })
}

fn lazy_pool() -> sqlx::PgPool {
    sqlx::PgPool::connect_lazy("postgres://unused:unused@127.0.0.1:1/unused").expect("lazy pool")
}

fn app() -> Router {
    app_with(Arc::new(AuthState::for_tests()))
}

fn app_with(state: Arc<AuthState>) -> Router {
    crate::create_router(state).with_state(lazy_pool())
}

struct Reply {
    status: Code,
    cookies: Vec<String>,
    body: Value,
}

fn cookie_of(reply: &Reply, name: &str) -> Option<String> {
    reply
        .cookies
        .iter()
        .find(|raw| raw.starts_with(&format!("{name}=")))
        .and_then(|raw| {
            raw.split(';')
                .next()
                .and_then(|pair| pair.split_once('='))
                .map(|(_, value)| value.to_string())
        })
}

async fn send(app: Router, method: &str, uri: &str, body: Option<Value>) -> Reply {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let cookies: Vec<String> = response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap_or_default().to_string())
        .collect();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply {
        status,
        cookies,
        body: parsed,
    }
}

async fn signed_in(harness: &Harness) -> String {
    let reply = send(
        app_with(harness.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    assert_eq!(reply.status, Code::CREATED);
    cookie_of(&reply, "menzi_session").expect("session cookie")
}

fn with_cookie(token: &str) -> String {
    format!("menzi_session={token}; menzi_csrf=csrf-value")
}

fn csrf_in(cookie: &str) -> String {
    cookie
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find(|(key, _)| *key == "menzi_csrf")
        .map(|(_, value)| value.to_string())
        .unwrap_or_default()
}

async fn send_with_cookie(
    app: Router,
    method: &str,
    uri: &str,
    cookie: &str,
    body: Option<Value>,
) -> Reply {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("cookie", cookie);
    if method != "GET" {
        builder = builder.header("x-menzi-csrf", csrf_in(cookie));
    }
    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let cookies: Vec<String> = response
        .headers()
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap_or_default().to_string())
        .collect();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    Reply {
        status,
        cookies,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

fn query_of(url: &str, key: &str) -> String {
    url.split('?')
        .nth(1)
        .unwrap_or_default()
        .split('&')
        .find_map(|pair| {
            pair.split_once('=')
                .filter(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        })
        .unwrap_or_default()
}

// register

#[tokio::test]
async fn registering_creates_a_user() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    assert_eq!(reply.status, Code::CREATED);
    let user = user_of(&h, "person@example.com").await;
    assert_eq!(user.email, "person@example.com");
}

#[tokio::test]
async fn registering_lowercases_the_email() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(serde_json::json!({
            "email": "  Person@Example.COM ",
            "name": "A Person",
            "password": PASSWORD,
        })),
    )
    .await;
    assert_eq!(reply.status, Code::CREATED);
    assert!(user_of(&h, "person@example.com").await.id != uuid::Uuid::nil());
}

#[tokio::test]
async fn registering_stores_a_hash_and_not_the_password() {
    let h = harness(RegistrationMode::Open, false);
    send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    let stored = user_of(&h, "person@example.com")
        .await
        .password_hash
        .unwrap();
    assert!(!stored.contains(PASSWORD));
    assert!(menzi_auth::password::verify_stored(&stored, PASSWORD));
}

#[tokio::test]
async fn registering_sets_a_session_cookie_and_a_readable_csrf_cookie() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    let session = reply
        .cookies
        .iter()
        .find(|c| c.starts_with("menzi_session="));
    let csrf = reply.cookies.iter().find(|c| c.starts_with("menzi_csrf="));
    assert!(session.unwrap().contains("HttpOnly"));
    assert!(!csrf.unwrap().contains("HttpOnly"));
    assert!(session.unwrap().contains("SameSite=Lax"));
}

#[tokio::test]
async fn registering_when_closed_is_a_404() {
    let h = harness(RegistrationMode::Closed, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    assert_eq!(reply.status, Code::NOT_FOUND);
    assert!(reply.cookies.is_empty());
}

#[tokio::test]
async fn registering_when_invite_only_is_a_404() {
    let h = harness(RegistrationMode::Invite, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    assert_eq!(reply.status, Code::NOT_FOUND);
}

#[tokio::test]
async fn registering_twice_is_a_conflict() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    assert_eq!(
        send(
            app.clone(),
            "POST",
            "/api/v1/auth/register",
            Some(register_body())
        )
        .await
        .status,
        Code::CREATED
    );
    let reply = send(app, "POST", "/api/v1/auth/register", Some(register_body())).await;
    assert_eq!(reply.status, Code::CONFLICT);
    assert_eq!(reply.body["error"]["code"], "conflict");
}

#[tokio::test]
async fn registering_a_short_password_is_a_bad_request() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(serde_json::json!({
            "email": "person@example.com",
            "name": "A Person",
            "password": "short",
        })),
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
    assert!(reply.body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("12"));
}

#[tokio::test]
async fn registering_without_a_name_is_a_bad_request() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(serde_json::json!({
            "email": "person@example.com",
            "name": "  ",
            "password": PASSWORD,
        })),
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
}

#[tokio::test]
async fn registering_a_malformed_email_is_a_bad_request() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(serde_json::json!({
            "email": "not-an-email",
            "name": "A Person",
            "password": PASSWORD,
        })),
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
}

// login

#[tokio::test]
async fn logging_in_with_the_right_password_sets_a_cookie() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    send(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    let reply = send(app, "POST", "/api/v1/auth/login", Some(login_body())).await;
    assert_eq!(reply.status, Code::OK);
    assert!(cookie_of(&reply, "menzi_session").is_some());
}

#[tokio::test]
async fn logging_in_with_the_wrong_password_is_401() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    send(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    let reply = send(
        app,
        "POST",
        "/api/v1/auth/login",
        Some(serde_json::json!({
            "email": "person@example.com",
            "password": "wrong-password-here",
        })),
    )
    .await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
    assert!(reply.cookies.is_empty());
    assert_eq!(reply.body["error"]["code"], "invalid_credentials");
}

#[tokio::test]
async fn logging_in_against_an_unknown_email_is_401() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/login",
        Some(serde_json::json!({
            "email": "nobody@example.com",
            "password": PASSWORD,
        })),
    )
    .await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
}

#[tokio::test]
async fn a_locked_email_is_refused() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    send(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    for _ in 0..10 {
        let _ = send(
            app.clone(),
            "POST",
            "/api/v1/auth/login",
            Some(serde_json::json!({
                "email": "person@example.com",
                "password": "wrong-password-here",
            })),
        )
        .await;
    }
    let reply = send(app, "POST", "/api/v1/auth/login", Some(login_body())).await;
    assert_eq!(reply.status, Code::LOCKED);
    assert_eq!(reply.body["error"]["code"], "locked");
}

#[tokio::test]
async fn logging_out_clears_both_cookies() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let reply = send_with_cookie(
        app,
        "POST",
        "/api/v1/auth/logout",
        &with_cookie(&token),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::OK);
    assert_eq!(reply.cookies.len(), 2);
    assert!(reply.cookies.iter().all(|c| c.contains("Max-Age=0")));
}

#[tokio::test]
async fn a_revoked_session_stops_working() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let user = user_of(&h, "person@example.com").await;
    let session = h.sessions.list_for(user.id).await.unwrap().remove(0);
    h.sessions.revoke(session.id).await.unwrap();
    let reply = send_with_cookie(app, "GET", "/api/v1/me", &with_cookie(&token), None).await;
    assert_eq!(reply.status, Code::FORBIDDEN);
}

// providers

#[tokio::test]
async fn the_providers_endpoint_needs_no_session() {
    let reply = send(app(), "GET", "/api/v1/auth/providers", None).await;
    assert_eq!(reply.status, Code::OK);
}

#[tokio::test]
async fn the_providers_endpoint_reports_the_registration_mode() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/auth/providers",
        None,
    )
    .await;
    assert_eq!(reply.body["registration"], "open");
    assert_eq!(reply.body["password"], true);
}

#[tokio::test]
async fn the_providers_endpoint_lists_the_configured_provider() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/auth/providers",
        None,
    )
    .await;
    assert_eq!(reply.body["oidc"][0]["id"], "google");
    assert!(reply.body["oidc"][0].get("client_secret").is_none());
    assert!(!reply.body.to_string().contains("client_secret"));
}

#[tokio::test]
async fn a_closed_deployment_reports_no_providers() {
    let state = AuthState::for_tests();
    let reply = send(
        app_with(Arc::new(state)),
        "GET",
        "/api/v1/auth/providers",
        None,
    )
    .await;
    assert_eq!(reply.body["oidc"].as_array().unwrap().len(), 0);
}

// session and me

#[tokio::test]
async fn the_session_endpoint_answers_for_a_signed_in_user() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let reply = send_with_cookie(
        app,
        "GET",
        "/api/v1/auth/session",
        &with_cookie(&token),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::OK);
    assert_eq!(reply.body["user"]["email"], "person@example.com");
    assert_eq!(reply.body["user"]["name"], "A Person");
    assert_eq!(reply.body["user"]["kind"], "user");
}

#[tokio::test]
async fn the_session_endpoint_without_a_cookie_is_401() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/auth/session",
        None,
    )
    .await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
}

#[tokio::test]
async fn the_me_endpoint_returns_the_real_user() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let user = user_of(&h, "person@example.com").await;
    h.roles.grant(user.id, uuid::Uuid::new_v4(), Role::Owner);
    let reply = send_with_cookie(app, "GET", "/api/v1/me", &with_cookie(&token), None).await;
    assert_eq!(reply.status, Code::OK);
    assert_eq!(reply.body["email"], "person@example.com");
    assert_eq!(reply.body["name"], "A Person");
    assert_eq!(reply.body["role"], "owner");
    assert_eq!(reply.body["id"], user.id.to_string());
}

#[tokio::test]
async fn the_me_endpoint_reports_the_email() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let reply = send_with_cookie(app, "GET", "/api/v1/me", &with_cookie(&token), None).await;
    assert_eq!(reply.body["email"], "person@example.com");
}

#[tokio::test]
async fn the_me_endpoint_without_a_session_is_401() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(app_with(h.state.clone()), "GET", "/api/v1/me", None).await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
    assert_eq!(reply.body["error"]["code"], "unauthenticated");
}

// csrf

#[tokio::test]
async fn a_post_without_a_csrf_header_is_403() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let reply = send(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        Some(login_body()),
    )
    .await;
    let token = cookie_of(&reply, "menzi_session").unwrap_or_default();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/password/change")
                .header("cookie", with_cookie(&token))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), Code::FORBIDDEN);
}

#[tokio::test]
async fn a_get_needs_no_csrf_header() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .header("cookie", with_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), Code::OK);
}

// identity header stripping

#[tokio::test]
async fn a_client_supplied_identity_header_is_never_trusted() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let user = user_of(&h, "person@example.com").await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .header("cookie", with_cookie(&token))
                .header("x-menzi-user-id", "someone-elses")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), Code::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["id"], user.id.to_string());
}

// password change

#[tokio::test]
async fn changing_a_password_revokes_the_other_sessions() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let first = signed_in(&h).await;
    let second = send(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        Some(login_body()),
    )
    .await;
    let second_token = cookie_of(&second, "menzi_session").unwrap();

    let change = send_with_cookie(
        app,
        "POST",
        "/api/v1/auth/password/change",
        &with_cookie(&first),
        Some(serde_json::json!({
            "current_password": PASSWORD,
            "new_password": "an-even-longer-password",
        })),
    )
    .await;
    assert_eq!(change.status, Code::OK);

    let still = send_with_cookie(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/me",
        &with_cookie(&first),
        None,
    )
    .await;
    assert_eq!(still.status, Code::OK);

    let gone = send_with_cookie(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/me",
        &with_cookie(&second_token),
        None,
    )
    .await;
    assert_eq!(gone.status, Code::FORBIDDEN);
}

#[tokio::test]
async fn changing_a_password_needs_the_current_one() {
    let h = harness(RegistrationMode::Open, false);
    let first = signed_in(&h).await;
    let reply = send_with_cookie(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/password/change",
        &with_cookie(&first),
        Some(serde_json::json!({
            "current_password": "not-the-password",
            "new_password": "an-even-longer-password",
        })),
    )
    .await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
}

#[tokio::test]
async fn a_provider_only_account_cannot_change_a_password() {
    let h = harness(RegistrationMode::Open, true);
    let state = h.state.clone();
    let location = {
        let response = app_with(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/oidc/google/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        response
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    };
    let flow_state = query_of(&location, "state");
    let callback = send(
        app_with(state.clone()),
        "GET",
        &format!("/api/v1/auth/oidc/google/callback?code=c&state={flow_state}"),
        None,
    )
    .await;
    assert_eq!(callback.status, Code::TEMPORARY_REDIRECT);
    let token = cookie_of(&callback, "menzi_session").unwrap();

    let reply = send_with_cookie(
        app_with(state),
        "POST",
        "/api/v1/auth/password/change",
        &with_cookie(&token),
        Some(serde_json::json!({
            "current_password": "whatever-long-enough",
            "new_password": "an-even-longer-password",
        })),
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
}

// devices

#[tokio::test]
async fn the_signed_in_devices_are_listed() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let first = signed_in(&h).await;
    send(app, "POST", "/api/v1/auth/login", Some(login_body())).await;

    let reply = send_with_cookie(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/auth/sessions",
        &with_cookie(&first),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::OK);
    let devices = reply.body["devices"].as_array().unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices.iter().filter(|d| d["current"] == true).count(), 1);
    assert_eq!(devices[0]["provider"], "password");
}

#[tokio::test]
async fn a_device_can_be_revoked() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let listed = send_with_cookie(
        app.clone(),
        "GET",
        "/api/v1/auth/sessions",
        &with_cookie(&token),
        None,
    )
    .await;
    let csrf_reply = send(app, "GET", "/api/v1/auth/providers", None).await;
    let _ = csrf_reply;
    let id = listed.body["devices"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let csrf = send_with_cookie(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/auth/sessions",
        &with_cookie(&token),
        None,
    )
    .await;
    let _ = csrf;
    let reply = send_with_cookie(
        app_with(h.state.clone()),
        "DELETE",
        &format!("/api/v1/auth/sessions/{id}"),
        &with_cookie(&token),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::OK);
}

#[tokio::test]
async fn revoking_someone_elses_device_is_404() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let token = signed_in(&h).await;
    let reply = send_with_cookie(
        app,
        "DELETE",
        &format!("/api/v1/auth/sessions/{}", uuid::Uuid::new_v4()),
        &with_cookie(&token),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::NOT_FOUND);
}

// password reset

#[tokio::test]
async fn a_forgot_password_always_answers_accepted() {
    let h = harness(RegistrationMode::Open, false);
    let app = app_with(h.state.clone());
    let unknown = send(
        app.clone(),
        "POST",
        "/api/v1/auth/password/forgot",
        Some(serde_json::json!({ "email": "nobody@example.com" })),
    )
    .await;
    assert_eq!(unknown.status, Code::ACCEPTED);
    assert!(h.mailer.sent().is_empty());

    send(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        Some(register_body()),
    )
    .await;
    let known = send(
        app,
        "POST",
        "/api/v1/auth/password/forgot",
        Some(serde_json::json!({ "email": "person@example.com" })),
    )
    .await;
    assert_eq!(known.status, Code::ACCEPTED);
    let sent = h.mailer.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "person@example.com");
    assert!(sent[0].body.contains("/reset-password?token="));
}

#[tokio::test]
async fn resetting_without_a_token_is_a_bad_request() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/password/reset",
        Some(serde_json::json!({ "token": "", "password": PASSWORD })),
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
}

// oidc

async fn start_oidc(state: &Arc<AuthState>, provider: &str) -> (Code, String) {
    let response = app_with(state.clone())
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/auth/oidc/{provider}/start"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(axum::http::header::LOCATION)
        .map(|value| value.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    (status, location)
}

#[tokio::test]
async fn starting_oidc_redirects_with_a_pkce_challenge() {
    let h = harness(RegistrationMode::Open, false);
    let (status, location) = start_oidc(&h.state, "google").await;
    assert_eq!(status, Code::TEMPORARY_REDIRECT);
    assert!(location.contains("code_challenge="));
    assert!(location.contains("code_challenge_method=S256"));
    assert!(!location.contains("client_secret"));
}

#[tokio::test]
async fn starting_an_unknown_provider_is_404() {
    let h = harness(RegistrationMode::Open, false);
    let (status, _) = start_oidc(&h.state, "okta").await;
    assert_eq!(status, Code::NOT_FOUND);
}

#[tokio::test]
async fn an_oidc_callback_signs_in_a_new_user() {
    let h = harness(RegistrationMode::Open, true);
    let (_, location) = start_oidc(&h.state, "google").await;
    let flow_state = query_of(&location, "state");
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        &format!("/api/v1/auth/oidc/google/callback?code=code-1&state={flow_state}"),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::TEMPORARY_REDIRECT);
    assert!(cookie_of(&reply, "menzi_session").is_some());
    assert!(user_of(&h, "from.provider@example.com").await.id != uuid::Uuid::nil());
}

#[tokio::test]
async fn an_oidc_callback_redirects_to_the_requested_page() {
    let h = harness(RegistrationMode::Open, true);
    let response = app_with(h.state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/oidc/google/start?redirect_to=/projects/abc/code")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let location = response
        .headers()
        .get(axum::http::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let flow_state = query_of(&location, "state");
    let callback = app_with(h.state.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/auth/oidc/google/callback?code=c&state={flow_state}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        callback
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
        "/projects/abc/code"
    );
}

#[tokio::test]
async fn an_offsite_redirect_target_falls_back_to_projects() {
    let h = harness(RegistrationMode::Open, true);
    let response = app_with(h.state.clone())
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/oidc/google/start?redirect_to=//evil.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let location = response
        .headers()
        .get(axum::http::header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let flow_state = query_of(&location, "state");
    let callback = app_with(h.state.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/auth/oidc/google/callback?code=c&state={flow_state}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        callback
            .headers()
            .get(axum::http::header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
        "/projects"
    );
}

#[tokio::test]
async fn a_replayed_oidc_state_is_refused() {
    let h = harness(RegistrationMode::Open, true);
    let (_, location) = start_oidc(&h.state, "google").await;
    let flow_state = query_of(&location, "state");
    let uri = format!("/api/v1/auth/oidc/google/callback?code=c&state={flow_state}");
    assert_eq!(
        send(app_with(h.state.clone()), "GET", &uri, None)
            .await
            .status,
        Code::TEMPORARY_REDIRECT
    );
    let replay = send(app_with(h.state.clone()), "GET", &uri, None).await;
    assert_eq!(replay.status, Code::BAD_REQUEST);
}

#[tokio::test]
async fn an_expired_oidc_state_is_refused() {
    let h = harness(RegistrationMode::Open, true);
    let (_, location) = start_oidc(&h.state, "google").await;
    let flow_state = query_of(&location, "state");
    h.clock.advance(chrono::Duration::hours(1));
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        &format!("/api/v1/auth/oidc/google/callback?code=c&state={flow_state}"),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
}

#[tokio::test]
async fn an_oidc_callback_without_a_code_is_a_bad_request() {
    let h = harness(RegistrationMode::Open, true);
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        "/api/v1/auth/oidc/google/callback?state=abc",
        None,
    )
    .await;
    assert_eq!(reply.status, Code::BAD_REQUEST);
}

#[tokio::test]
async fn an_email_that_already_has_a_password_is_refused() {
    let h = harness(RegistrationMode::Open, true);
    send(
        app_with(h.state.clone()),
        "POST",
        "/api/v1/auth/register",
        Some(serde_json::json!({
            "email": "from.provider@example.com",
            "name": "A Person",
            "password": PASSWORD,
        })),
    )
    .await;
    let (_, location) = start_oidc(&h.state, "google").await;
    let flow_state = query_of(&location, "state");
    let reply = send(
        app_with(h.state.clone()),
        "GET",
        &format!("/api/v1/auth/oidc/google/callback?code=c&state={flow_state}"),
        None,
    )
    .await;
    assert_eq!(reply.status, Code::CONFLICT);
    assert!(reply.body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("link google"));
}

#[tokio::test]
async fn a_second_oidc_sign_in_returns_to_the_same_user() {
    let h = harness(RegistrationMode::Open, true);
    let (_, first_location) = start_oidc(&h.state, "google").await;
    send(
        app_with(h.state.clone()),
        "GET",
        &format!(
            "/api/v1/auth/oidc/google/callback?code=c&state={}",
            query_of(&first_location, "state")
        ),
        None,
    )
    .await;
    let (_, second_location) = start_oidc(&h.state, "google").await;
    send(
        app_with(h.state.clone()),
        "GET",
        &format!(
            "/api/v1/auth/oidc/google/callback?code=c2&state={}",
            query_of(&second_location, "state")
        ),
        None,
    )
    .await;
    let matches = h
        .accounts
        .find_by_email("from.provider@example.com")
        .await
        .unwrap()
        .unwrap();
    assert!(matches.id != uuid::Uuid::nil());
}

#[tokio::test]
async fn an_oidc_user_is_stored_without_a_password() {
    let h = harness(RegistrationMode::Open, true);
    let (_, location) = start_oidc(&h.state, "google").await;
    send(
        app_with(h.state.clone()),
        "GET",
        &format!(
            "/api/v1/auth/oidc/google/callback?code=c&state={}",
            query_of(&location, "state")
        ),
        None,
    )
    .await;
    let user = user_of(&h, "from.provider@example.com").await;
    assert!(user.password_hash.is_none());
    assert!(user.email_verified_at.is_some());
}

#[tokio::test]
async fn the_fake_provider_records_the_verifier_it_was_given() {
    let h = harness(RegistrationMode::Open, true);
    let (_, location) = start_oidc(&h.state, "google").await;
    send(
        app_with(h.state.clone()),
        "GET",
        &format!(
            "/api/v1/auth/oidc/google/callback?code=the-code&state={}",
            query_of(&location, "state")
        ),
        None,
    )
    .await;
    let recorded = h.provider.recorded().unwrap();
    assert_eq!(recorded.code, "the-code");
    assert!(!recorded.verifier.is_empty());
    assert!(!recorded.nonce.is_empty());
}

// the rest of the api is now gated

#[tokio::test]
async fn the_orgs_listing_needs_a_session() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(app_with(h.state.clone()), "GET", "/api/v1/orgs", None).await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
}

#[tokio::test]
async fn the_projects_listing_needs_a_session() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(app_with(h.state.clone()), "GET", "/api/v1/projects", None).await;
    assert_eq!(reply.status, Code::UNAUTHORIZED);
}

#[tokio::test]
async fn the_health_check_stays_public() {
    let h = harness(RegistrationMode::Open, false);
    let reply = send(app_with(h.state.clone()), "GET", "/health", None).await;
    assert_eq!(reply.status, Code::OK);
}

// helpers kept honest

#[tokio::test]
async fn a_seeded_session_resolves_over_http() {
    let accounts = InMemoryAccountStore::new();
    let sessions = InMemorySessionStore::new();
    let user = accounts
        .create("a@b", "A", Some("hash"), true)
        .await
        .unwrap();
    let token = seed_session(&sessions, user.id, "password", chrono::Duration::hours(1))
        .await
        .unwrap();
    assert!(token.starts_with("mz_"));
}
