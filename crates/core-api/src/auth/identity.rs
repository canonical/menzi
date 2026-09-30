use async_trait::async_trait;
use axum::http::HeaderMap;
use chrono::Utc;
use menzi_auth::record::NewSession;
use menzi_auth::secret::digest;
use menzi_auth::store::{AccountStore, SessionStore};
use std::sync::Arc;
use uuid::Uuid;

use crate::identity::{Caller, CallerResolver, IdentityError};

use super::config::AuthConfig;
use super::cookie;

pub const HEADER_SESSION: &str = "x-menzi-session";

pub struct SessionResolver {
    sessions: Arc<dyn SessionStore>,
    accounts: Arc<dyn AccountStore>,
    config: Arc<AuthConfig>,
}

impl SessionResolver {
    pub fn new(
        sessions: Arc<dyn SessionStore>,
        accounts: Arc<dyn AccountStore>,
        config: Arc<AuthConfig>,
    ) -> Self {
        Self {
            sessions,
            accounts,
            config,
        }
    }

    fn bearer(headers: &HeaderMap) -> Option<String> {
        let raw = headers
            .get(axum::http::header::AUTHORIZATION)?
            .to_str()
            .ok()?;
        let rest = raw
            .strip_prefix("Bearer ")
            .or_else(|| raw.strip_prefix("bearer "))?;
        let token = rest.trim();
        if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        }
    }

    async fn resolve_token(&self, token: &str) -> Result<Caller, IdentityError> {
        let session = self
            .sessions
            .find_active(&digest(token))
            .await
            .map_err(|error| {
                tracing::warn!("session lookup failed: {error}");
                IdentityError::Invalid
            })?
            .ok_or(IdentityError::Invalid)?;

        if Utc::now() - session.last_seen_at > self.config.session_idle {
            let _ = self.sessions.revoke(session.id).await;
            return Err(IdentityError::Invalid);
        }

        let user = self
            .accounts
            .find_by_id(session.user_id)
            .await
            .map_err(|error| {
                tracing::warn!("account lookup failed: {error}");
                IdentityError::Invalid
            })?
            .ok_or(IdentityError::Invalid)?;

        let _ = self.sessions.touch(session.id).await;
        Ok(Caller::User(user.id.to_string()))
    }
}

#[async_trait]
impl CallerResolver for SessionResolver {
    async fn resolve(&self, headers: &HeaderMap) -> Result<Caller, IdentityError> {
        if let Some(token) = cookie::value_of(headers, &self.config.session_cookie) {
            return self.resolve_token(token).await;
        }
        match Self::bearer(headers) {
            Some(token) => self.resolve_token(&token).await,
            None => Err(IdentityError::Missing),
        }
    }
}

pub struct DevResolver {
    user_id: String,
}

impl DevResolver {
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
        }
    }
}

#[async_trait]
impl CallerResolver for DevResolver {
    async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
        if self.user_id.is_empty() {
            return Err(IdentityError::Missing);
        }
        Ok(Caller::User(self.user_id.clone()))
    }
}

pub struct ServiceResolver {
    name: String,
}

