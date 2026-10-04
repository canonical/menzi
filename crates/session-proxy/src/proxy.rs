use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use futures_util::SinkExt;
use menzi_common::ids::SessionId;
use serde_json::json;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as UpstreamMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

use crate::auth;
use crate::config::ProxyConfig;
use crate::fanout::FanoutManager;
use crate::router::{
    endpoint_override, is_capability_path, session_from_path, session_hint, InMemorySessionRouter,
    SessionRouter,
};
use crate::transcript::TranscriptArchiver;
use crate::tunnel::{MessageType, TunnelConnection, TunnelManager, TunnelMessage};

#[derive(Clone)]
pub struct ProxyState {
    pub http: reqwest::Client,
    pub config: ProxyConfig,
    pub opencode_url: Arc<RwLock<String>>,
    pub shared: Arc<RwLock<Option<String>>>,
    pub tunnels: Arc<Mutex<TunnelManager>>,
    pub archiver: Arc<Mutex<TranscriptArchiver>>,
    pub fanout: Arc<Mutex<FanoutManager>>,
    pub sequence: Arc<AtomicI64>,
    pub events: broadcast::Sender<TunnelMessage>,
    pub router: Arc<dyn SessionRouter>,
    pub ptys: Arc<Mutex<HashMap<String, String>>>,
}

impl ProxyState {
    pub fn new(config: ProxyConfig, opencode_url: impl Into<String>) -> Self {
        let (events, _) = broadcast::channel(1024);
        Self {
            http: reqwest::Client::new(),
            config,
            opencode_url: Arc::new(RwLock::new(opencode_url.into())),
            shared: Arc::new(RwLock::new(None)),
            tunnels: Arc::new(Mutex::new(TunnelManager::new())),
            archiver: Arc::new(Mutex::new(TranscriptArchiver::new(
                "/tmp/menzi/transcripts",
            ))),
            fanout: Arc::new(Mutex::new(FanoutManager::new())),
            sequence: Arc::new(AtomicI64::new(0)),
            events,
            router: Arc::new(InMemorySessionRouter::new()),
            ptys: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_router(mut self, router: Arc<dyn SessionRouter>) -> Self {
        self.router = router;
        self
    }

    pub async fn target_for(&self, headers: &HeaderMap, path: &str) -> Option<String> {
        if let Some(endpoint) = endpoint_override(headers) {
            return Some(endpoint);
        }
        match session_from_path(path) {
            Some(session) => self.target_for_session(&session).await,
            None => Some(self.fallback().await),
        }
    }

    pub async fn fallback(&self) -> String {
        self.opencode_url.read().await.clone()
    }

    pub async fn shared_endpoint(&self) -> Option<String> {
        self.shared.read().await.clone().into()
    }

    pub async fn register_shared(&self, url: String) {
        *self.shared.write().await = url.into();
    }

    pub async fn target_for_session(&self, session: &str) -> Option<String> {
        if session.is_empty() {
            return None;
        }
        self.router.endpoint_for(session).await
    }
}

pub fn create_router(state: ProxyState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check))
        .route("/api/opencode/register", post(register_opencode))
        .route("/api/opencode/bind", post(bind_session))
        .route("/api/tunnel/register", post(register_tunnel))
        .route("/api/tunnel/sessions", get(list_sessions))
        .route("/api/oc/event", get(stream_upstream_event))
        .route("/api/tunnel/{session_id}/status", get(tunnel_status))
        .route("/api/tunnel/{session_id}/fanout", post(fanout_subscribe))
        .route("/api/pty/{pty_id}/connect", get(connect_pty))
        .route(
            "/api/tunnel/{session_id}/transcript",
            get(transcript_events),
        )
        .route("/api/{*rest}", any(forward))
        .route("/{*rest}", any(forward))
        .with_state(state)
}

async fn health_check(State(state): State<ProxyState>) -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "shared_opencode": state.shared_endpoint().await,
            "tunnels": state.tunnels.lock().expect("tunnels lock").tunnels.len(),
        })),
    )
        .into_response()
}

