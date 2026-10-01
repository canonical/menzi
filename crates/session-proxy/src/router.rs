use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CacheEntry {
    endpoint: Option<String>,
    at: Instant,
}

#[async_trait]
pub trait SessionRouter: Send + Sync {
    async fn endpoint_for(&self, session: &str) -> Option<String>;

    fn bind(&self, _session: &str, _endpoint: &str) {}

    fn unbind(&self, _session: &str) {}
}

#[derive(Default)]
pub struct InMemorySessionRouter {
    entries: Mutex<HashMap<String, String>>,
}

impl InMemorySessionRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, session: impl Into<String>, endpoint: impl Into<String>) {
        self.entries
            .lock()
            .expect("session router lock")
            .insert(session.into(), endpoint.into());
    }

    pub fn remove(&self, session: &str) {
        self.entries
            .lock()
            .expect("session router lock")
            .remove(session);
    }

    pub fn bound(&self, session: &str) -> Option<String> {
        self.entries
            .lock()
            .expect("session router lock")
            .get(session)
            .cloned()
    }
}

#[async_trait]
impl SessionRouter for InMemorySessionRouter {
    async fn endpoint_for(&self, session: &str) -> Option<String> {
        self.bound(session)
    }

    fn bind(&self, session: &str, endpoint: &str) {
        self.set(session, endpoint);
    }

    fn unbind(&self, session: &str) {
        self.remove(session);
    }
}

pub const HEADER_WORKSPACE_ENDPOINT: &str = "x-menzi-workspace-endpoint";
pub const HEADER_SERVICE: &str = "x-menzi-service";

pub struct WorkspaceRouter {
    base_url: String,
    service: String,
    client: reqwest::Client,
    cache: Mutex<HashMap<String, CacheEntry>>,
    ttl: Duration,
}

impl WorkspaceRouter {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            service: String::new(),
            client: reqwest::Client::new(),
            cache: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(30),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            std::env::var("MENZI_WORKSPACE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8096".to_string()),
        )
        .with_service(std::env::var("MENZI_WORKSPACE_SERVICE").unwrap_or_default())
    }

    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = service.into();
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn path(session: &str) -> String {
        format!("/api/v1/sessions/{session}/workspace")
    }

    pub fn parse_response(body: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(body).ok()?;
        value
            .get("endpoint")
            .and_then(|endpoint| endpoint.as_str())
            .map(str::to_string)
            .filter(|endpoint| !endpoint.is_empty())
    }

    fn cached(&self, session: &str) -> Option<Option<String>> {
        let cache = self.cache.lock().expect("session router lock");
        let entry = cache.get(session)?;
        if entry.at.elapsed() > self.ttl {
            return None;
        }
        Some(entry.endpoint.clone())
    }

    fn remember(&self, session: &str, endpoint: Option<String>) -> Option<String> {
        self.cache.lock().expect("session router lock").insert(
            session.to_string(),
            CacheEntry {
                endpoint: endpoint.clone(),
                at: Instant::now(),
            },
        );
        endpoint
    }
}

#[async_trait]
impl SessionRouter for WorkspaceRouter {
    async fn endpoint_for(&self, session: &str) -> Option<String> {
        if let Some(cached) = self.cached(session) {
            return cached;
        }
        let target = format!("{}{}", self.base_url, Self::path(session));
        let mut request = self.client.get(target);
        if !self.service.is_empty() {
            request = request.header(HEADER_SERVICE, self.service.clone());
        }
        match request.send().await {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                self.remember(session, Self::parse_response(&body))
            }
            _ => self.remember(session, None),
        }
    }
}

pub fn session_from_path(path: &str) -> Option<&str> {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let index = parts
        .iter()
        .position(|part| *part == "session" || *part == "api")?;
    if parts.get(index) == Some(&"api") {
        return match parts.get(index + 1) {
            Some(&"session") => parts.get(index + 2).copied(),
            _ => None,
        };
    }
    match parts.get(index + 1) {
        Some(candidate) if *candidate != "message" && *candidate != "diff" => Some(candidate),
        _ => None,
    }
}

const CAPABILITY_ROUTES: [&str; 3] = ["/api/model", "/agent", "/api/agent"];

pub fn is_capability_path(path: &str) -> bool {
    CAPABILITY_ROUTES.contains(&path)
}

