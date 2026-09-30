use axum::extract::{Extension, Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::config::AuthConfig;
use super::cookie;
use super::middleware::{json_error, require_caller, require_csrf, strip_identity_headers};
use super::oidc::{AuthError, ProviderDescriptor};
use super::service::{AuthOutcome, LoginRequest, RegisterRequest, RequestMeta};
use super::state::AuthState;

pub fn meta_of(headers: &HeaderMap) -> RequestMeta {
    RequestMeta {
        user_agent: headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        ip: headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(',').next().unwrap_or(value).trim().to_string()),
    }
}

pub fn error_response(error: AuthError) -> Response {
    match error {
        AuthError::NotFound => json_error(StatusCode::NOT_FOUND, "not_found", "not found"),
        AuthError::BadRequest(message) => {
            json_error(StatusCode::BAD_REQUEST, "invalid_request", &message)
        }
        AuthError::Unauthorized => json_error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "the email or password is wrong",
        ),
        AuthError::Forbidden => json_error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "this account cannot be created from that sign-in",
        ),
        AuthError::Conflict(message) => json_error(StatusCode::CONFLICT, "conflict", &message),
        AuthError::TooManyRequests => json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "too_many_requests",
            "too many attempts",
        ),
        AuthError::Locked => json_error(
            StatusCode::LOCKED,
            "locked",
            "too many failed attempts, try again later",
        ),
        AuthError::Unavailable(_) => json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the service is temporarily unavailable",
        ),
    }
}

fn with_cookies(mut response: Response, cookies: Vec<String>) -> Response {
    for value in cookies {
        if let Ok(header) = axum::http::HeaderValue::from_str(&value) {
            response
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, header);
        }
    }
    response
}

fn session_cookies(config: &AuthConfig, outcome: &AuthOutcome) -> Vec<String> {
    let max_age = config.max_age();
    vec![
        cookie::session(
            &config.session_cookie,
            outcome.session_token.expose(),
            max_age,
            config.secure_cookies,
        ),
        cookie::csrf(
            &config.csrf_cookie,
            outcome.csrf_token.expose(),
            max_age,
            config.secure_cookies,
        ),
    ]
}

pub fn clear_cookies(config: &AuthConfig) -> Vec<String> {
    vec![
        cookie::clear(&config.session_cookie, config.secure_cookies),
        cookie::clear(&config.csrf_cookie, config.secure_cookies),
    ]
}

fn ok_with_cookies(config: &AuthConfig, outcome: &AuthOutcome, status: StatusCode) -> Response {
    with_cookies(
        (status, Json(serde_json::json!({ "ok": true }))).into_response(),
        session_cookies(config, outcome),
    )
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ForgotBody {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetBody {
    pub token: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize)]
pub struct ProvidersResponse {
    pub registration: &'static str,
    pub password: bool,
    pub oidc: Vec<ProviderDescriptor>,
}

#[derive(Serialize)]
pub struct Device {
    pub id: String,
    pub provider: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub current: bool,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

pub async fn providers(Extension(state): Extension<Arc<AuthState>>) -> Response {
    let service = &state.service;
    Json(ProvidersResponse {
        registration: service.config().registration_mode.as_str(),
        password: true,
        oidc: service
            .providers()
            .iter()
            .map(|provider| ProviderDescriptor {
                id: provider.id().to_string(),
                label: provider.label().to_string(),
            })
            .collect(),
    })
    .into_response()
}

pub async fn register(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(body): Json<RegisterBody>,
) -> Response {
    match state
        .service
        .register(
            RegisterRequest {
                email: body.email,
                name: body.name,
                password: body.password,
            },
            meta_of(&headers),
        )
        .await
    {
        Ok(outcome) => ok_with_cookies(&state.config, &outcome, StatusCode::CREATED),
        Err(error) => error_response(error),
    }
}

pub async fn login(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Response {
    match state
        .service
        .login(
            LoginRequest {
                email: body.email,
                password: body.password,
            },
            meta_of(&headers),
        )
        .await
    {
        Ok(outcome) => ok_with_cookies(&state.config, &outcome, StatusCode::OK),
        Err(error) => error_response(error),
    }
}

pub async fn logout(Extension(state): Extension<Arc<AuthState>>, headers: HeaderMap) -> Response {
    let token = cookie::value_of(&headers, &state.config.session_cookie).map(str::to_string);
    let _ = state.service.logout(token.as_deref()).await;
    with_cookies(
        (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        clear_cookies(&state.config),
    )
}

pub async fn forgot(
    Extension(state): Extension<Arc<AuthState>>,
    Json(body): Json<ForgotBody>,
) -> Response {
    let _ = state.service.request_reset(&body.email).await;
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}

pub async fn reset(
    Extension(state): Extension<Arc<AuthState>>,
    Json(body): Json<ResetBody>,
) -> Response {
    match state
        .service
        .complete_reset(&body.token, &body.password)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn session(Extension(state): Extension<Arc<AuthState>>, headers: HeaderMap) -> Response {
    let Some(token) = cookie::value_of(&headers, &state.config.session_cookie) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "no session" })),
        )
            .into_response();
    };
    let Ok(Some(user)) = state.service.session_for(token).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "the session has ended" })),
        )
            .into_response();
    };
    let Some(described) = state.service.describe(user.id).await else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "the user no longer exists" })),
        )
            .into_response();
    };
    Json(serde_json::json!({
        "user": described,
        "csrf_token": cookie::value_of(&headers, &state.config.csrf_cookie),
    }))
    .into_response()
}

