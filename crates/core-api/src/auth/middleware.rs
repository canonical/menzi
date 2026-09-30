use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::sync::Arc;

use crate::identity::{Caller, CallerResolver, IdentityError, IDENTITY_HEADERS};

use super::config::AuthConfig;
use super::cookie;

pub fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

pub async fn strip_identity_headers(mut request: Request, next: Next) -> Response {
    for name in IDENTITY_HEADERS {
        request.headers_mut().remove(name);
    }
    next.run(request).await
}

pub async fn require_caller(
    State(resolver): State<Arc<dyn CallerResolver>>,
    mut request: Request,
    next: Next,
) -> Response {
    match resolver.resolve(request.headers()).await {
        Ok(caller) => {
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
        Err(IdentityError::Missing) => json_error(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "sign in to continue",
        ),
        Err(IdentityError::Invalid) => json_error(
            StatusCode::FORBIDDEN,
            "invalid_credentials",
            "the credentials were refused",
        ),
    }
}

pub async fn require_csrf(
    State(config): State<Arc<AuthConfig>>,
    request: Request,
    next: Next,
) -> Response {
    if matches!(
        request.method(),
        &Method::GET | &Method::HEAD | &Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    if cookie::value_of(request.headers(), &config.session_cookie).is_none() {
        return next.run(request).await;
    }
    let sent: Option<String> = request
        .headers()
        .get(config.csrf_header.as_str())
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let expected = cookie::value_of(request.headers(), &config.csrf_cookie);
    match (sent, expected) {
        (Some(sent), Some(expected)) if sent == expected => next.run(request).await,
        _ => json_error(
            StatusCode::FORBIDDEN,
            "csrf",
            "the request could not be attributed to a session",
        ),
    }
}

pub fn caller_id(caller: &Caller) -> Option<uuid::Uuid> {
    caller.user_id().and_then(|id| id.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DevCallerResolver, IdentityError};
    use axum::http::HeaderMap;
    use axum::http::Request as HttpRequest;
    use axum::routing::{get, post};
    use axum::{Extension, Router};
    use tower::ServiceExt;

    struct AlwaysRefuses;

    #[async_trait::async_trait]
    impl CallerResolver for AlwaysRefuses {
        async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
            Err(IdentityError::Invalid)
        }
    }

    struct RecordsRequest;

    #[async_trait::async_trait]
    impl CallerResolver for RecordsRequest {
        async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
            Ok(Caller::User("resolved".to_string()))
        }
    }

    struct NoCredentials;

    #[async_trait::async_trait]
    impl CallerResolver for NoCredentials {
        async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
            Err(IdentityError::Missing)
        }
    }

    async fn echo_caller(caller: Option<Extension<Caller>>) -> Response {
        Json(json!({
            "caller": caller.map(|Extension(value)| value.header_value()),
            "saw_identity_header": false,
        }))
        .into_response()
    }

    async fn saw_header(headers: HeaderMap) -> Response {
        Json(json!({
            "saw_identity_header": headers.get("x-menzi-user-id").is_some(),
        }))
        .into_response()
    }

    fn config() -> Arc<AuthConfig> {
        Arc::new(AuthConfig::for_tests())
    }

    fn protected(resolver: Arc<dyn CallerResolver>) -> Router {
        let config = config();
        let guarded = Router::new()
            .route("/api/v1/me", get(echo_caller))
            .route("/api/v1/thing", post(echo_caller))
            .layer(axum::middleware::from_fn_with_state(
                config.clone(),
                require_csrf,
            ))
            .layer(axum::middleware::from_fn_with_state(
                resolver,
                require_caller,
            ));

        Router::new()
            .route("/health", get(saw_header))
            .route("/api/v1/auth/login", post(echo_caller))
            .merge(guarded)
            .layer(axum::middleware::from_fn(strip_identity_headers))
            .with_state(Arc::new(()))
    }

    async fn body_of(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn call(
        app: Router,
        method: &str,
        uri: &str,
        cookie: Option<&str>,
        csrf: Option<&str>,
        spoof: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = HttpRequest::builder().method(method).uri(uri);
        if let Some(value) = cookie {
            builder = builder.header("cookie", value);
        }
        if let Some(value) = csrf {
            builder = builder.header("x-menzi-csrf", value);
        }
        if let Some(value) = spoof {
            builder = builder.header("x-menzi-user-id", value);
        }
        let response = app
            .oneshot(builder.body(axum::body::Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        (status, body_of(response).await)
    }

    #[tokio::test]
    async fn the_health_check_is_not_gated() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, body) = call(app, "GET", "/health", None, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["saw_identity_header"], false);
    }

    #[tokio::test]
    async fn login_is_reachable_without_a_session() {
        let app = protected(Arc::new(AlwaysRefuses));
        let (status, _) = call(app, "POST", "/api/v1/auth/login", None, None, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_protected_route_without_a_session_is_unauthenticated() {
        let app = protected(Arc::new(NoCredentials));
        let (status, body) = call(app, "GET", "/api/v1/me", None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthenticated");
    }

    #[tokio::test]
    async fn a_refused_credential_is_forbidden_not_unauthenticated() {
        let app = protected(Arc::new(AlwaysRefuses));
        let (status, body) = call(app, "GET", "/api/v1/me", None, None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "invalid_credentials");
    }

    #[tokio::test]
    async fn a_resolved_caller_reaches_the_handler() {
        let app = protected(Arc::new(RecordsRequest));
        let (status, body) = call(app, "GET", "/api/v1/me", None, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["caller"], "resolved");
    }

    #[tokio::test]
    async fn the_development_resolver_reaches_the_handler() {
        let app = protected(Arc::new(DevCallerResolver::new("dev-user")));
        let (status, body) = call(app, "GET", "/api/v1/me", None, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["caller"], "dev-user");
    }

    #[tokio::test]
    async fn a_client_supplied_identity_header_is_removed() {
        let app = protected(Arc::new(DevCallerResolver::new("real-user")));
        let (status, body) =
            call(app, "GET", "/api/v1/me", None, None, Some("someone-elses")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["caller"], "real-user");
    }

    #[tokio::test]
    async fn a_spoofed_header_never_reaches_a_public_handler() {
        let app = protected(Arc::new(DevCallerResolver::new("real-user")));
        let (status, body) = call(app, "GET", "/health", None, None, Some("attacker")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["saw_identity_header"], false);
    }

    #[tokio::test]
    async fn a_post_with_a_session_cookie_but_no_csrf_header_is_refused() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, body) = call(
            app,
            "POST",
            "/api/v1/thing",
            Some("menzi_session=mz_abc; menzi_csrf=tok"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "csrf");
    }

    #[tokio::test]
    async fn a_post_with_a_mismatched_csrf_header_is_refused() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, _) = call(
            app,
            "POST",
            "/api/v1/thing",
            Some("menzi_session=mz_abc; menzi_csrf=tok"),
            Some("other"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_post_without_a_session_cookie_needs_no_csrf_header() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, body) = call(app, "POST", "/api/v1/thing", None, None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["caller"], "dev");
    }

    #[tokio::test]
    async fn a_post_with_a_session_cookie_needs_a_csrf_header() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, body) = call(
            app,
            "POST",
            "/api/v1/thing",
            Some("menzi_session=mz_abc"),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "csrf");
    }

    #[tokio::test]
    async fn a_post_with_a_matching_csrf_header_succeeds() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, body) = call(
            app,
            "POST",
            "/api/v1/thing",
            Some("menzi_session=mz_abc; menzi_csrf=tok"),
            Some("tok"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["caller"], "dev");
    }

    #[tokio::test]
    async fn a_get_needs_no_csrf_header() {
        let app = protected(Arc::new(DevCallerResolver::new("dev")));
        let (status, _) = call(app, "GET", "/api/v1/me", None, None, None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn an_unauthenticated_post_is_401_not_403() {
        let app = protected(Arc::new(NoCredentials));
        let (status, body) = call(app, "POST", "/api/v1/thing", None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"]["code"], "unauthenticated");
    }

    #[tokio::test]
    async fn a_refused_credential_is_403_on_a_post() {
        let app = protected(Arc::new(AlwaysRefuses));
        let (status, body) = call(app, "POST", "/api/v1/thing", None, None, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"]["code"], "invalid_credentials");
    }

    #[test]
    fn the_csrf_names_come_from_the_configuration() {
        let config = AuthConfig::for_tests();
        assert_eq!(config.csrf_header, "x-menzi-csrf");
        assert_eq!(config.csrf_cookie, "menzi_csrf");
    }

    #[test]
    fn a_caller_id_parses_into_a_uuid() {
        let caller = Caller::User("11111111-1111-1111-1111-111111111111".to_string());
        assert!(caller_id(&caller).is_some());
        let service = Caller::Service("session-proxy".to_string());
        assert_eq!(caller_id(&service), None);
        let garbage = Caller::User("not-a-uuid".to_string());
        assert_eq!(caller_id(&garbage), None);
    }
}
