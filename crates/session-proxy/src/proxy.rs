use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use menzi_common::ids::SessionId;
use serde_json::json;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::config::ProxyConfig;
use crate::fanout::FanoutManager;
use crate::transcript::TranscriptArchiver;
use crate::tunnel::{MessageType, TunnelConnection, TunnelManager, TunnelMessage};

#[derive(Clone)]
pub struct ProxyState {
    pub http: reqwest::Client,
    pub config: ProxyConfig,
    pub opencode_url: Arc<RwLock<String>>,
    pub tunnels: Arc<Mutex<TunnelManager>>,
    pub archiver: Arc<Mutex<TranscriptArchiver>>,
    pub fanout: Arc<Mutex<FanoutManager>>,
    pub sequence: Arc<AtomicI64>,
}

impl ProxyState {
    pub fn new(config: ProxyConfig, opencode_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
            opencode_url: Arc::new(RwLock::new(opencode_url.into())),
            tunnels: Arc::new(Mutex::new(TunnelManager::new())),
            archiver: Arc::new(Mutex::new(TranscriptArchiver::new(
                "/tmp/menzi/transcripts",
            ))),
            fanout: Arc::new(Mutex::new(FanoutManager::new())),
            sequence: Arc::new(AtomicI64::new(0)),
        }
    }
}

pub fn create_router(state: ProxyState) -> Router {
    Router::new()
        .route("/api/opencode/register", post(register_opencode))
        .route("/api/tunnel/register", post(register_tunnel))
        .route("/api/tunnel/{session_id}/status", get(tunnel_status))
        .route("/api/tunnel/{session_id}/fanout", post(fanout_subscribe))
        .route(
            "/api/tunnel/{session_id}/transcript",
            get(transcript_events),
        )
        .route("/api/{*rest}", any(forward))
        .with_state(state)
}

async fn register_opencode(
    State(state): State<ProxyState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(url) = body.get("url").and_then(|value| value.as_str()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "url is required"})),
        )
            .into_response();
    };
    if url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "url is required"})),
        )
            .into_response();
    }
    *state.opencode_url.write().await = url.to_string();
    (
        StatusCode::OK,
        Json(json!({"status": "registered", "url": url})),
    )
        .into_response()
}

async fn register_tunnel(
    State(state): State<ProxyState>,
    Json(tunnel): Json<TunnelConnection>,
) -> Response {
    {
        let mut tunnels = state.tunnels.lock().expect("tunnels lock");
        tunnels.register_tunnel(tunnel.clone());
    }
    record_event(
        &state,
        tunnel.session_id,
        MessageType::Event,
        json!({"event": "session.tunnel_connected"}),
    );
    (StatusCode::OK, Json(json!({"status": "registered"}))).into_response()
}

async fn tunnel_status(
    State(state): State<ProxyState>,
    Path(session_id): Path<SessionId>,
) -> Response {
    let tunnel = {
        let tunnels = state.tunnels.lock().expect("tunnels lock");
        tunnels.get_tunnel(&session_id).cloned()
    };
    match tunnel {
        Some(tunnel) => (StatusCode::OK, Json(tunnel)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "tunnel not found"})),
        )
            .into_response(),
    }
}

async fn fanout_subscribe(
    State(state): State<ProxyState>,
    Path(session_id): Path<SessionId>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(viewer_id) = body.get("viewer_id").and_then(|value| value.as_str()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "viewer_id is required"})),
        )
            .into_response();
    };
    {
        let mut tunnels = state.tunnels.lock().expect("tunnels lock");
        tunnels.add_viewer(&session_id, viewer_id.to_string());
    }
    {
        let mut fanout = state.fanout.lock().expect("fanout lock");
        fanout.subscribe(session_id, viewer_id.to_string());
    }
    record_event(
        &state,
        session_id,
        MessageType::Event,
        json!({"event": "fanout.subscribed", "viewer_id": viewer_id}),
    );
    (StatusCode::OK, Json(json!({"status": "subscribed"}))).into_response()
}

async fn transcript_events(
    State(state): State<ProxyState>,
    Path(session_id): Path<SessionId>,
) -> Response {
    let events = {
        let archiver = state.archiver.lock().expect("archiver lock");
        archiver
            .get_events(&session_id)
            .cloned()
            .unwrap_or_default()
    };
    (StatusCode::OK, Json(events)).into_response()
}

