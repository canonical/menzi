use menzi_auth::role_store::{InMemoryRoleStore, PostgresRoleStore, RoleStore};
use menzi_auth::store::{
    AccountStore, InMemoryAccountStore, InMemoryOidcStateStore, InMemorySessionStore,
    InMemoryThrottleStore, OidcStateStore, SessionStore, ThrottleStore,
};
use sqlx::PgPool;
use std::sync::Arc;

use super::clock::{Clock, SystemClock};
use super::config::AuthConfig;
use super::mailer::{mailer_from_env, Mailer, StubMailer};
use super::oidc::{rust_provider, OpenIdProvider, ProviderRegistry};
use super::service::AuthService;
use crate::identity::CallerResolver;

pub struct Stores {
    pub accounts: Arc<dyn AccountStore>,
    pub sessions: Arc<dyn SessionStore>,
    pub states: Arc<dyn OidcStateStore>,
    pub throttle: Arc<dyn ThrottleStore>,
    pub roles: Arc<dyn RoleStore>,
}

pub fn postgres_stores(pool: PgPool) -> Stores {
    Stores {
        accounts: Arc::new(super::stores::PostgresAccountStore::new(pool.clone())),
        sessions: Arc::new(super::stores::PostgresSessionStore::new(pool.clone())),
        states: Arc::new(super::stores::PostgresOidcStateStore::new(pool.clone())),
        throttle: Arc::new(super::stores::PostgresThrottleStore::new(pool.clone())),
        roles: Arc::new(PostgresRoleStore::new(pool)),
    }
}

pub fn in_memory_stores() -> Stores {
    Stores {
        accounts: Arc::new(InMemoryAccountStore::new()),
        sessions: Arc::new(InMemorySessionStore::new()),
        states: Arc::new(InMemoryOidcStateStore::new()),
        throttle: Arc::new(InMemoryThrottleStore::new()),
        roles: Arc::new(InMemoryRoleStore::new()),
    }
}

pub struct AuthState {
    pub service: Arc<AuthService>,
    pub config: Arc<AuthConfig>,
    pub resolver: Arc<dyn CallerResolver>,
}

impl AuthState {
    pub fn new(
        stores: Stores,
        clock: Arc<dyn Clock>,
        mailer: Arc<dyn Mailer>,
        providers: Vec<Arc<dyn OpenIdProvider>>,
        config: AuthConfig,
    ) -> Self {
        let shared = Arc::new(config.clone());
        let service = AuthService::new(
            stores.accounts.clone(),
            stores.sessions.clone(),
            stores.states.clone(),
            stores.throttle,
            clock,
            mailer,
            providers,
            config,
        )
        .with_roles(stores.roles);
        let resolver = Arc::new(super::identity::SessionResolver::new(
            stores.sessions,
            stores.accounts,
            shared.clone(),
        ));
        Self {
            service: Arc::new(service),
            config: shared,
            resolver,
        }
    }

    pub async fn from_env(pool: PgPool) -> Self {
        let config = AuthConfig::from_env();
        let mut providers: Vec<Arc<dyn OpenIdProvider>> = Vec::new();
        for entry in ProviderRegistry::from_env().entries().to_vec() {
            match rust_provider(&entry, &config).await {
                Ok(provider) => providers.push(Arc::new(provider) as Arc<dyn OpenIdProvider>),
                Err(error) => tracing::warn!("oidc provider {} is unusable: {error}", entry.id),
            }
        }
        Self::new(
            postgres_stores(pool),
            Arc::new(SystemClock),
            mailer_from_env(),
            providers,
            config,
        )
    }

    pub fn for_tests() -> Self {
        Self::new(
            in_memory_stores(),
            Arc::new(super::clock::FixedClock::new(chrono::Utc::now())),
            Arc::new(StubMailer::new()),
            Vec::new(),
            AuthConfig::for_tests(),
        )
    }

    pub fn for_tests_with(resolver: Arc<dyn CallerResolver>) -> Self {
        let mut state = Self::for_tests();
        state.resolver = resolver;
        state
    }
}
