use axum::extract::Extension;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;

use crate::identity::{Caller, CallerResolver, IdentityError, HEADER_USER_ID};

#[derive(Serialize)]
pub struct CurrentUser {
    pub id: Option<String>,
    pub kind: &'static str,
}

pub async fn whoami(
    Extension(resolver): Extension<Arc<dyn CallerResolver>>,
    headers: HeaderMap,
) -> Response {
    match resolver.resolve(&headers).await {
        Ok(Caller::User(id)) => Json(CurrentUser {
            id: Some(id),
            kind: "user",
        })
        .into_response(),
        Ok(Caller::Service(name)) => (
            StatusCode::OK,
            Json(json!({ "id": name, "kind": "service" })),
        )
            .into_response(),
        Err(IdentityError::Missing) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "no credentials" })),
        )
            .into_response(),
        Err(IdentityError::Invalid) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "the credentials were refused" })),
        )
            .into_response(),
    }
}

pub const USER_ID_HEADER: &str = HEADER_USER_ID;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DevCallerResolver;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn app(resolver: Arc<dyn CallerResolver>) -> Router {
        Router::new()
            .route("/api/v1/me", get(whoami))
            .layer(Extension(resolver))
    }

    async fn body_of(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn it_answers_with_the_resolved_user() {
        let response = app(Arc::new(DevCallerResolver::new("u-42")))
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/me")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_of(response).await;
        assert_eq!(body["id"], "u-42");
        assert_eq!(body["kind"], "user");
    }

    #[tokio::test]
    async fn it_refuses_a_caller_it_cannot_resolve() {
        let response = app(Arc::new(DevCallerResolver::new("")))
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/me")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn the_user_id_header_is_the_one_the_workspace_service_reads() {
        assert_eq!(USER_ID_HEADER, "x-menzi-user-id");
    }
}
