use async_trait::async_trait;
use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    User(String),
    Service(String),
}

impl Caller {
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Caller::User(id) => Some(id),
            Caller::Service(_) => None,
        }
    }

    pub fn header_value(&self) -> String {
        match self {
            Caller::User(id) => id.clone(),
            Caller::Service(name) => name.clone(),
        }
    }

    pub fn header_name(&self) -> &'static str {
        match self {
            Caller::User(_) => "x-menzi-user-id",
            Caller::Service(_) => "x-menzi-service",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    Missing,
    Invalid,
}

pub const HEADER_USER_ID: &str = "x-menzi-user-id";
pub const HEADER_SERVICE: &str = "x-menzi-service";
pub const HEADER_CREDENTIALS: &str = "authorization";

/// The identity headers a client is never allowed to choose for itself. They
/// are stripped from every inbound request and replaced with what this resolves.
pub const IDENTITY_HEADERS: [&str; 2] = [HEADER_USER_ID, HEADER_SERVICE];

#[async_trait]
pub trait CallerResolver: Send + Sync {
    async fn resolve(&self, headers: &HeaderMap) -> Result<Caller, IdentityError>;
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let raw = header(headers, HEADER_CREDENTIALS)?;
    let rest = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    let token = rest.trim();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

pub struct TokenCallerResolver {
    pool: PgPool,
}

impl TokenCallerResolver {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CallerResolver for TokenCallerResolver {
    async fn resolve(&self, headers: &HeaderMap) -> Result<Caller, IdentityError> {
        let Some(token) = bearer(headers) else {
            return Err(IdentityError::Missing);
        };
        let row: Result<Option<(uuid::Uuid,)>, sqlx::Error> = sqlx::query_as(
            "SELECT user_id FROM session_tokens
             WHERE token = $1 AND revoked_at IS NULL AND expires_at > now()",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await;
        match row {
            Ok(Some((user_id,))) => Ok(Caller::User(user_id.to_string())),
            Ok(None) => Err(IdentityError::Invalid),
            Err(error) => {
                tracing::warn!("session token lookup failed: {error}");
                Err(IdentityError::Invalid)
            }
        }
    }
}

pub struct DevCallerResolver {
    user_id: String,
}

impl DevCallerResolver {
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
        }
    }
}

#[async_trait]
impl CallerResolver for DevCallerResolver {
    async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
        if self.user_id.is_empty() {
            return Err(IdentityError::Missing);
        }
        Ok(Caller::User(self.user_id.clone()))
    }
}

pub struct ServiceCallerResolver {
    name: String,
}

impl ServiceCallerResolver {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl CallerResolver for ServiceCallerResolver {
    async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
        if self.name.is_empty() {
            return Err(IdentityError::Missing);
        }
        Ok(Caller::Service(self.name.clone()))
    }
}

pub fn resolver_from_env(pool: PgPool) -> std::sync::Arc<dyn CallerResolver> {
    if let Ok(service) = std::env::var("MENZI_WORKSPACE_SERVICE_ONLY") {
        if !service.is_empty() {
            return std::sync::Arc::new(ServiceCallerResolver::new(service));
        }
    }
    if std::env::var("MENZI_DEV_AUTH").unwrap_or_default() == "1" {
        let user = std::env::var("MENZI_DEV_USER_ID").unwrap_or_default();
        return std::sync::Arc::new(DevCallerResolver::new(user));
    }
    std::sync::Arc::new(TokenCallerResolver::new(pool))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn a_bearer_token_is_read() {
        assert_eq!(
            bearer(&headers(&[("authorization", "Bearer abc")])),
            Some("abc")
        );
        assert_eq!(
            bearer(&headers(&[("authorization", "bearer abc")])),
            Some("abc")
        );
        assert_eq!(bearer(&headers(&[("authorization", "Bearer  ")])), None);
        assert_eq!(bearer(&headers(&[])), None);
    }

    #[test]
    fn a_raw_authorization_header_is_not_a_token() {
        assert_eq!(bearer(&headers(&[("authorization", "abc")])), None);
    }

    #[test]
    fn a_caller_renders_its_identity_header() {
        let user = Caller::User("u-1".to_string());
        assert_eq!(user.header_name(), "x-menzi-user-id");
        assert_eq!(user.header_value(), "u-1");
        assert_eq!(user.user_id(), Some("u-1"));

        let service = Caller::Service("autonomous".to_string());
        assert_eq!(service.header_name(), "x-menzi-service");
        assert_eq!(service.user_id(), None);
    }

    #[tokio::test]
    async fn the_development_resolver_ignores_the_request() {
        let resolver = DevCallerResolver::new("dev-user");
        let caller = resolver.resolve(&HeaderMap::new()).await.unwrap();
        assert_eq!(caller, Caller::User("dev-user".to_string()));
    }

    #[tokio::test]
    async fn a_development_resolver_without_a_user_resolves_nothing() {
        let resolver = DevCallerResolver::new("");
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Err(IdentityError::Missing)
        );
    }

    #[tokio::test]
    async fn the_service_resolver_reports_its_own_name() {
        let resolver = ServiceCallerResolver::new("autonomous");
        let caller = resolver.resolve(&HeaderMap::new()).await.unwrap();
        assert_eq!(caller, Caller::Service("autonomous".to_string()));
    }

    #[tokio::test]
    async fn a_request_without_a_token_is_missing_not_invalid() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        let resolver = TokenCallerResolver::new(pool);
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Err(IdentityError::Missing)
        );
    }

    #[test]
    fn the_identity_headers_are_the_two_a_client_cannot_set() {
        assert!(IDENTITY_HEADERS.contains(&HEADER_USER_ID));
        assert!(IDENTITY_HEADERS.contains(&HEADER_SERVICE));
        assert!(!IDENTITY_HEADERS.contains(&HEADER_CREDENTIALS));
    }
}

pub fn caller_uuid(caller: &Caller) -> Option<Uuid> {
    caller.user_id().and_then(|id| Uuid::parse_str(id).ok())
}