async fn ready_check() -> Response {
    (StatusCode::OK, Json(json!({"status": "ready"}))).into_response()
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
    state.register_shared(url.to_string()).await;
    (
        StatusCode::OK,
        Json(json!({"status": "registered", "url": url})),
    )
        .into_response()
}

async fn bind_session(
    State(state): State<ProxyState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let (Some(session), Some(endpoint)) = (
        body.get("session").and_then(|value| value.as_str()),
        body.get("endpoint").and_then(|value| value.as_str()),
    ) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "session and endpoint are required"})),
        )
            .into_response();
    };
    if session.is_empty() || endpoint.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "session and endpoint are required"})),
        )
            .into_response();
    }
    state.router.bind(session, endpoint);
    (
        StatusCode::OK,
        Json(json!({"status": "bound", "session": session, "endpoint": endpoint})),
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

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct TranscriptQuery {
    pub after: Option<i64>,
    pub follow: Option<bool>,
}

fn backlog(state: &ProxyState, session_id: &SessionId, after: i64) -> Vec<TunnelMessage> {
    let archiver = state.archiver.lock().expect("archiver lock");
    archiver
        .get_events(session_id)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|message| message.sequence > after)
        .collect()
}

fn message_type_name(message_type: MessageType) -> String {
    serde_json::to_value(message_type)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn to_sse_event(message: &TunnelMessage) -> Result<Event, std::convert::Infallible> {
    let data = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string());
    Ok(Event::default()
        .id(message.sequence.to_string())
        .event(message_type_name(message.message_type))
        .data(data))
}

async fn list_sessions(State(state): State<ProxyState>) -> Response {
    let mut sessions: Vec<serde_json::Value> = {
        let archiver = state.archiver.lock().expect("archiver lock");
        archiver
            .sessions
            .iter()
            .map(|(id, events)| {
                serde_json::json!({
                    "id": id.to_string(),
                    "events_count": events.len(),
                    "last_sequence": events.last().map(|e| e.sequence).unwrap_or(0),
                })
            })
            .collect()
    };
    sessions.sort_by(|a, b| {
        let left = a["last_sequence"].as_i64().unwrap_or(0);
        let right = b["last_sequence"].as_i64().unwrap_or(0);
        right.cmp(&left)
    });
    (StatusCode::OK, Json(json!({ "sessions": sessions }))).into_response()
}

async fn transcript_events(
    State(state): State<ProxyState>,
    Path(session_id): Path<SessionId>,
    Query(query): Query<TranscriptQuery>,
) -> Response {
    let after = query.after.unwrap_or(-1);

    if query.follow != Some(true) {
        let events = backlog(&state, &session_id, after);
        return (StatusCode::OK, Json(events)).into_response();
    }

    let receiver = state.events.subscribe();
    let pending = backlog(&state, &session_id, after);
    let session = session_id;

    let stream = futures_util::stream::unfold(
        (pending.into_iter(), receiver),
        move |(mut backlog, mut receiver)| async move {
            if let Some(message) = backlog.next() {
                let event = to_sse_event(&message);
                return Some((event, (backlog, receiver)));
            }
            loop {
                match receiver.recv().await {
                    Ok(message) => {
                        if message.session_id != session {
                            continue;
                        }
                        let event = to_sse_event(&message);
                        return Some((event, (backlog, receiver)));
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub fn upstream_credentials() -> Option<(String, String)> {
    let username = std::env::var("MENZI_OPENCODE_USERNAME")
        .ok()
        .or_else(|| std::env::var("OPENCODE_SERVER_USERNAME").ok())?;
    let password = std::env::var("MENZI_OPENCODE_PASSWORD")
        .ok()
        .or_else(|| std::env::var("OPENCODE_SERVER_PASSWORD").ok())?;
    if username.is_empty() {
        return None;
    }
    Some((username, password))
}

#[derive(serde::Deserialize)]
struct EventStreamQuery {
    session: Option<String>,
}

async fn stream_upstream_event(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<EventStreamQuery>,
) -> Response {
    let target = match endpoint_override(&headers) {
        Some(endpoint) => endpoint,
        None => match state
            .target_for_session(query.session.as_deref().unwrap_or_default())
            .await
        {
            Some(endpoint) => endpoint,
            None => state.opencode_url.read().await.clone(),
        },
    };
    let url = format!("{target}/api/event");

    let upstream = match state.http.get(&url).send().await {
        Ok(response) => response,
        Err(error) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": error.to_string()})),
            )
                .into_response();
        }
    };

    if upstream.status() != StatusCode::OK {
        let status = upstream.status();
        return (
            status,
            Json(json!({"error": "upstream event stream unavailable"})),
        )
            .into_response();
    }

    let stream = upstream
        .bytes_stream()
        .map(|item| item.map_err(|error| std::io::Error::other(error.to_string())));

    let mut response = Response::new(axum::body::Body::from_stream(stream));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "no opencode serves this session"})),
    )
        .into_response()
}