impl ServiceResolver {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[async_trait]
impl CallerResolver for ServiceResolver {
    async fn resolve(&self, _headers: &HeaderMap) -> Result<Caller, IdentityError> {
        if self.name.is_empty() {
            return Err(IdentityError::Missing);
        }
        Ok(Caller::Service(self.name.clone()))
    }
}

pub struct FallbackResolver {
    primary: Arc<dyn CallerResolver>,
    secondary: Arc<dyn CallerResolver>,
}

impl FallbackResolver {
    pub fn new(primary: Arc<dyn CallerResolver>, secondary: Arc<dyn CallerResolver>) -> Self {
        Self { primary, secondary }
    }
}

#[async_trait]
impl CallerResolver for FallbackResolver {
    async fn resolve(&self, headers: &HeaderMap) -> Result<Caller, IdentityError> {
        match self.primary.resolve(headers).await {
            Ok(caller) => Ok(caller),
            Err(IdentityError::Missing) => self.secondary.resolve(headers).await,
            Err(error) => Err(error),
        }
    }
}

pub fn resolver_for(
    sessions: Arc<dyn SessionStore>,
    accounts: Arc<dyn AccountStore>,
    config: Arc<AuthConfig>,
) -> Arc<dyn CallerResolver> {
    let session = Arc::new(SessionResolver::new(sessions, accounts, config));
    if let Ok(dev) = std::env::var("MENZI_DEV_USER_ID") {
        if !dev.trim().is_empty() {
            return Arc::new(FallbackResolver::new(
                session,
                Arc::new(DevResolver::new(dev)),
            ));
        }
    }
    session
}

pub fn service_resolver_from_env() -> Arc<dyn CallerResolver> {
    match std::env::var("MENZI_WORKSPACE_SERVICE_ONLY") {
        Ok(name) if !name.trim().is_empty() => Arc::new(ServiceResolver::new(name)),
        _ => Arc::new(DevResolver::new("")),
    }
}

pub async fn seed_session(
    sessions: &dyn SessionStore,
    user_id: Uuid,
    provider_id: &str,
    ttl: chrono::Duration,
) -> Result<String, menzi_auth::store::StoreError> {
    let token = menzi_auth::secret::SessionSecret::mint()
        .map_err(|_| menzi_auth::store::StoreError::NotFound)?;
    sessions
        .create(NewSession {
            user_id,
            token_hash: token.digest(),
            csrf_hash: "unused".to_string(),
            provider_id: provider_id.to_string(),
            expires_at: Utc::now() + ttl,
            user_agent: None,
            ip: None,
        })
        .await?;
    Ok(token.expose().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use menzi_auth::store::{InMemoryAccountStore, InMemorySessionStore};

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    struct Harness {
        resolver: Arc<dyn CallerResolver>,
        sessions: Arc<InMemorySessionStore>,
        accounts: Arc<InMemoryAccountStore>,
    }

    fn harness(idle: chrono::Duration) -> Harness {
        let sessions = Arc::new(InMemorySessionStore::new());
        let accounts = Arc::new(InMemoryAccountStore::new());
        let config = Arc::new(AuthConfig {
            session_idle: idle,
            ..AuthConfig::for_tests()
        });
        let resolver: Arc<dyn CallerResolver> = Arc::new(SessionResolver::new(
            sessions.clone(),
            accounts.clone(),
            config,
        ));
        Harness {
            resolver,
            sessions,
            accounts,
        }
    }

    async fn seed(harness: &Harness) -> (Uuid, String) {
        let user = harness
            .accounts
            .create("a@b", "A Person", Some("hash"), true)
            .await
            .unwrap();
        let token = seed_session(
            harness.sessions.as_ref(),
            user.id,
            "password",
            chrono::Duration::hours(1),
        )
        .await
        .unwrap();
        (user.id, token)
    }

    #[tokio::test]
    async fn a_session_cookie_resolves_to_its_user() {
        let h = harness(chrono::Duration::hours(24));
        let (user_id, token) = seed(&h).await;
        let caller = h
            .resolver
            .resolve(&headers(&[("cookie", &format!("menzi_session={token}"))]))
            .await
            .unwrap();
        assert_eq!(caller, Caller::User(user_id.to_string()));
        assert_eq!(caller.header_name(), "x-menzi-user-id");
    }

    #[tokio::test]
    async fn a_bearer_token_resolves_to_its_user() {
        let h = harness(chrono::Duration::hours(24));
        let (user_id, token) = seed(&h).await;
        let caller = h
            .resolver
            .resolve(&headers(&[("authorization", &format!("Bearer {token}"))]))
            .await
            .unwrap();
        assert_eq!(caller, Caller::User(user_id.to_string()));
    }

    #[tokio::test]
    async fn a_request_with_no_credentials_is_missing() {
        let h = harness(chrono::Duration::hours(24));
        assert_eq!(
            h.resolver.resolve(&HeaderMap::new()).await,
            Err(IdentityError::Missing)
        );
    }

    #[tokio::test]
    async fn an_unknown_token_is_invalid() {
        let h = harness(chrono::Duration::hours(24));
        assert_eq!(
            h.resolver
                .resolve(&headers(&[("cookie", "menzi_session=mz_nope")]))
                .await,
            Err(IdentityError::Invalid)
        );
    }

    #[tokio::test]
    async fn a_revoked_token_is_invalid() {
        let h = harness(chrono::Duration::hours(24));
        let (_, token) = seed(&h).await;
        let devices = h.accounts.find_by_email("a@b").await.unwrap().unwrap();
        let session = h.sessions.list_for(devices.id).await.unwrap().remove(0);
        h.sessions.revoke(session.id).await.unwrap();
        assert_eq!(
            h.resolver
                .resolve(&headers(&[("cookie", &format!("menzi_session={token}"))]))
                .await,
            Err(IdentityError::Invalid)
        );
    }

    #[tokio::test]
    async fn an_idle_token_is_invalid() {
        let h = harness(chrono::Duration::seconds(0));
        let (_, token) = seed(&h).await;
        assert_eq!(
            h.resolver
                .resolve(&headers(&[("cookie", &format!("menzi_session={token}"))]))
                .await,
            Err(IdentityError::Invalid)
        );
    }

    #[tokio::test]
    async fn a_raw_authorization_header_is_not_a_token() {
        let h = harness(chrono::Duration::hours(24));
        assert_eq!(
            h.resolver
                .resolve(&headers(&[("authorization", "abc")]))
                .await,
            Err(IdentityError::Missing)
        );
    }

    #[tokio::test]
    async fn a_dev_resolver_ignores_the_request() {
        let resolver = DevResolver::new("dev-user");
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Ok(Caller::User("dev-user".to_string()))
        );
    }

    #[tokio::test]
    async fn a_dev_resolver_without_a_user_resolves_nothing() {
        let resolver = DevResolver::new("");
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Err(IdentityError::Missing)
        );
    }

