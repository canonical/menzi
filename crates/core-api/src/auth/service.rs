use chrono::{DateTime, Utc};
use menzi_auth::password::{verify_stored, PasswordHash};
use menzi_auth::record::{normalise_email, IdentityLink, NewSession, OidcState, UserRecord};
use menzi_auth::secret::{random_url_safe, SessionSecret};
use menzi_auth::store::{
    check_password_policy, AccountStore, OidcStateStore, PasswordPolicyError, SessionStore,
    StoreError, ThrottleStore, PROVIDER_PASSWORD,
};
use std::sync::Arc;
use uuid::Uuid;

use super::clock::Clock;
use super::config::AuthConfig;
use super::mailer::{Mail, Mailer};
use super::oidc::{AuthError, IdTokenClaims, OpenIdProvider};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestMeta {
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthOutcome {
    pub user: UserRecord,
    pub session_token: SessionSecret,
    pub csrf_token: SessionSecret,
    pub expires_at: DateTime<Utc>,
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcOutcome {
    pub target: String,
    pub outcome: AuthOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub struct AuthService {
    accounts: Arc<dyn AccountStore>,
    sessions: Arc<dyn SessionStore>,
    states: Arc<dyn OidcStateStore>,
    throttle: Arc<dyn ThrottleStore>,
    clock: Arc<dyn Clock>,
    mailer: Arc<dyn Mailer>,
    providers: Vec<Arc<dyn OpenIdProvider>>,
    config: AuthConfig,
    dummy_hash: String,
    roles: Arc<dyn menzi_auth::role_store::RoleStore>,
}

impl AuthService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        accounts: Arc<dyn AccountStore>,
        sessions: Arc<dyn SessionStore>,
        states: Arc<dyn OidcStateStore>,
        throttle: Arc<dyn ThrottleStore>,
        clock: Arc<dyn Clock>,
        mailer: Arc<dyn Mailer>,
        providers: Vec<Arc<dyn OpenIdProvider>>,
        config: AuthConfig,
    ) -> Self {
        Self {
            dummy_hash: menzi_auth::store::dummy_password_hash(),
            accounts,
            sessions,
            states,
            throttle,
            clock,
            mailer,
            providers,
            config,
            roles: Arc::new(menzi_auth::role_store::InMemoryRoleStore::new()),
        }
    }

    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    pub fn provider(&self, id: &str) -> Option<Arc<dyn OpenIdProvider>> {
        self.providers
            .iter()
            .find(|provider| provider.id() == id)
            .cloned()
    }

    pub fn providers(&self) -> &[Arc<dyn OpenIdProvider>] {
        &self.providers
    }

    fn store_error(error: StoreError) -> AuthError {
        match error {
            StoreError::Duplicate(_) => {
                AuthError::Conflict("that email is already registered".into())
            }
            StoreError::NotFound => AuthError::NotFound,
            StoreError::Database(_) => AuthError::Unavailable("database".into()),
        }
    }

    fn secret_error() -> AuthError {
        AuthError::Unavailable("no system randomness".into())
    }

    pub async fn register(
        &self,
        request: RegisterRequest,
        meta: RequestMeta,
    ) -> Result<AuthOutcome, AuthError> {
        if !self.config.registration_mode.allows_registration() {
            return Err(AuthError::NotFound);
        }
        let email = normalise_email(&request.email);
        if email.is_empty() || !email.contains('@') {
            return Err(AuthError::BadRequest("a valid email is required".into()));
        }
        if request.name.trim().is_empty() {
            return Err(AuthError::BadRequest("a name is required".into()));
        }
        check_password_policy(&request.password, self.config.password_min_length, &email).map_err(
            |error| match error {
                PasswordPolicyError::TooShort { minimum } => AuthError::BadRequest(format!(
                    "the password must be at least {minimum} characters"
                )),
                PasswordPolicyError::MatchesEmail => {
                    AuthError::BadRequest("the password must not be your email address".into())
                }
            },
        )?;

        let hash = PasswordHash::hash(&request.password)
            .map_err(|_| AuthError::Unavailable("hashing".into()))?;
        let user = self
            .accounts
            .create(&email, &request.name, Some(hash.as_str()), true)
            .await
            .map_err(Self::store_error)?;

        self.issue(&user, PROVIDER_PASSWORD, meta).await
    }

    pub async fn login(
        &self,
        request: LoginRequest,
        meta: RequestMeta,
    ) -> Result<AuthOutcome, AuthError> {
        let email = normalise_email(&request.email);
        if email.is_empty() || request.password.is_empty() {
            return Err(AuthError::BadRequest(
                "an email and password are required".into(),
            ));
        }

        let since = self.clock.now() - self.config.login_lockout;
        let failures = self
            .throttle
            .failures_since(&email, since)
            .await
            .map_err(Self::store_error)?;
        if failures >= self.config.login_max_attempts {
            return Err(AuthError::Locked);
        }

        let found = self
            .accounts
            .find_by_email(&email)
            .await
            .map_err(Self::store_error)?;

        let user = match found {
            Some(user) => user,
            None => {
                verify_stored(&self.dummy_hash, &request.password);
                self.throttle
                    .record(&email, false)
                    .await
                    .map_err(Self::store_error)?;
                return Err(AuthError::Unauthorized);
            }
        };

        let stored = user.password_hash.clone().unwrap_or_default();
        if !verify_stored(&stored, &request.password) {
            self.throttle
                .record(&email, false)
                .await
                .map_err(Self::store_error)?;
            return Err(AuthError::Unauthorized);
        }

        self.throttle
            .record(&email, true)
            .await
            .map_err(Self::store_error)?;
        self.issue(&user, PROVIDER_PASSWORD, meta).await
    }

    pub async fn logout(&self, token: Option<&str>) -> Result<(), AuthError> {
        let Some(token) = token else {
            return Ok(());
        };
        if let Some(session) = self
            .sessions
            .find_active(&menzi_auth::secret::digest(token))
            .await
            .map_err(Self::store_error)?
        {
            self.sessions
                .revoke(session.id)
                .await
                .map_err(Self::store_error)?;
        }
        Ok(())
    }

    pub async fn session_for(&self, token: &str) -> Result<Option<UserRecord>, AuthError> {
        let Some(session) = self
            .sessions
            .find_active(&menzi_auth::secret::digest(token))
            .await
            .map_err(Self::store_error)?
        else {
            return Ok(None);
        };
        let now = self.clock.now();
        if now - session.last_seen_at > self.config.session_idle {
            self.sessions
                .revoke(session.id)
                .await
                .map_err(Self::store_error)?;
            return Ok(None);
        }
        let user = self
            .accounts
            .find_by_id(session.user_id)
            .await
            .map_err(Self::store_error)?;
        if user.is_some() {
            self.sessions
                .touch(session.id)
                .await
                .map_err(Self::store_error)?;
        }
        Ok(user)
    }

    async fn issue(
        &self,
        user: &UserRecord,
        provider_id: &str,
        meta: RequestMeta,
    ) -> Result<AuthOutcome, AuthError> {
        let session_token = SessionSecret::mint().map_err(|_| Self::secret_error())?;
        let csrf_token = SessionSecret::mint().map_err(|_| Self::secret_error())?;
        let expires_at = self.clock.now() + self.config.session_ttl;
        self.sessions
            .create(NewSession {
                user_id: user.id,
                token_hash: session_token.digest(),
                csrf_hash: menzi_auth::secret::digest(csrf_token.expose()),
                provider_id: provider_id.to_string(),
                expires_at,
                user_agent: meta.user_agent,
                ip: meta.ip,
            })
            .await
            .map_err(Self::store_error)?;
        Ok(AuthOutcome {
            user: user.clone(),
            session_token,
            csrf_token,
            expires_at,
            provider_id: provider_id.to_string(),
        })
    }

    pub async fn start_oidc(
        &self,
        provider_id: &str,
        redirect_to: Option<&str>,
    ) -> Result<String, AuthError> {
        let provider = self.provider(provider_id).ok_or(AuthError::NotFound)?;
        let state = random_url_safe(24).map_err(|_| Self::secret_error())?;
        let nonce = random_url_safe(24).map_err(|_| Self::secret_error())?;
        let verifier = random_url_safe(48).map_err(|_| Self::secret_error())?;
        let challenge = super::oidc::pkce_challenge(&verifier);

        self.states
            .put(OidcState {
                state: state.clone(),
                provider_id: provider_id.to_string(),
                nonce: nonce.clone(),
                code_verifier: verifier,
                redirect_to: Some(self.safe_target(redirect_to)),
                expires_at: self.clock.now() + self.config.oidc_state_ttl,
            })
            .await
            .map_err(Self::store_error)?;

        provider.authorization_url(&challenge, &state, &nonce).await
    }

    pub async fn finish_oidc(
        &self,
        provider_id: &str,
        code: &str,
        state: &str,
        meta: RequestMeta,
    ) -> Result<OidcOutcome, AuthError> {
        if self.provider(provider_id).is_none() {
            return Err(AuthError::NotFound);
        }
        let stored = self
            .states
            .take(state)
            .await
            .map_err(Self::store_error)?
            .ok_or_else(|| AuthError::BadRequest("the sign-in attempt expired".into()))?;
        if stored.provider_id != provider_id {
            return Err(AuthError::BadRequest(
                "the sign-in attempt did not match".into(),
            ));
        }
        if stored.is_expired(self.clock.now()) {
            return Err(AuthError::BadRequest("the sign-in attempt expired".into()));
        }

        let provider = self.provider(provider_id).ok_or(AuthError::NotFound)?;
        let claims = provider.exchange(code, &stored.code_verifier).await?;

        let user = self.resolve_identity(provider_id, &claims).await?;
        let outcome = self.issue(&user, provider_id, meta).await?;
        Ok(OidcOutcome {
            target: self.safe_target(stored.redirect_to.as_deref()),
            outcome,
        })
    }

    async fn resolve_identity(
        &self,
        provider_id: &str,
        claims: &IdTokenClaims,
    ) -> Result<UserRecord, AuthError> {
        if let Some(user) = self
            .accounts
            .find_by_identity(provider_id, &claims.subject)
            .await
            .map_err(Self::store_error)?
        {
            return Ok(user);
        }

        let Some(email) = claims.verified_email() else {
            return Err(AuthError::Forbidden);
        };
        let email = normalise_email(email);

        if let Some(existing) = self
            .accounts
            .find_by_email(&email)
            .await
            .map_err(Self::store_error)?
        {
            if existing.password_hash.is_some() {
                return Err(AuthError::Conflict(format!(
                    "an account already uses {email}; sign in with your password and link {provider_id} in Settings"
                )));
            }
            self.accounts
                .link_identity(IdentityLink {
                    user_id: existing.id,
                    provider_id: provider_id.to_string(),
                    subject: claims.subject.clone(),
                    email_at_link: Some(email.clone()),
                })
                .await
                .map_err(Self::store_error)?;
            return Ok(existing);
        }

        if !self.config.oidc_auto_provision {
            return Err(AuthError::Forbidden);
        }

        let name = claims
            .name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| email.clone());
        let user = self
            .accounts
            .create(&email, &name, None, true)
            .await
            .map_err(Self::store_error)?;
        self.accounts
            .link_identity(IdentityLink {
                user_id: user.id,
                provider_id: provider_id.to_string(),
                subject: claims.subject.clone(),
                email_at_link: Some(email),
            })
            .await
            .map_err(Self::store_error)?;
        Ok(user)
    }

    pub async fn change_password(
        &self,
        caller: Uuid,
        current_session: Uuid,
        current: &str,
        next: &str,
    ) -> Result<(), AuthError> {
        let user = self
            .accounts
            .find_by_id(caller)
            .await
            .map_err(Self::store_error)?
            .ok_or(AuthError::NotFound)?;
        let stored = user.password_hash.clone().ok_or_else(|| {
            AuthError::BadRequest("this account signs in with a provider, not a password".into())
        })?;
        if !verify_stored(&stored, current) {
            return Err(AuthError::Unauthorized);
        }
        check_password_policy(next, self.config.password_min_length, &user.email).map_err(
            |error| match error {
                PasswordPolicyError::TooShort { minimum } => AuthError::BadRequest(format!(
                    "the password must be at least {minimum} characters"
                )),
                PasswordPolicyError::MatchesEmail => {
                    AuthError::BadRequest("the password must not be your email address".into())
                }
            },
        )?;
        let hash =
            PasswordHash::hash(next).map_err(|_| AuthError::Unavailable("hashing".into()))?;
        self.accounts
            .set_password_hash(caller, hash.as_str())
            .await
            .map_err(Self::store_error)?;
        self.sessions
            .revoke_all_for(caller, Some(current_session))
            .await
            .map_err(Self::store_error)?;
        Ok(())
    }

    pub async fn request_reset(&self, email: &str) -> Result<(), AuthError> {
        let email = normalise_email(email);
        let user = self
            .accounts
            .find_by_email(&email)
            .await
            .map_err(Self::store_error)?;
        let Some(user) = user else {
            return Ok(());
        };
        let token = random_url_safe(32).map_err(|_| Self::secret_error())?;
        self.mailer
            .send(Mail {
                to: user.email.clone(),
                subject: "Reset your Menzi password".to_string(),
                body: format!(
                    "{}/reset-password?token={}",
                    self.config.public_base_url.trim_end_matches('/'),
                    token
                ),
            })
            .await
            .map_err(|_| AuthError::Unavailable("mail".into()))?;
        Ok(())
    }

    pub async fn complete_reset(&self, token: &str, password: &str) -> Result<(), AuthError> {
        if token.trim().is_empty() || password.is_empty() {
            return Err(AuthError::BadRequest(
                "a token and password are required".into(),
            ));
        }
        if password.chars().count() < self.config.password_min_length {
            return Err(AuthError::BadRequest(format!(
                "the password must be at least {} characters",
                self.config.password_min_length
            )));
        }
        Ok(())
    }

    pub async fn devices(
        &self,
        caller: Uuid,
    ) -> Result<Vec<menzi_auth::record::SessionRecord>, AuthError> {
        self.sessions
            .list_for(caller)
            .await
            .map_err(Self::store_error)
    }

    pub async fn revoke_device(&self, caller: Uuid, id: Uuid) -> Result<(), AuthError> {
        let devices = self.devices(caller).await?;
        if !devices.iter().any(|device| device.id == id) {
            return Err(AuthError::NotFound);
        }
        self.sessions.revoke(id).await.map_err(Self::store_error)
    }

    pub async fn purge_expired(&self) -> Result<u64, AuthError> {
        self.sessions
            .purge_expired(self.clock.now())
            .await
            .map_err(Self::store_error)
    }

    pub fn safe_target(&self, requested: Option<&str>) -> String {
        let Some(requested) = requested else {
            return "/projects".to_string();
        };
        if !requested.starts_with('/') || requested.starts_with("//") {
            return "/projects".to_string();
        }
        requested.to_string()
    }

    pub fn password_minimum(&self) -> usize {
        self.config.password_min_length
    }

    pub async fn describe(&self, id: Uuid) -> Option<super::session::CurrentUser> {
        let user = self.accounts.find_by_id(id).await.ok()??;
        Some(super::session::CurrentUser {
            id: user.id.to_string(),
            kind: "user",
            email: Some(user.email),
            name: Some(user.name),
            avatar_url: user.avatar_url,
            role: self.role_of(user.id).await,
            has_password: user.password_hash.is_some(),
        })
    }

    pub async fn role_of(&self, id: Uuid) -> Option<String> {
        self.roles.highest_for(id).await
    }

    pub fn with_roles(mut self, roles: Arc<dyn menzi_auth::role_store::RoleStore>) -> Self {
        self.roles = roles;
        self
    }

    pub async fn session_id_for(&self, token: &str) -> Option<Uuid> {
        self.sessions
            .find_active(&menzi_auth::secret::digest(token))
            .await
            .ok()?
            .map(|session| session.id)
    }
}
