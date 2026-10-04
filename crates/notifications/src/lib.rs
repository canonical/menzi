pub mod dispatch;
pub mod preferences;
pub mod types;

use axum::http::StatusCode;
use axum::routing::get;
use axum::Json;
use serde_json::json;

pub use types::*;

pub fn create_router() -> axum::Router {
    axum::Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
}

async fn health_check() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}

async fn ready_check() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({"status": "ready"})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_and_ready_endpoints_are_available() {
        let app = create_router();
        let health = app
            .clone()
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        let ready = app
            .oneshot(Request::builder().uri("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(ready.status(), StatusCode::OK);
    }
}
