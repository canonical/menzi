use axum::body::Body;
use axum::extract::Extension;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct PreviewProxy {
    base_url: String,
    client: reqwest::Client,
}

impl PreviewProxy {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env() -> Self {
        let base_url = std::env::var("MENZI_PREVIEWS_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8095".to_string());
        Self::new(base_url)
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "host"
    )
}

pub async fn forward_previews(
    method: Method,
    headers: HeaderMap,
    uri: Uri,
    Extension(proxy): Extension<Arc<PreviewProxy>>,
    body: Body,
) -> Response {
    let path = uri
        .path_and_query()
        .map(|path_and_query| path_and_query.as_str())
        .unwrap_or_else(|| uri.path());
    let target = format!("{}{}", proxy.base_url, path);
    let bytes = match axum::body::to_bytes(body, 64 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "unreadable request body"})),
            )
                .into_response();
        }
    };
    let mut forwarded = proxy.client.request(method, &target);
    for (name, value) in headers.iter() {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            forwarded = forwarded.header(name, value);
        }
    }
    if !bytes.is_empty() {
        forwarded = forwarded.body(bytes.to_vec());
    }
    match forwarded.send().await {
        Ok(response) => {
            let status = response.status();
            let response_bytes = response.bytes().await.unwrap_or_default();
            (status, response_bytes.to_vec()).into_response()
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": format!("previews unreachable: {error}")})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use axum::Router;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    fn lazy_pool() -> sqlx::postgres::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .expect("lazy pool")
    }

    async fn spawn_fake_previews() -> String {
        let app = Router::new()
            .route(
                "/api/v1/projects/{project_id}/previews",
                get(|| async { Json(json!([{"id": "prv-1", "status": "ready"}])) }),
            )
            .route(
                "/api/v1/previews",
                post(|| async {
                    (
                        StatusCode::CREATED,
                        Json(json!({"id": "prv-2", "status": "ready", "mode": "pinned"})),
                    )
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("fake previews server");
        });
        format!("http://{}", address)
    }

    async fn core_router(proxy: Arc<PreviewProxy>) -> Router<()> {
        crate::create_router()
            .layer(Extension(proxy))
            .with_state(lazy_pool())
    }

    #[tokio::test]
    async fn forwards_list_to_previews_service() {
        let base = spawn_fake_previews().await;
        let app = core_router(Arc::new(PreviewProxy::new(base))).await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/previews")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body[0]["id"], "prv-1");
    }

    #[tokio::test]
    async fn forwards_create_to_previews_service() {
        let base = spawn_fake_previews().await;
        let app = core_router(Arc::new(PreviewProxy::new(base))).await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/previews")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"project_id":"p1","mode":"pinned"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["id"], "prv-2");
    }

    #[tokio::test]
    async fn returns_gateway_error_when_previews_down() {
        let app = core_router(Arc::new(PreviewProxy::new("http://127.0.0.1:1"))).await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/previews")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }
}
