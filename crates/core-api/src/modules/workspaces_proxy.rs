use axum::body::Body;
use axum::extract::Extension;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::sync::Arc;

use crate::identity::{resolver_from_env, Caller, CallerResolver, IdentityError, IDENTITY_HEADERS};

#[derive(Clone)]
pub struct WorkspaceProxy {
    base_url: String,
    client: reqwest::Client,
    resolver: Arc<dyn CallerResolver>,
}

impl WorkspaceProxy {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            resolver: resolver_from_env(
                sqlx::postgres::PgPoolOptions::new()
                    .max_connections(2)
                    .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
                    .expect("lazy pool"),
            ),
        }
    }

    pub fn with_resolver(mut self, resolver: Arc<dyn CallerResolver>) -> Self {
        self.resolver = resolver;
        self
    }

    /// The one resolver the process uses, so every surface that asks "who is
    /// calling" answers from the same source.
    pub fn caller_resolver(&self) -> Arc<dyn CallerResolver> {
        self.resolver.clone()
    }

    pub fn from_env() -> Self {
        let base_url = std::env::var("MENZI_WORKSPACE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8096".to_string());
        let pool = std::env::var("MENZI_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://unused:unused@127.0.0.1:1/unused".to_string());
        let resolver = resolver_from_env(
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(4)
                .connect_lazy(&pool)
                .expect("lazy pool"),
        );
        Self {
            base_url,
            client: reqwest::Client::new(),
            resolver,
        }
    }
}

const HOP_BY_HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "content-length",
    "host",
];

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.contains(&name)
}

fn identity_status(error: IdentityError) -> StatusCode {
    match error {
        IdentityError::Missing => StatusCode::UNAUTHORIZED,
        IdentityError::Invalid => StatusCode::FORBIDDEN,
    }
}

pub async fn forward_workspaces(
    method: Method,
    headers: HeaderMap,
    uri: Uri,
    Extension(proxy): Extension<Arc<WorkspaceProxy>>,
    body: Body,
) -> Response {
    let caller = match proxy.resolver.resolve(&headers).await {
        Ok(caller) => caller,
        Err(error) => {
            return (
                identity_status(error),
                Json(json!({ "error": "who is calling is not established" })),
            )
                .into_response();
        }
    };
    forward_with_caller(&proxy, caller, method, headers, uri, body).await
}