    #[tokio::test]
    async fn a_service_resolver_reports_its_own_name() {
        let resolver = ServiceResolver::new("session-proxy");
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Ok(Caller::Service("session-proxy".to_string()))
        );
    }

    #[tokio::test]
    async fn an_empty_service_resolver_resolves_nothing() {
        let resolver = ServiceResolver::new("");
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Err(IdentityError::Missing)
        );
    }

    #[tokio::test]
    async fn the_fallback_prefers_a_real_session() {
        let h = harness(chrono::Duration::hours(24));
        let (user_id, token) = seed(&h).await;
        let resolver = FallbackResolver::new(h.resolver.clone(), Arc::new(DevResolver::new("dev")));
        let caller = resolver
            .resolve(&headers(&[("cookie", &format!("menzi_session={token}"))]))
            .await
            .unwrap();
        assert_eq!(caller, Caller::User(user_id.to_string()));
    }

    #[tokio::test]
    async fn the_fallback_uses_the_dev_user_when_there_is_no_session() {
        let h = harness(chrono::Duration::hours(24));
        let resolver = FallbackResolver::new(h.resolver.clone(), Arc::new(DevResolver::new("dev")));
        assert_eq!(
            resolver.resolve(&HeaderMap::new()).await,
            Ok(Caller::User("dev".to_string()))
        );
    }

    #[tokio::test]
    async fn the_fallback_does_not_mask_an_invalid_session() {
        let h = harness(chrono::Duration::hours(24));
        let resolver = FallbackResolver::new(h.resolver.clone(), Arc::new(DevResolver::new("dev")));
        assert_eq!(
            resolver
                .resolve(&headers(&[("cookie", "menzi_session=mz_nope")]))
                .await,
            Err(IdentityError::Invalid)
        );
    }

    #[test]
    fn the_service_header_is_the_workspace_services_name() {
        assert_eq!(HEADER_SESSION, "x-menzi-session");
    }
}