/// Whether a capability request came alongside a session, which is how the
/// composer asks the workspace it is about to prompt rather than a shared
/// instance.
pub fn session_hint(path: &str, query: Option<&str>) -> Option<String> {
    if is_capability_path(path) {
        return query
            .and_then(|query| {
                query.split('&').find_map(|pair| {
                    let (key, value) = pair.split_once('=')?;
                    (key == "session" && !value.is_empty()).then(|| percent_decode(value))
                })
            })
            .filter(|session| !session.is_empty());
    }
    session_from_path(path).map(str::to_string)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn endpoint_override(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(HEADER_WORKSPACE_ENDPOINT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn a_capability_path_is_recognised() {
        assert!(is_capability_path("/api/model"));
        assert!(is_capability_path("/agent"));
        assert!(is_capability_path("/api/agent"));
    }

    #[test]
    fn a_session_route_is_not_a_capability_path() {
        assert!(!is_capability_path("/session/ses_1/message"));
        assert!(!is_capability_path("/vcs/status"));
    }

    #[test]
    fn a_capability_request_carries_its_session_in_the_query() {
        assert_eq!(
            session_hint("/api/model", Some("session=ses_abc")),
            Some("ses_abc".to_string())
        );
    }

    #[test]
    fn a_capability_session_is_percent_decoded() {
        assert_eq!(
            session_hint("/agent", Some("session=ses%5Fa%2Bb")),
            Some("ses_a+b".to_string())
        );
    }

    #[test]
    fn a_capability_request_without_a_session_has_no_hint() {
        assert_eq!(session_hint("/api/model", None), None);
        assert_eq!(session_hint("/api/model", Some("other=1")), None);
        assert_eq!(session_hint("/api/model", Some("session=")), None);
    }

    #[test]
    fn a_capability_hint_ignores_a_later_session_pair() {
        assert_eq!(
            session_hint("/api/model", Some("a=1&session=ses_x&b=2")),
            Some("ses_x".to_string())
        );
    }

    #[test]
    fn a_session_route_takes_its_hint_from_the_path() {
        assert_eq!(
            session_hint("/session/ses_1/message", None),
            Some("ses_1".to_string())
        );
        assert_eq!(
            session_hint("/session/ses_1/message", Some("session=ses_2")),
            Some("ses_1".to_string())
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn the_sdk_path_yields_the_session() {
        assert_eq!(session_from_path("/session/ses_1"), Some("ses_1"));
        assert_eq!(session_from_path("/session/ses_1/message"), Some("ses_1"));
        assert_eq!(session_from_path("/session/ses_1/diff"), Some("ses_1"));
    }

    #[test]
    fn the_legacy_path_yields_the_session() {
        assert_eq!(session_from_path("/api/session/ses_2"), Some("ses_2"));
        assert_eq!(
            session_from_path("/api/session/ses_2/message"),
            Some("ses_2")
        );
    }

    #[test]
    fn a_listing_has_no_session() {
        assert_eq!(session_from_path("/session"), None);
        assert_eq!(session_from_path("/api/session"), None);
    }

    #[test]
    fn an_unrelated_path_has_no_session() {
        assert_eq!(session_from_path("/vcs/status"), None);
        assert_eq!(session_from_path("/api/config"), None);
        assert_eq!(session_from_path("/event"), None);
    }

    #[test]
    fn a_binding_is_remembered() {
        let router = InMemorySessionRouter::new();
        assert_eq!(router.bound("ses_1"), None);
        router.set("ses_1", "http://10.0.0.1:17999");
        assert_eq!(
            router.bound("ses_1"),
            Some("http://10.0.0.1:17999".to_string())
        );
        router.remove("ses_1");
        assert_eq!(router.bound("ses_1"), None);
    }

    #[tokio::test]
    async fn binding_through_the_trait_is_visible_to_the_resolver() {
        let router = InMemorySessionRouter::new();
        let bound: &dyn SessionRouter = &router;
        bound.bind("ses_1", "http://10.0.0.4:17999");
        assert_eq!(
            bound.endpoint_for("ses_1").await,
            Some("http://10.0.0.4:17999".to_string())
        );
        bound.unbind("ses_1");
        assert_eq!(bound.endpoint_for("ses_1").await, None);
    }

    #[tokio::test]
    async fn the_resolver_answers_from_the_binding() {
        let router = InMemorySessionRouter::new();
        router.set("ses_1", "http://10.0.0.2:17999");
        let resolved: Option<String> = SessionRouter::endpoint_for(&router, "ses_1").await;
        assert_eq!(resolved, Some("http://10.0.0.2:17999".to_string()));
    }

    #[test]
    fn the_workspace_lookup_path_is_per_session() {
        assert_eq!(
            WorkspaceRouter::path("ses_1"),
            "/api/v1/sessions/ses_1/workspace"
        );
    }

    #[test]
    fn the_workspace_response_is_read_for_an_endpoint() {
        let body = r#"{"workspace_id":"wsp-1","endpoint":"http://10.0.0.2:17999"}"#;
        assert_eq!(
            WorkspaceRouter::parse_response(body),
            Some("http://10.0.0.2:17999".to_string())
        );
    }

    #[test]
    fn a_workspace_response_without_an_endpoint_is_no_routing() {
        assert_eq!(WorkspaceRouter::parse_response(r#"{"endpoint":""}"#), None);
        assert_eq!(WorkspaceRouter::parse_response("not json"), None);
        assert_eq!(WorkspaceRouter::parse_response("{}"), None);
    }

    #[tokio::test]
    async fn the_workspace_router_identifies_itself_as_a_service() {
        let seen = Arc::new(std::sync::Mutex::new(String::new()));
        let recorder = seen.clone();
        let app = axum::Router::new().route(
            "/api/v1/sessions/{session}/workspace",
            axum::routing::get(
                move |headers: axum::http::HeaderMap,
                      axum::extract::Path(session): axum::extract::Path<String>| {
                    let recorder = recorder.clone();
                    async move {
                        *recorder.lock().unwrap() = headers
                            .get(HEADER_SERVICE)
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or_default()
                            .to_string();
                        axum::Json(serde_json::json!({
                            "endpoint": "http://10.0.0.1:17999",
                            "session": session,
                        }))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let router =
            WorkspaceRouter::new(format!("http://{address}")).with_service("session-proxy");
        assert!(router.endpoint_for("ses_1").await.is_some());
        assert_eq!(*seen.lock().unwrap(), "session-proxy");
    }

    #[tokio::test]
    async fn the_workspace_router_caches_a_lookup() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let app = axum::Router::new().route(
            "/api/v1/sessions/{session}/workspace",
            axum::routing::get(
                move |axum::extract::Path(session): axum::extract::Path<String>| {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        axum::Json(serde_json::json!({
                            "endpoint": "http://10.0.0.1:17999",
                            "session": session,
                        }))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let router = WorkspaceRouter::new(format!("http://{address}"));
        assert_eq!(
            router.endpoint_for("ses_1").await,
            Some("http://10.0.0.1:17999".to_string())
        );
        assert_eq!(
            router.endpoint_for("ses_1").await,
            Some("http://10.0.0.1:17999".to_string())
        );
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_workspace_router_looks_again_after_the_cache_expires() {
        let router = WorkspaceRouter::new("http://127.0.0.1:1").with_ttl(Duration::from_millis(1));
        assert_eq!(router.endpoint_for("ses_1").await, None);
        assert_eq!(router.cached("ses_1"), Some(None));
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(router.cached("ses_1"), None);
    }

    #[tokio::test]
    async fn the_workspace_router_asks_per_session() {
        let router = WorkspaceRouter::new("http://127.0.0.1:1").with_ttl(Duration::from_secs(30));
        assert_eq!(router.endpoint_for("ses_a").await, None);
        assert_eq!(router.endpoint_for("ses_b").await, None);
        assert_eq!(router.cached("ses_a"), Some(None));
        assert_eq!(router.cached("ses_b"), Some(None));
        assert_eq!(router.cached("ses_c"), None);
    }

    #[test]
    fn the_header_override_is_read_when_present() {
        let mut headers = axum::http::HeaderMap::new();
        assert_eq!(endpoint_override(&headers), None);
        headers.insert(
            HEADER_WORKSPACE_ENDPOINT,
            "http://10.0.0.3:17999".parse().unwrap(),
        );
        assert_eq!(
            endpoint_override(&headers),
            Some("http://10.0.0.3:17999".to_string())
        );
    }

    #[test]
    fn an_empty_header_override_is_ignored() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(HEADER_WORKSPACE_ENDPOINT, "".parse().unwrap());
        assert_eq!(endpoint_override(&headers), None);
    }
}