async fn forward_with_caller(
    proxy: &WorkspaceProxy,
    caller: Caller,
    method: Method,
    headers: HeaderMap,
    uri: Uri,
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
        if is_hop_by_hop(name.as_str()) || IDENTITY_HEADERS.contains(&name.as_str()) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            forwarded = forwarded.header(name, value);
        }
    }
    forwarded = forwarded.header(caller.header_name(), caller.header_value());
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
            Json(json!({"error": format!("workspace service unreachable: {error}")})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DevCallerResolver;
    use axum::routing::{get, post};
    use axum::Router;
    use tokio::net::TcpListener;
    use tower::ServiceExt;

    struct EchoIdentity;

    #[async_trait::async_trait]
    impl CallerResolver for EchoIdentity {
        async fn resolve(&self, headers: &HeaderMap) -> Result<Caller, IdentityError> {
            match headers.get("authorization").and_then(|v| v.to_str().ok()) {
                Some("Bearer dev") => Ok(Caller::User("resolved-user".to_string())),
                Some(_) => Err(IdentityError::Invalid),
                None => Err(IdentityError::Missing),
            }
        }
    }

    fn lazy_pool() -> sqlx::postgres::PgPool {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .expect("lazy pool")
    }

    async fn spawn_fake_workspaces() -> String {
        let app = Router::new()
            .route(
                "/api/v1/projects/{project_id}/workspaces",
                get(|| async { Json(json!([{"instance_name": "wsp-a-b", "status": "ready"}])) }),
            )
            .route(
                "/api/v1/workspaces",
                post(|| async {
                    (
                        StatusCode::OK,
                        Json(json!({"instance_name": "wsp-a-b", "status": "ready"})),
                    )
                }),
            );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("fake workspaces server");
        });
        format!("http://{}", address)
    }

    async fn core_router(proxy: Arc<WorkspaceProxy>) -> Router {
        crate::create_router()
            .layer(Extension(proxy))
            .with_state(lazy_pool())
    }

    async fn json_of(response: Response) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn forwards_list_to_workspace_service() {
        let base = spawn_fake_workspaces().await;
        let app = core_router(Arc::new(
            WorkspaceProxy::new(base).with_resolver(Arc::new(DevCallerResolver::new("dev-user"))),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/workspaces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert_eq!(body[0]["instance_name"], "wsp-a-b");
    }

    #[tokio::test]
    async fn forwards_ensure_with_its_body() {
        let base = spawn_fake_workspaces().await;
        let app = core_router(Arc::new(
            WorkspaceProxy::new(base).with_resolver(Arc::new(DevCallerResolver::new("dev-user"))),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/workspaces")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"project_id":"p1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn forwards_a_nested_workspace_sub_route() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recorder = seen.clone();
        let nested = Router::new().route(
            "/api/v1/workspaces/{user}/{project}/sessions/{session}/workspace",
            get(
                move |axum::extract::Path((user, project, session)): axum::extract::Path<(
                    String,
                    String,
                    String,
                )>| {
                    let recorder = recorder.clone();
                    async move {
                        recorder.lock().unwrap().push(session);
                        Json(json!({ "endpoint": format!("http://{user}/{project}:17999") }))
                    }
                },
            ),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, nested).await.unwrap();
        });

        let app = core_router(Arc::new(
            WorkspaceProxy::new(format!("http://{address}"))
                .with_resolver(Arc::new(DevCallerResolver::new("dev-user"))),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/workspaces/u1/p1/sessions/ses_9/workspace")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json_of(response).await;
        assert_eq!(body["endpoint"], "http://u1/p1:17999");
        assert_eq!(*seen.lock().unwrap(), vec!["ses_9".to_string()]);
    }

    #[tokio::test]
    async fn forwards_a_workspace_sub_route() {
        let base = spawn_fake_workspaces().await;
        let app = core_router(Arc::new(
            WorkspaceProxy::new(base).with_resolver(Arc::new(DevCallerResolver::new("dev-user"))),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/workspaces/u1/p1/connect")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn returns_gateway_error_when_the_service_is_down() {
        let app = core_router(Arc::new(
            WorkspaceProxy::new("http://127.0.0.1:1")
                .with_resolver(Arc::new(DevCallerResolver::new("dev-user"))),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/workspaces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn a_request_with_no_identity_is_refused_before_forwarding() {
        let base = spawn_fake_workspaces().await;
        let app = core_router(Arc::new(
            WorkspaceProxy::new(base).with_resolver(Arc::new(EchoIdentity)),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/workspaces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_bad_token_is_forbidden() {
        let base = spawn_fake_workspaces().await;
        let app = core_router(Arc::new(
            WorkspaceProxy::new(base).with_resolver(Arc::new(EchoIdentity)),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/workspaces")
                    .header("authorization", "Bearer nope")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_client_supplied_identity_is_replaced_not_forwarded() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let recorder = seen.clone();
        let app_local = Router::new().route(
            "/api/v1/projects/p1/workspaces",
            get(move |headers: HeaderMap| {
                let recorder = recorder.clone();
                async move {
                    let user = headers
                        .get("x-menzi-user-id")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    recorder.lock().unwrap().push(user);
                    Json(json!([{"id": "wsp-a-b"}]))
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app_local).await.unwrap();
        });

        let app = core_router(Arc::new(
            WorkspaceProxy::new(format!("http://{address}")).with_resolver(Arc::new(EchoIdentity)),
        ))
        .await;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/projects/p1/workspaces")
                    .header("authorization", "Bearer dev")
                    .header("x-menzi-user-id", "someone-elses")
                    .header("x-menzi-service", "autonomous")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let recorded = seen.lock().unwrap().clone();
        assert_eq!(recorded, vec!["resolved-user".to_string()]);
    }
}