fn pty_id_from_path(path: &str) -> Option<String> {
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    match (parts.next(), parts.next(), parts.next()) {
        (Some("api"), Some("pty"), Some(id)) if id.starts_with("pty") => Some(id.to_string()),
        _ => None,
    }
}

fn ws_target(target: &str) -> String {
    if let Some(rest) = target.strip_prefix("https://") {
        return format!("wss://{rest}");
    }
    if let Some(rest) = target.strip_prefix("http://") {
        return format!("ws://{rest}");
    }
    target.to_string()
}

fn remember_pty(state: &ProxyState, pty_id: &str, endpoint: &str) {
    state
        .ptys
        .lock()
        .expect("pty route lock")
        .insert(pty_id.to_string(), endpoint.to_string());
}

fn remove_pty(state: &ProxyState, pty_id: &str) {
    state.ptys.lock().expect("pty route lock").remove(pty_id);
}

fn pty_target(state: &ProxyState, pty_id: &str) -> Option<String> {
    state
        .ptys
        .lock()
        .expect("pty route lock")
        .get(pty_id)
        .cloned()
}

fn pty_id_from_response(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    value
        .get("data")
        .and_then(|data| data.get("id"))
        .and_then(|id| id.as_str())
        .map(str::to_string)
        .filter(|id| id.starts_with("pty"))
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

    if auth::is_guarded(uri.path()) && !auth::has_credential(&headers) {
        return auth::unauthorized();
    }

    let query = uri.query().map(|value| value.to_string());
    let path_session = session_from_path(uri.path()).map(str::to_string);
    let capability_session = session_hint(uri.path(), query.as_deref());
    let session = path_session.clone().or(capability_session.clone());

    let override_target = endpoint_override(&headers);
    let mapped_pty_target = pty_id_from_path(uri.path()).and_then(|pty_id| pty_target(&state, &pty_id));

    let target = match override_target.clone() {
        Some(override_endpoint) => override_endpoint,
        None => match mapped_pty_target {
            Some(mapped) => mapped,
            None => match (
            session,
            is_capability_path(uri.path()),
            path_session.is_some(),
        ) {
            (Some(session), _, _) => match state.target_for_session(&session).await {
                Some(endpoint) => endpoint,
                None => return not_found(),
            },
            (None, true, false) => match state.shared_endpoint().await {
                Some(url) => url,
                None => return not_found(),
            },
            (None, _, true) => return not_found(),
            (None, _, false) => state.fallback().await,
        }},
    };

    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or(uri.path());
    let url = format!("{target}{path_and_query}");

    let mut builder = state.http.request(method.clone(), url);
    for (name, value) in auth::strip_identity(&headers).iter() {
        if name == header::HOST
            || name == header::CONTENT_LENGTH
            || name == header::TRANSFER_ENCODING
            || name == header::CONNECTION
        {
            continue;
        }
        builder = builder.header(name, value);
    }
    if let Some(credentials) = upstream_credentials() {
        builder = builder.basic_auth(credentials.0, Some(credentials.1));
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

    if method == Method::POST && uri.path() == "/api/pty" && status.is_success() {
        if let Some(pty_id) = pty_id_from_response(&bytes) {
            remember_pty(&state, &pty_id, &target);
        }
    }

    if let Some(pty_id) = pty_id_from_path(uri.path()) {
        if status.is_success() {
            if let Some(override_endpoint) = override_target {
                remember_pty(&state, &pty_id, &override_endpoint);
            }
            if method == Method::DELETE {
                remove_pty(&state, &pty_id);
            }
        }
    }

    response
}

async fn connect_pty(
    State(state): State<ProxyState>,
    Path(pty_id): Path<String>,
    headers: HeaderMap,
    uri: Uri,
    ws: WebSocketUpgrade,
) -> Response {
    if !state.config.is_allowed("GET", uri.path()) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "route not allowed"})),
        )
            .into_response();
    }
    let target = if let Some(override_endpoint) = endpoint_override(&headers) {
        remember_pty(&state, &pty_id, &override_endpoint);
        override_endpoint
    } else if let Some(mapped) = pty_target(&state, &pty_id) {
        mapped
    } else {
        return not_found();
    };
    let upstream = format!("{}{}", ws_target(&target), uri.path_and_query().map(|v| v.as_str()).unwrap_or(uri.path()));
    let credentials = upstream_credentials();

    ws.on_upgrade(move |socket| async move {
        proxy_websocket(socket, upstream, credentials).await;
    })
    .into_response()
}