async fn forward(
    State(state): State<ProxyState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !state.config.is_allowed(method.as_str(), uri.path()) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "route not allowed"})),
        )
            .into_response();
    }

    let target = state.opencode_url.read().await.clone();
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or(uri.path());
    let url = format!("{target}{path_and_query}");

    let mut builder = state.http.request(method, url);
    for (name, value) in headers.iter() {
        if name == header::HOST
            || name == header::CONTENT_LENGTH
            || name == header::TRANSFER_ENCODING
            || name == header::CONNECTION
        {
            continue;
        }
        builder = builder.header(name, value);
    }
    let upstream = match builder.body(body).send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": error.to_string()})),
            )
                .into_response();
        }
    };

    let status = upstream.status();
    let forwarded_headers = upstream.headers().clone();
    let bytes = match upstream.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": error.to_string()})),
            )
                .into_response();
        }
    };

    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    *response.status_mut() = status;
    for (name, value) in forwarded_headers.iter() {
        if name == header::CONTENT_LENGTH
            || name == header::TRANSFER_ENCODING
            || name == header::CONNECTION
        {
            continue;
        }
        response.headers_mut().insert(name.clone(), value.clone());
    }
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).expect("valid content length"),
    );
    response
}

fn record_event(
    state: &ProxyState,
    session_id: SessionId,
    message_type: MessageType,
    payload: serde_json::Value,
) {
    let sequence = state.sequence.fetch_add(1, Ordering::Relaxed);
    let message = TunnelMessage {
        message_type,
        session_id,
        payload,
        sequence,
    };
    let mut archiver = state.archiver.lock().expect("archiver lock");
    archiver.record_event(session_id, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    fn test_state() -> ProxyState {
        ProxyState::new(ProxyConfig::default_allowlist(), "http://127.0.0.1:17999")
    }

    async fn start_fake_opencode() -> String {
        let app = Router::new()
            .route(
                "/api/session",
                get(|| async { Json(json!({"provider": "opencode"})) }),
            )
            .route(
                "/api/session/{id}/log",
                get(|| async { Json(json!({"lines": ["ok"]})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    async fn body_string(response: Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[tokio::test]
    async fn register_opencode_sets_proxy_target() {
        let state = test_state();
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/opencode/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"url": "http://127.0.0.1:19999"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let url = state.opencode_url.read().await.clone();
        assert_eq!(url, "http://127.0.0.1:19999");
    }

    #[tokio::test]
    async fn register_opencode_rejects_empty_url() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/opencode/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&json!({"url": ""})).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn forward_forwards_allowed_route_to_opencode() {
        let target = start_fake_opencode().await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), target);
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("opencode"));
    }

    #[tokio::test]
    async fn forward_forwards_parameterized_allowed_route() {
        let target = start_fake_opencode().await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), target);
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/session/abc123/log")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("ok"));
    }

    #[tokio::test]
    async fn forward_denies_blacklisted_route() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn forward_rejects_unknown_route() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/unknown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn register_tunnel_records_transcript() {
        let state = test_state();
        let app = create_router(state.clone());
        let session_id = SessionId::new();
        let tunnel = TunnelConnection {
            session_id,
            instance_name: "mz-dev".to_string(),
            status: crate::tunnel::TunnelStatus::Connected,
            connected_at: "2026-09-28T00:00:00Z".to_string(),
            last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
        };
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tunnel/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&tunnel).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let count = {
            let archiver = state.archiver.lock().unwrap();
            archiver.get_event_count(&session_id)
        };
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn tunnel_status_returns_registered_tunnel() {
        let state = test_state();
        let session_id = SessionId::new();
        {
            let mut tunnels = state.tunnels.lock().unwrap();
            tunnels.register_tunnel(TunnelConnection {
                session_id,
                instance_name: "mz-dev".to_string(),
                status: crate::tunnel::TunnelStatus::Connected,
                connected_at: "2026-09-28T00:00:00Z".to_string(),
                last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
            });
        }
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{session_id}/status"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("mz-dev"));
    }

    #[tokio::test]
    async fn tunnel_status_returns_not_found_for_unknown() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{}/status", SessionId::new()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn fanout_subscribe_adds_viewer() {
        let state = test_state();
        let session_id = SessionId::new();
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/tunnel/{session_id}/fanout"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"viewer_id": "viewer-1"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let count = {
            let tunnels = state.tunnels.lock().unwrap();
            tunnels.get_viewer_count(&session_id)
        };
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn transcript_events_returns_recorded_events() {
        let state = test_state();
        let session_id = SessionId::new();
        let app = create_router(state.clone());
        {
            let mut tunnels = state.tunnels.lock().unwrap();
            tunnels.register_tunnel(TunnelConnection {
                session_id,
                instance_name: "mz-dev".to_string(),
                status: crate::tunnel::TunnelStatus::Connected,
                connected_at: "2026-09-28T00:00:00Z".to_string(),
                last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
            });
        }
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tunnel/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&TunnelConnection {
                            session_id,
                            instance_name: "mz-dev".to_string(),
                            status: crate::tunnel::TunnelStatus::Connected,
                            connected_at: "2026-09-28T00:00:00Z".to_string(),
                            last_heartbeat: "2026-09-28T00:00:00Z".to_string(),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{session_id}/transcript"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("tunnel_connected"));
    }
}
