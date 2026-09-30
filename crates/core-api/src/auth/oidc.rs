use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Mutex;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdTokenClaims {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

impl IdTokenClaims {
    pub fn verified_email(&self) -> Option<&str> {
        match self.email_verified {
            Some(true) => self.email.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("too many requests")]
    TooManyRequests,
    #[error("locked")]
    Locked,
    #[error("{0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProviderDescriptor {
    pub id: String,
    pub label: String,
}

#[async_trait]
pub trait OpenIdProvider: Send + Sync {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    async fn authorization_url(
        &self,
        challenge: &str,
        state: &str,
        nonce: &str,
    ) -> Result<String, AuthError>;
    async fn exchange(&self, code: &str, verifier: &str) -> Result<IdTokenClaims, AuthError>;
}

#[derive(Debug, Deserialize)]
struct ProviderFile {
    #[serde(default)]
    providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ProviderEntry {
    pub id: String,
    pub label: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
}

fn default_scopes() -> Vec<String> {
    vec![
        "openid".to_string(),
        "email".to_string(),
        "profile".to_string(),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistry {
    entries: Vec<ProviderEntry>,
}

impl ProviderRegistry {
    pub fn from_json(raw: &str) -> Result<Self, AuthError> {
        let parsed: ProviderFile = serde_json::from_str(raw)
            .map_err(|error| AuthError::Unavailable(format!("oidc providers: {error}")))?;
        Ok(Self {
            entries: parsed.providers,
        })
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn from_env() -> Self {
        if let Ok(path) = std::env::var("MENZI_OIDC_PROVIDERS_FILE") {
            if !path.trim().is_empty() {
                if let Ok(raw) = std::fs::read_to_string(&path) {
                    if let Ok(registry) = Self::from_json(&raw) {
                        return registry;
                    }
                }
            }
        }
        match std::env::var("MENZI_OIDC_PROVIDERS") {
            Ok(raw) if !raw.trim().is_empty() => {
                Self::from_json(&raw).unwrap_or_else(|_| Self::empty())
            }
            _ => Self::empty(),
        }
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.entries
            .iter()
            .map(|entry| ProviderDescriptor {
                id: entry.id.clone(),
                label: entry.label.clone(),
            })
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&ProviderEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[ProviderEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecordedRequest {
    pub challenge: String,
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    pub code: String,
}

pub struct FakeOidcProvider {
    id: String,
    label: String,
    claims: IdTokenClaims,
    fail_exchange: bool,
    recorded: Mutex<Option<RecordedRequest>>,
    calls: Mutex<usize>,
}

impl FakeOidcProvider {
    pub fn new(id: &str, claims: IdTokenClaims) -> Self {
        Self {
            id: id.to_string(),
            label: format!("{id} test"),
            claims,
            fail_exchange: false,
            recorded: Mutex::new(None),
            calls: Mutex::new(0),
        }
    }

    pub fn refusing(id: &str) -> Self {
        let mut provider = Self::new(
            id,
            IdTokenClaims {
                subject: "sub-1".to_string(),
                ..IdTokenClaims::default()
            },
        );
        provider.fail_exchange = true;
        provider
    }

    pub fn recorded(&self) -> Option<RecordedRequest> {
        self.recorded.lock().expect("provider lock").clone()
    }

    pub fn calls(&self) -> usize {
        *self.calls.lock().expect("provider lock")
    }

    pub fn claims(&self) -> IdTokenClaims {
        self.claims.clone()
    }
}

#[async_trait]
impl OpenIdProvider for FakeOidcProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    async fn authorization_url(
        &self,
        challenge: &str,
        state: &str,
        nonce: &str,
    ) -> Result<String, AuthError> {
        *self.calls.lock().expect("provider lock") += 1;
        *self.recorded.lock().expect("provider lock") = Some(RecordedRequest {
            challenge: challenge.to_string(),
            state: state.to_string(),
            nonce: nonce.to_string(),
            verifier: String::new(),
            code: String::new(),
        });
        Ok(format!(
            "https://{}.test/authorize?code_challenge={challenge}&code_challenge_method=S256&state={state}&nonce={nonce}",
            self.id
        ))
    }

    async fn exchange(&self, code: &str, verifier: &str) -> Result<IdTokenClaims, AuthError> {
        *self.calls.lock().expect("provider lock") += 1;
        if self.fail_exchange {
            return Err(AuthError::Unavailable("exchange refused".to_string()));
        }
        let mut slot = self.recorded.lock().expect("provider lock");
        let recorded = slot.get_or_insert_with(RecordedRequest::default);
        recorded.code = code.to_string();
        recorded.verifier = verifier.to_string();
        Ok(self.claims.clone())
    }
}

type ProviderClient = openidconnect::core::CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

pub struct RustOidcProvider {
    id: String,
    label: String,
    client: ProviderClient,
    issuer: String,
    scopes: Vec<openidconnect::Scope>,
    redirect_uri: String,
    expected_nonce: std::sync::Arc<std::sync::Mutex<Option<String>>>,
}

pub async fn rust_provider(
    entry: &ProviderEntry,
    config: &super::config::AuthConfig,
) -> Result<RustOidcProvider, AuthError> {
    let issuer = openidconnect::IssuerUrl::new(entry.issuer.clone())
        .map_err(|error| AuthError::Unavailable(format!("{}: {error}", entry.id)))?;
    let http = openidconnect::reqwest::Client::builder()
        .redirect(openidconnect::reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| AuthError::Unavailable(format!("{}: {error}", entry.id)))?;
    let metadata = openidconnect::core::CoreProviderMetadata::discover_async(issuer, &http)
        .await
        .map_err(|error| AuthError::Unavailable(format!("{}: {error}", entry.id)))?;
    let redirect_uri = config.redirect_uri(&entry.id);
    let client: ProviderClient = openidconnect::core::CoreClient::from_provider_metadata(
        metadata,
        openidconnect::ClientId::new(entry.client_id.clone()),
        Some(openidconnect::ClientSecret::new(
            entry.client_secret.clone(),
        )),
    )
    .set_redirect_uri(
        openidconnect::RedirectUrl::new(redirect_uri.clone())
            .map_err(|error| AuthError::Unavailable(format!("{}: {error}", entry.id)))?,
    );

    Ok(RustOidcProvider {
        id: entry.id.clone(),
        label: entry.label.clone(),
        client,
        issuer: entry.issuer.clone(),
        scopes: entry
            .scopes
            .iter()
            .map(|scope| openidconnect::Scope::new(scope.clone()))
            .collect(),
        redirect_uri: config.redirect_uri(&entry.id),
        expected_nonce: std::sync::Arc::new(std::sync::Mutex::new(None)),
    })
}

impl RustOidcProvider {
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }
}

#[async_trait]
impl OpenIdProvider for RustOidcProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn label(&self) -> &str {
        &self.label
    }

    async fn authorization_url(
        &self,
        challenge: &str,
        state: &str,
        nonce: &str,
    ) -> Result<String, AuthError> {
        let state = state.to_string();
        let nonce = nonce.to_string();
        *self.expected_nonce.lock().expect("nonce lock") = Some(nonce.clone());
        let verifier = openidconnect::PkceCodeVerifier::new(challenge.to_string());
        let (url, _csrf, _nonce) = self
            .client
            .authorize_url(
                openidconnect::AuthenticationFlow::<openidconnect::core::CoreResponseType>::AuthorizationCode,
                move || openidconnect::CsrfToken::new(state.clone()),
                move || openidconnect::Nonce::new(nonce.clone()),
            )
            .set_pkce_challenge(openidconnect::PkceCodeChallenge::from_code_verifier_sha256(
                &verifier,
            ))
            .add_scopes(self.scopes.clone())
            .url();
        Ok(url.to_string())
    }

    async fn exchange(&self, code: &str, verifier: &str) -> Result<IdTokenClaims, AuthError> {
        let http = openidconnect::reqwest::Client::builder()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AuthError::Unavailable(error.to_string()))?;
        let token = self
            .client
            .exchange_code(openidconnect::AuthorizationCode::new(code.to_string()))
            .map_err(|error| AuthError::Unavailable(error.to_string()))?
            .set_pkce_verifier(openidconnect::PkceCodeVerifier::new(verifier.to_string()))
            .request_async(&http)
            .await
            .map_err(|error| AuthError::Unavailable(error.to_string()))?;

        use openidconnect::TokenResponse;
        let id_token = token
            .id_token()
            .ok_or_else(|| AuthError::Unavailable("the provider returned no id token".into()))?;
        let verifier = self.client.id_token_verifier();
        let expected = self.expected_nonce.lock().expect("nonce lock").clone();
        let check = move |nonce: Option<&openidconnect::Nonce>| -> Result<(), String> {
            match (&expected, nonce) {
                (Some(wanted), Some(found)) if wanted == found.secret() => Ok(()),
                (None, _) => Ok(()),
                _ => Err("the id token nonce did not match the sign-in attempt".to_string()),
            }
        };
        let claims = id_token
            .claims(&verifier, &check)
            .map_err(|error| AuthError::Unavailable(error.to_string()))?;

        let subject = claims.subject().as_str().to_string();
        if subject.is_empty() {
            return Err(AuthError::Unavailable("the id token has no subject".into()));
        }
        let email = claims.email().map(|value| value.to_string());
        let email_verified = claims.email_verified();
        let name = claims
            .name()
            .and_then(|value| value.get(None).map(|v| v.to_string()));
        let picture = claims
            .picture()
            .and_then(|value| value.get(None).map(|v| v.to_string()));

        Ok(IdTokenClaims {
            subject,
            email,
            email_verified,
            name,
            picture,
        })
    }
}

pub fn pkce_digest(verifier: &str) -> String {
    use base64::Engine;
    let digest = menzi_auth::secret::sha256(verifier);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

pub fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    let digest = menzi_auth::secret::sha256(verifier);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}