async fn proxy_websocket(
    mut downstream: WebSocket,
    upstream_url: String,
    credentials: Option<(String, String)>,
) {
    let mut request = match upstream_url.clone().into_client_request() {
        Ok(request) => request,
        Err(_) => {
            let _ = downstream.close().await;
            return;
        }
    };
    if let Some((username, password)) = credentials {
        let token = BASE64.encode(format!("{username}:{password}"));
        if let Ok(value) = HeaderValue::from_str(&format!("Basic {token}")) {
            request.headers_mut().insert(header::AUTHORIZATION, value);
        }
    }

    let Ok((upstream, _)) = connect_async(request).await else {
        let _ = downstream.close().await;
        return;
    };
    let (mut upstream_sink, mut upstream_stream) = upstream.split();
    let (mut downstream_sink, mut downstream_stream) = downstream.split();

    let to_upstream = async {
        while let Some(message) = downstream_stream.next().await {
            let Ok(message) = message else { break };
            let mapped = match message {
                Message::Text(text) => UpstreamMessage::Text(text.to_string()),
                Message::Binary(data) => UpstreamMessage::Binary(data.to_vec()),
                Message::Ping(data) => UpstreamMessage::Ping(data.to_vec()),
                Message::Pong(data) => UpstreamMessage::Pong(data.to_vec()),
                Message::Close(_) => {
                    let _ = upstream_sink.send(UpstreamMessage::Close(None)).await;
                    break;
                }
            };
            if upstream_sink.send(mapped).await.is_err() {
                break;
            }
        }
    };

    let to_downstream = async {
        while let Some(message) = upstream_stream.next().await {
            let Ok(message) = message else { break };
            let mapped = match message {
                UpstreamMessage::Text(text) => Message::Text(text.to_string().into()),
                UpstreamMessage::Binary(data) => Message::Binary(data.into()),
                UpstreamMessage::Ping(data) => Message::Ping(data.into()),
                UpstreamMessage::Pong(data) => Message::Pong(data.into()),
                UpstreamMessage::Close(_) => Message::Close(None),
                UpstreamMessage::Frame(_) => continue,
            };
            if downstream_sink.send(mapped).await.is_err() {
                break;
            }
        }
    };

    tokio::select! {
        _ = to_upstream => {}
        _ = to_downstream => {}
    }
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
    {
        let mut archiver = state.archiver.lock().expect("archiver lock");
        archiver.record_event(session_id, message.clone());
    }
    let _ = state.events.send(message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::HEADER_WORKSPACE_ENDPOINT;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;

    use std::time::Duration;
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

    fn authed(method: &str, uri: &str) -> axum::http::Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", "Bearer test-token")
            .body(Body::empty())
            .unwrap()
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
        let router = Arc::new(InMemorySessionRouter::new());
        router.set("abc123", target.clone());
        let state = ProxyState::new(ProxyConfig::default_allowlist(), target).with_router(router);
        let app = create_router(state);
        let response = app
            .oneshot(authed("GET", "/api/session/abc123/log"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("ok"));
    }

    #[tokio::test]
    async fn a_session_read_without_a_credential_is_unauthorized() {
        let target = start_named_opencode("default").await;
        let app = create_router(ProxyState::new(ProxyConfig::default_allowlist(), target));
        for uri in [
            "/session/ses_a/message",
            "/session/ses_a/diff",
            "/api/session/ses_a/diff",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
        }
    }

    #[tokio::test]
    async fn a_control_route_stays_readable_without_a_credential() {
        let target = start_named_opencode("default").await;
        let app = create_router(ProxyState::new(ProxyConfig::default_allowlist(), target));
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/event")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
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

    async fn start_named_opencode(name: &'static str) -> String {
        let app = Router::new()
            .route(
                "/api/session/{id}/log",
                get(move || {
                    let name = name.to_string();
                    async move { Json(json!({"lines": [name]})) }
                }),
            )
            .route(
                "/api/model",
                get(move || {
                    let name = name.to_string();
                    async move { Json(json!({"data": [{"id": name}]})) }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn an_unbound_session_is_not_served_by_the_shared_opencode() {
        let target = start_named_opencode("default").await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), target);
        let app = create_router(state);
        let response = app
            .oneshot(authed("GET", "/api/session/unknown/log"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_capability_read_without_a_session_needs_a_shared_opencode() {
        let state = ProxyState::new(ProxyConfig::default_allowlist(), "http://127.0.0.1:1");
        let app = create_router(state);
        let response = app.oneshot(authed("GET", "/api/model")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_capability_read_without_a_session_uses_a_registered_one() {
        let target = start_named_opencode("shared").await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), "http://127.0.0.1:1");
        let app = create_router(state.clone());
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/opencode/register")
                    .header("content-type", "application/json")
                    .body(Body::from(format!("{{\"url\":\"{target}\"}}")))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app.oneshot(authed("GET", "/api/model")).await.unwrap();
        assert!(body_string(response).await.contains("shared"));
    }

    #[tokio::test]
    async fn a_capability_read_for_a_session_goes_to_that_workspace() {
        let default_target = start_named_opencode("default").await;
        let workspace_target = start_named_opencode("workspace").await;
        let router = Arc::new(InMemorySessionRouter::new());
        router.set("ses_a", workspace_target);
        let state =
            ProxyState::new(ProxyConfig::default_allowlist(), default_target).with_router(router);
        let app = create_router(state);

        let response = app
            .oneshot(authed("GET", "/api/model?session=ses_a"))
            .await
            .unwrap();
        assert!(body_string(response).await.contains("workspace"));
    }

    #[tokio::test]
    async fn a_capability_read_for_an_unknown_session_is_not_found() {
        let default_target = start_named_opencode("default").await;
        let router = Arc::new(InMemorySessionRouter::new());
        let state =
            ProxyState::new(ProxyConfig::default_allowlist(), default_target).with_router(router);
        let app = create_router(state);

        let response = app
            .oneshot(authed("GET", "/api/model?session=ses_zz"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_bound_session_goes_to_its_own_opencode() {
        let default_target = start_named_opencode("default").await;
        let workspace_target = start_named_opencode("workspace").await;
        let router = Arc::new(InMemorySessionRouter::new());
        router.set("ses_a", workspace_target.clone());
        let state = ProxyState::new(ProxyConfig::default_allowlist(), default_target)
            .with_router(router.clone());
        let app = create_router(state);

        let bound = app
            .clone()
            .oneshot(authed("GET", "/api/session/ses_a/log"))
            .await
            .unwrap();
        assert!(body_string(bound).await.contains("workspace"));

        let unbound = app
            .oneshot(authed("GET", "/api/session/ses_b/log"))
            .await
            .unwrap();
        assert_eq!(unbound.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_header_override_wins_over_the_binding() {
        let default_target = start_named_opencode("default").await;
        let override_target = start_named_opencode("override").await;
        let router = Arc::new(InMemorySessionRouter::new());
        router.set("ses_a", default_target);
        let state = ProxyState::new(ProxyConfig::default_allowlist(), "http://127.0.0.1:1")
            .with_router(router);
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/session/ses_a/log")
                    .header("authorization", "Bearer test-token")
                    .header(HEADER_WORKSPACE_ENDPOINT, override_target)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(body_string(response).await.contains("override"));
    }

    #[tokio::test]
    async fn binding_a_session_over_http_routes_the_next_request() {
        let default_target = start_named_opencode("default").await;
        let workspace_target = start_named_opencode("bound").await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), default_target);
        let app = create_router(state);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/opencode/bind")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "session": "ses_c",
                            "endpoint": workspace_target,
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(authed("GET", "/api/session/ses_c/log"))
            .await
            .unwrap();
        assert!(body_string(response).await.contains("bound"));
    }

    #[tokio::test]
    async fn binding_needs_both_fields() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/opencode/bind")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({"session": "ses_d"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    async fn start_event_source(name: &'static str) -> String {
        let app = Router::new()
            .route(
                "/api/event",
                get(move || {
                    let name = name.to_string();
                    async move {
                        let payload = format!("data: {{\"name\":\"{name}\"}}\n\n");
                        let stream = futures_util::stream::once(async move {
                            Ok::<_, std::io::Error>(axum::body::Bytes::from(payload))
                        });
                        let mut response = Response::new(axum::body::Body::from_stream(stream));
                        *response.status_mut() = StatusCode::OK;
                        response.headers_mut().insert(
                            header::CONTENT_TYPE,
                            HeaderValue::from_static("text/event-stream"),
                        );
                        response
                    }
                }),
            )
            .route("/session/{id}/message", get(|| async { Json(json!([])) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn the_event_stream_falls_back_to_the_default_target() {
        let target = start_event_source("default").await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), target);
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/oc/event")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("default"), "{body}");
    }

    #[tokio::test]
    async fn the_event_stream_routes_to_a_bound_session_opencode() {
        let default_target = start_event_source("default").await;
        let workspace_target = start_event_source("workspace").await;
        let router = Arc::new(InMemorySessionRouter::new());
        router.set("ses_s", workspace_target);
        let state =
            ProxyState::new(ProxyConfig::default_allowlist(), default_target).with_router(router);
        let app = create_router(state);

        let bound = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/oc/event?session=ses_s")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(body_string(bound).await.contains("workspace"));

        let unbound = app
            .oneshot(
                Request::builder()
                    .uri("/api/oc/event?session=ses_other")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(body_string(unbound).await.contains("default"));
    }

    #[tokio::test]
    async fn the_event_stream_routes_from_the_workspace_header() {
        let default_target = start_event_source("default").await;
        let override_target = start_event_source("override").await;
        let state = ProxyState::new(ProxyConfig::default_allowlist(), default_target);
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/oc/event")
                    .header(HEADER_WORKSPACE_ENDPOINT, override_target)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(body_string(response).await.contains("override"));
    }

    #[tokio::test]
    async fn the_sdk_path_routes_by_session_too() {
        let default_target = start_fake_opencode().await;
        let router = Arc::new(InMemorySessionRouter::new());
        router.set("ses_e", "http://127.0.0.1:1");
        let state =
            ProxyState::new(ProxyConfig::default_allowlist(), default_target).with_router(router);
        let app = create_router(state);
        let response = app
            .oneshot(authed("GET", "/session/ses_e/message"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
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

    fn seed_events(state: &ProxyState, session_id: SessionId, count: usize) {
        for index in 0..count {
            record_event(state, session_id, MessageType::Event, json!({"n": index}));
        }
    }

    #[tokio::test]
    async fn transcript_after_cursor_returns_only_newer_events() {
        let state = test_state();
        let session_id = SessionId::new();
        seed_events(&state, session_id, 3);
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{session_id}/transcript?after=1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        let events: Vec<TunnelMessage> = serde_json::from_str(&body).unwrap();
        assert_eq!(events.len(), 1);
        assert!(events.iter().all(|event| event.sequence > 1));
    }

    #[tokio::test]
    async fn transcript_without_cursor_returns_everything() {
        let state = test_state();
        let session_id = SessionId::new();
        seed_events(&state, session_id, 3);
        let app = create_router(state);
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
        let body = body_string(response).await;
        let events: Vec<TunnelMessage> = serde_json::from_str(&body).unwrap();
        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn transcript_follow_streams_backlog_then_live_events() {
        let state = test_state();
        let session_id = SessionId::new();
        record_event(&state, session_id, MessageType::Event, json!({"n": 0}));
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/api/tunnel/{session_id}/transcript?after=-1&follow=true"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );

        let mut stream = response.into_body().into_data_stream();

        let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("backlog frame timed out")
            .expect("stream closed early")
            .expect("frame error");
        let first = String::from_utf8_lossy(&first).to_string();
        assert!(first.contains("id:"), "{first}");
        assert!(first.contains("event: event"), "{first}");
        assert!(first.contains("\"n\":0"), "{first}");

        record_event(&state, session_id, MessageType::Response, json!({"n": 1}));

        let second = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("live frame timed out")
            .expect("stream closed early")
            .expect("frame error");
        let second = String::from_utf8_lossy(&second).to_string();
        assert!(second.contains("event: response"), "{second}");
        assert!(second.contains("\"n\":1"), "{second}");
    }

    #[tokio::test]
    async fn transcript_follow_is_empty_for_unknown_session_then_streams_live() {
        let state = test_state();
        let session_id = SessionId::new();
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{session_id}/transcript?follow=true"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut stream = response.into_body().into_data_stream();
        record_event(&state, session_id, MessageType::Event, json!({"n": 7}));
        let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("live frame timed out")
            .expect("stream closed early")
            .expect("frame error");
        let frame = String::from_utf8_lossy(&frame).to_string();
        assert!(frame.contains("\"n\":7"), "{frame}");
    }

    #[tokio::test]
    async fn transcript_follow_excludes_other_sessions() {
        let state = test_state();
        let wanted = SessionId::new();
        let other = SessionId::new();
        record_event(&state, wanted, MessageType::Event, json!({"n": 0}));
        record_event(&state, other, MessageType::Event, json!({"n": 1}));
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{wanted}/transcript?after=-1"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = body_string(response).await;
        let events: Vec<TunnelMessage> = serde_json::from_str(&body).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session_id, wanted);
    }

    #[tokio::test]
    async fn transcript_unknown_session_is_empty_not_an_error() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/tunnel/{}/transcript", SessionId::new()))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response).await, "[]");
    }

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn upstream_credentials_prefers_menzi_overrides() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::set_var("OPENCODE_SERVER_USERNAME", "env-user");
            std::env::set_var("OPENCODE_SERVER_PASSWORD", "env-pass");
        }
        assert_eq!(
            upstream_credentials(),
            Some(("env-user".to_string(), "env-pass".to_string()))
        );

        unsafe {
            std::env::set_var("MENZI_OPENCODE_USERNAME", "menzi-user");
            std::env::set_var("MENZI_OPENCODE_PASSWORD", "menzi-pass");
        }
        assert_eq!(
            upstream_credentials(),
            Some(("menzi-user".to_string(), "menzi-pass".to_string()))
        );

        unsafe {
            std::env::remove_var("MENZI_OPENCODE_USERNAME");
            std::env::remove_var("MENZI_OPENCODE_PASSWORD");
            std::env::remove_var("OPENCODE_SERVER_USERNAME");
            std::env::remove_var("OPENCODE_SERVER_PASSWORD");
        }
        assert_eq!(upstream_credentials(), None);
    }

    #[tokio::test]
    async fn forwarded_calls_carry_upstream_basic_auth() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/api/session",
            get(|headers: HeaderMap| async move {
                let auth = headers
                    .get(header::AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                Json(json!({ "auth": auth }))
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        unsafe {
            std::env::set_var("MENZI_OPENCODE_USERNAME", "proxy-user");
            std::env::set_var("MENZI_OPENCODE_PASSWORD", "proxy-pass");
        }

        let state = ProxyState::new(
            ProxyConfig::default_allowlist(),
            format!("http://{address}"),
        );
        let proxy = create_router(state);
        let response = proxy
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
        assert!(
            body.contains("Basic cHJveHktdXNlcjpwcm94eS1wYXNz"),
            "{body}"
        );

        unsafe {
            std::env::remove_var("MENZI_OPENCODE_USERNAME");
            std::env::remove_var("MENZI_OPENCODE_PASSWORD");
        }
    }

    #[tokio::test]
    async fn event_stream_proxies_upstream_frames() {
        let app = Router::new().route(
            "/api/event",
            get(|| async {
                let stream = futures_util::stream::iter([
                    Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"data: {\"a\":1}\n\n")),
                    Ok(axum::body::Bytes::from_static(b": heartbeat\n\n")),
                ]);
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(axum::body::Body::from_stream(stream))
                    .unwrap()
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let state = ProxyState::new(
            ProxyConfig::default_allowlist(),
            format!("http://{address}"),
        );
        let proxy = create_router(state);
        let response = proxy
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/oc/event")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        let body = body_string(response).await;
        assert!(body.contains("data: {\"a\":1}"), "{body}");
        assert!(!body.contains("data: data:"), "{body}");
    }

    #[tokio::test]
    async fn event_stream_reports_upstream_failure() {
        let app = Router::new().route(
            "/api/event",
            get(|| async { StatusCode::SERVICE_UNAVAILABLE }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let state = ProxyState::new(
            ProxyConfig::default_allowlist(),
            format!("http://{address}"),
        );
        let proxy = create_router(state);
        let response = proxy
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/oc/event")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn sdk_session_routes_are_forwarded_without_the_api_prefix() {
        let app = Router::new().route(
            "/session/{id}/message",
            get(|| async { Json(json!([{"info": {"id": "m1"}}])) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let router = Arc::new(InMemorySessionRouter::new());
        router.set("abc", format!("http://{address}"));
        let state = ProxyState::new(ProxyConfig::default_allowlist(), "http://127.0.0.1:1")
            .with_router(router);
        let proxy = create_router(state);
        let response = proxy
            .oneshot(authed("GET", "/session/abc/message"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("m1"), "{body}");
    }

    #[tokio::test]
    async fn unlisted_unprefixed_routes_are_refused() {
        let state = ProxyState::new(
            ProxyConfig::default_allowlist(),
            "http://127.0.0.1:1".to_string(),
        );
        let proxy = create_router(state);
        let response = proxy
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/session/abc/deleteeverything")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn list_sessions_is_empty_before_any_event() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/tunnel/sessions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_string(response).await, r#"{"sessions":[]}"#);
    }

    #[tokio::test]
    async fn list_sessions_orders_by_most_recent_sequence() {
        let state = test_state();
        let older = SessionId::new();
        let newer = SessionId::new();
        record_event(&state, older, MessageType::Event, json!({"n": 0}));
        record_event(&state, newer, MessageType::Event, json!({"n": 1}));
        record_event(&state, newer, MessageType::Response, json!({"n": 2}));
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/tunnel/sessions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = body_string(response).await;
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        let sessions = value["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["id"], newer.to_string());
        assert_eq!(sessions[0]["events_count"], 2);
        assert_eq!(sessions[1]["id"], older.to_string());
    }

    #[tokio::test]
    async fn health_check_is_bodyless_and_reports_opencode_url() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_string(response).await;
        assert!(body.contains("\"status\":\"ok\""), "{body}");
        assert!(body.contains("\"shared_opencode\":null"), "{body}");
        assert!(!body.contains("opencode_url"), "{body}");
    }

    #[tokio::test]
    async fn health_check_reports_a_registered_shared_opencode() {
        let state = test_state();
        let app = create_router(state);
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/opencode/register")
                    .header("content-type", "application/json")
                    .body(Body::from("{\"url\":\"http://shared:4096\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(body_string(response).await.contains("http://shared:4096"));
    }

    #[tokio::test]
    async fn ready_check_returns_ok() {
        let state = test_state();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(body_string(response).await.contains("\"status\":\"ready\""));
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