pub async fn change_password(
    Extension(state): Extension<Arc<AuthState>>,
    Extension(caller): Extension<crate::identity::Caller>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> Response {
    let Some(user_id) = caller.user_id().and_then(|id| id.parse::<Uuid>().ok()) else {
        return error_response(AuthError::Forbidden);
    };
    let Some(token) = cookie::value_of(&headers, &state.config.session_cookie) else {
        return error_response(AuthError::Unauthorized);
    };
    let Some(session_id) = state.service.session_id_for(token).await else {
        return error_response(AuthError::Unauthorized);
    };
    match state
        .service
        .change_password(
            user_id,
            session_id,
            &body.current_password,
            &body.new_password,
        )
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn devices(
    Extension(state): Extension<Arc<AuthState>>,
    Extension(caller): Extension<crate::identity::Caller>,
    headers: HeaderMap,
) -> Response {
    let Some(user_id) = caller.user_id().and_then(|id| id.parse::<Uuid>().ok()) else {
        return error_response(AuthError::Forbidden);
    };
    let current_id = match cookie::value_of(&headers, &state.config.session_cookie) {
        Some(token) => state.service.session_id_for(token).await,
        None => None,
    };
    match state.service.devices(user_id).await {
        Ok(list) => {
            let devices: Vec<Device> = list
                .into_iter()
                .map(|device| Device {
                    current: Some(device.id) == current_id,
                    id: device.id.to_string(),
                    provider: device.provider_id,
                    created_at: device.created_at.to_rfc3339(),
                    last_seen_at: device.last_seen_at.to_rfc3339(),
                    user_agent: device.user_agent,
                    ip: device.ip,
                })
                .collect();
            Json(serde_json::json!({ "devices": devices })).into_response()
        }
        Err(error) => error_response(error),
    }
}

pub async fn revoke_device(
    Extension(state): Extension<Arc<AuthState>>,
    Extension(caller): Extension<crate::identity::Caller>,
    Path(id): Path<Uuid>,
) -> Response {
    let Some(user_id) = caller.user_id().and_then(|id| id.parse::<Uuid>().ok()) else {
        return error_response(AuthError::Forbidden);
    };
    match state.service.revoke_device(user_id, id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn oidc_start(
    Extension(state): Extension<Arc<AuthState>>,
    Path(provider_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    match state
        .service
        .start_oidc(&provider_id, query.get("redirect_to").map(String::as_str))
        .await
    {
        Ok(url) => Redirect::temporary(&url).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn oidc_callback(
    Extension(state): Extension<Arc<AuthState>>,
    Path(provider_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    let code = query.get("code").cloned().unwrap_or_default();
    let flow_state = query.get("state").cloned().unwrap_or_default();
    if code.is_empty() || flow_state.is_empty() {
        return error_response(AuthError::BadRequest("the sign-in was incomplete".into()));
    }
    match state
        .service
        .finish_oidc(&provider_id, &code, &flow_state, meta_of(&headers))
        .await
    {
        Ok(result) => with_cookies(
            Redirect::temporary(&result.target).into_response(),
            session_cookies(&state.config, &result.outcome),
        ),
        Err(error) => error_response(error),
    }
}

pub fn public_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/auth/providers", get(providers))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/password/forgot", post(forgot))
        .route("/api/v1/auth/password/reset", post(reset))
        .route("/api/v1/auth/oidc/{provider}/start", get(oidc_start))
        .route("/api/v1/auth/oidc/{provider}/callback", get(oidc_callback))
}

pub fn protected_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/v1/me", get(super::session::me))
        .route("/api/v1/auth/session", get(session))
        .route("/api/v1/auth/sessions", get(devices))
        .route("/api/v1/auth/sessions/{id}", delete(revoke_device))
        .route("/api/v1/auth/password/change", post(change_password))
}

pub fn merge<S>(state: Arc<AuthState>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let config = state.config.clone();
    let resolver = state.resolver.clone();
    let protected = protected_routes::<S>()
        .layer(axum::middleware::from_fn_with_state(config, require_csrf))
        .layer(axum::middleware::from_fn_with_state(
            resolver,
            require_caller,
        ));
    public_routes::<S>()
        .merge(protected)
        .layer(axum::middleware::from_fn(strip_identity_headers))
        .layer(Extension(state))
}
