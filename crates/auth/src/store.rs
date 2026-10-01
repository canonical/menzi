use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::record::{IdentityLink, NewSession, OidcState, SessionRecord, UserRecord};

pub const PROVIDER_PASSWORD: &str = "password";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordPolicyError {
    TooShort { minimum: usize },
    MatchesEmail,
}

pub fn check_password_policy(
    password: &str,
    minimum: usize,
    email: &str,
) -> Result<(), PasswordPolicyError> {
    if password.chars().count() < minimum {
        return Err(PasswordPolicyError::TooShort { minimum });
    }
    let local = email.split('@').next().unwrap_or_default();
    if !local.is_empty() && password.eq_ignore_ascii_case(local) {
        return Err(PasswordPolicyError::MatchesEmail);
    }
    Ok(())
}

#[async_trait]
pub trait AccountStore: Send + Sync {
    async fn find_by_email(&self, email: &str) -> Result<Option<UserRecord>, StoreError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<UserRecord>, StoreError>;
    async fn create(
        &self,
        email: &str,
        name: &str,
        password_hash: Option<&str>,
        verified: bool,
    ) -> Result<UserRecord, StoreError>;
    async fn set_password_hash(&self, id: Uuid, hash: &str) -> Result<(), StoreError>;
    async fn find_by_identity(
        &self,
        provider_id: &str,
        subject: &str,
    ) -> Result<Option<UserRecord>, StoreError>;
    async fn link_identity(&self, link: IdentityLink) -> Result<(), StoreError>;
    async fn list_identities(&self, user_id: Uuid) -> Result<Vec<IdentityLink>, StoreError>;
    async fn unlink_identity(&self, user_id: Uuid, provider_id: &str) -> Result<bool, StoreError>;
}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: NewSession) -> Result<SessionRecord, StoreError>;
    async fn find_active(&self, token_hash: &str) -> Result<Option<SessionRecord>, StoreError>;
    async fn touch(&self, id: Uuid) -> Result<(), StoreError>;
    async fn revoke(&self, id: Uuid) -> Result<(), StoreError>;
    async fn revoke_all_for(&self, user_id: Uuid, keep: Option<Uuid>) -> Result<u64, StoreError>;
    async fn list_for(&self, user_id: Uuid) -> Result<Vec<SessionRecord>, StoreError>;
    async fn purge_expired(&self, now: DateTime<Utc>) -> Result<u64, StoreError>;
}

#[async_trait]
pub trait OidcStateStore: Send + Sync {
    async fn put(&self, state: OidcState) -> Result<(), StoreError>;
    async fn take(&self, state: &str) -> Result<Option<OidcState>, StoreError>;
}

#[async_trait]
pub trait ThrottleStore: Send + Sync {
    async fn failures_since(&self, email: &str, since: DateTime<Utc>) -> Result<i64, StoreError>;
    async fn record(&self, email: &str, succeeded: bool) -> Result<(), StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("duplicate: {0}")]
    Duplicate(String),
    #[error("not found")]
    NotFound,
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Default)]
pub struct InMemoryAccountStore {
    by_id: Mutex<HashMap<Uuid, UserRecord>>,
    identities: Mutex<HashMap<(String, String), Uuid>>,
}

impl InMemoryAccountStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AccountStore for InMemoryAccountStore {
    async fn find_by_email(&self, email: &str) -> Result<Option<UserRecord>, StoreError> {
        let wanted = crate::record::normalise_email(email);
        let by_id = self.by_id.lock().expect("account store lock");
        Ok(by_id.values().find(|user| user.email == wanted).cloned())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<UserRecord>, StoreError> {
        Ok(self
            .by_id
            .lock()
            .expect("account store lock")
            .get(&id)
            .cloned())
    }

    async fn create(
        &self,
        email: &str,
        name: &str,
        password_hash: Option<&str>,
        verified: bool,
    ) -> Result<UserRecord, StoreError> {
        let normalised = crate::record::normalise_email(email);
        let now = Utc::now();
        let record = UserRecord {
            id: Uuid::new_v4(),
            email: normalised.clone(),
            name: name.trim().to_string(),
            avatar_url: None,
            password_hash: password_hash.map(str::to_string),
            email_verified_at: verified.then_some(now),
            created_at: now,
        };
        let mut by_id = self.by_id.lock().expect("account store lock");
        if by_id.values().any(|user| user.email == normalised) {
            return Err(StoreError::Duplicate(normalised));
        }
        by_id.insert(record.id, record.clone());
        Ok(record)
    }

    async fn set_password_hash(&self, id: Uuid, hash: &str) -> Result<(), StoreError> {
        let mut by_id = self.by_id.lock().expect("account store lock");
        match by_id.get_mut(&id) {
            Some(user) => {
                user.password_hash = Some(hash.to_string());
                Ok(())
            }
            None => Err(StoreError::NotFound),
        }
    }

    async fn find_by_identity(
        &self,
        provider_id: &str,
        subject: &str,
    ) -> Result<Option<UserRecord>, StoreError> {
        let owner = self
            .identities
            .lock()
            .expect("identity store lock")
            .get(&(provider_id.to_string(), subject.to_string()))
            .copied();
        match owner {
            Some(user_id) => self.find_by_id(user_id).await,
            None => Ok(None),
        }
    }

    async fn link_identity(&self, link: IdentityLink) -> Result<(), StoreError> {
        let key = (link.provider_id.clone(), link.subject.clone());
        self.identities
            .lock()
            .expect("identity store lock")
            .entry(key)
            .or_insert(link.user_id);
        Ok(())
    }

    async fn list_identities(&self, user_id: Uuid) -> Result<Vec<IdentityLink>, StoreError> {
        let identities = self.identities.lock().expect("identity store lock");
        let mut found: Vec<IdentityLink> = identities
            .iter()
            .filter(|(_, owner)| **owner == user_id)
            .map(|((provider_id, subject), owner)| IdentityLink {
                user_id: *owner,
                provider_id: provider_id.clone(),
                subject: subject.clone(),
                email_at_link: None,
            })
            .collect();
        found.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        Ok(found)
    }

    async fn unlink_identity(&self, user_id: Uuid, provider_id: &str) -> Result<bool, StoreError> {
        let mut identities = self.identities.lock().expect("identity store lock");
        let key: Vec<(String, String)> = identities
            .iter()
            .filter(|((provider, _), owner)| provider == provider_id && **owner == user_id)
            .map(|(key, _)| key.clone())
            .collect();
        let removed = !key.is_empty();
        for key in key {
            identities.remove(&key);
        }
        Ok(removed)
    }
}

struct StoredSession {
    token_hash: String,
    record: SessionRecord,
    revoked: bool,
}

#[derive(Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<HashMap<Uuid, StoredSession>>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create(&self, session: NewSession) -> Result<SessionRecord, StoreError> {
        let now = Utc::now();
        let record = SessionRecord {
            id: Uuid::new_v4(),
            user_id: session.user_id,
            provider_id: session.provider_id.clone(),
            created_at: now,
            expires_at: session.expires_at,
            last_seen_at: now,
            user_agent: session.user_agent.clone(),
            ip: session.ip.clone(),
        };
        let mut sessions = self.sessions.lock().expect("session store lock");
        if sessions
            .values()
            .any(|entry| entry.token_hash == session.token_hash)
        {
            return Err(StoreError::Duplicate(session.token_hash));
        }
        sessions.insert(
            record.id,
            StoredSession {
                token_hash: session.token_hash,
                record: record.clone(),
                revoked: false,
            },
        );
        Ok(record)
    }

    async fn find_active(&self, token_hash: &str) -> Result<Option<SessionRecord>, StoreError> {
        let now = Utc::now();
        let sessions = self.sessions.lock().expect("session store lock");
        Ok(sessions
            .values()
            .find(|entry| {
                entry.token_hash == token_hash && !entry.revoked && entry.record.expires_at > now
            })
            .map(|entry| entry.record.clone()))
    }

    async fn touch(&self, id: Uuid) -> Result<(), StoreError> {
        let mut sessions = self.sessions.lock().expect("session store lock");
        if let Some(entry) = sessions.get_mut(&id) {
            entry.record.last_seen_at = Utc::now();
        }
        Ok(())
    }

    async fn revoke(&self, id: Uuid) -> Result<(), StoreError> {
        let mut sessions = self.sessions.lock().expect("session store lock");
        if let Some(entry) = sessions.get_mut(&id) {
            entry.revoked = true;
        }
        Ok(())
    }

    async fn revoke_all_for(&self, user_id: Uuid, keep: Option<Uuid>) -> Result<u64, StoreError> {
        let mut sessions = self.sessions.lock().expect("session store lock");
        let mut count = 0;
        for entry in sessions.values_mut() {
            if entry.record.user_id == user_id && !entry.revoked && Some(entry.record.id) != keep {
                entry.revoked = true;
                count += 1;
            }
        }
        Ok(count)
    }

    async fn list_for(&self, user_id: Uuid) -> Result<Vec<SessionRecord>, StoreError> {
        let sessions = self.sessions.lock().expect("session store lock");
        let mut found: Vec<SessionRecord> = sessions
            .values()
            .filter(|entry| entry.record.user_id == user_id)
            .map(|entry| entry.record.clone())
            .collect();
        found.sort_by_key(|record| std::cmp::Reverse(record.created_at));
        Ok(found)
    }

    async fn purge_expired(&self, now: DateTime<Utc>) -> Result<u64, StoreError> {
        let mut sessions = self.sessions.lock().expect("session store lock");
        let before = sessions.len();
        sessions.retain(|_, entry| entry.record.expires_at > now);
        Ok((before - sessions.len()) as u64)
    }
}

#[derive(Default)]
pub struct InMemoryOidcStateStore {
    states: Mutex<HashMap<String, OidcState>>,
}

impl InMemoryOidcStateStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OidcStateStore for InMemoryOidcStateStore {
    async fn put(&self, state: OidcState) -> Result<(), StoreError> {
        self.states
            .lock()
            .expect("oidc state store lock")
            .insert(state.state.clone(), state);
        Ok(())
    }

    async fn take(&self, state: &str) -> Result<Option<OidcState>, StoreError> {
        let now = Utc::now();
        let mut states = self.states.lock().expect("oidc state store lock");
        match states.get(state) {
            Some(found) if !found.is_expired(now) => {
                let taken = found.clone();
                states.remove(state);
                Ok(Some(taken))
            }
            Some(_) => {
                states.remove(state);
                Ok(None)
            }
            None => Ok(None),
        }
    }
}

#[derive(Default)]
pub struct InMemoryThrottleStore {
    attempts: Mutex<Vec<(String, bool, DateTime<Utc>)>>,
}

impl InMemoryThrottleStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ThrottleStore for InMemoryThrottleStore {
    async fn failures_since(&self, email: &str, since: DateTime<Utc>) -> Result<i64, StoreError> {
        let wanted = crate::record::normalise_email(email);
        let attempts = self.attempts.lock().expect("throttle store lock");
        Ok(attempts
            .iter()
            .filter(|(recorded, succeeded, at)| *recorded == wanted && !*succeeded && *at > since)
            .count() as i64)
    }

    async fn record(&self, email: &str, succeeded: bool) -> Result<(), StoreError> {
        self.attempts.lock().expect("throttle store lock").push((
            crate::record::normalise_email(email),
            succeeded,
            Utc::now(),
        ));
        Ok(())
    }
}

pub fn dummy_password_hash() -> String {
    crate::password::PasswordHash::hash(DUMMY_PASSWORD)
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default()
}

pub const DUMMY_PASSWORD: &str = "not-a-real-password";

pub fn token_hash_of(secret: &crate::secret::SessionSecret) -> String {
    secret.digest()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::SessionSecret;
    use chrono::Duration;

    fn new_session() -> (NewSession, SessionSecret) {
        new_session_for(Uuid::new_v4())
    }

    fn new_session_for(user_id: Uuid) -> (NewSession, SessionSecret) {
        let secret = SessionSecret::mint().unwrap();
        (
            NewSession {
                user_id,
                token_hash: secret.digest(),
                csrf_hash: "csrf".to_string(),
                provider_id: PROVIDER_PASSWORD.to_string(),
                expires_at: Utc::now() + Duration::hours(1),
                user_agent: None,
                ip: None,
            },
            secret,
        )
    }

    #[tokio::test]
    async fn a_created_session_is_found_by_its_digest_only() {
        let store = InMemorySessionStore::new();
        let (new, secret) = new_session();
        store.create(new).await.unwrap();
        assert!(store.find_active(&secret.digest()).await.unwrap().is_some());
        assert!(store.find_active(secret.expose()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_revoked_session_is_not_found() {
        let store = InMemorySessionStore::new();
        let (new, secret) = new_session();
        let record = store.create(new).await.unwrap();
        store.revoke(record.id).await.unwrap();
        assert!(store.find_active(&secret.digest()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn an_expired_session_is_not_found() {
        let store = InMemorySessionStore::new();
        let (mut new, _) = new_session();
        let token_hash = new.token_hash.clone();
        new.expires_at = Utc::now() - Duration::minutes(1);
        store.create(new).await.unwrap();
        assert!(store.find_active(&token_hash).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn revoking_all_keeps_the_named_session() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        let first = new_session_for(user_id);
        let first_record = store.create(first.0.clone()).await.unwrap();
        let second = new_session_for(user_id);
        store.create(second.0.clone()).await.unwrap();

        let revoked = store
            .revoke_all_for(first_record.user_id, Some(first_record.id))
            .await
            .unwrap();

        assert_eq!(revoked, 1);
        assert!(store
            .find_active(&first.0.token_hash)
            .await
            .unwrap()
            .is_some());
        assert!(store
            .find_active(&second.0.token_hash)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn revoking_all_without_a_kept_session_revokes_everything() {
        let store = InMemorySessionStore::new();
        let user_id = Uuid::new_v4();
        let first = new_session_for(user_id);
        let first_record = store.create(first.0.clone()).await.unwrap();
        let second = new_session_for(user_id);
        store.create(second.0.clone()).await.unwrap();

        let revoked = store
            .revoke_all_for(first_record.user_id, None)
            .await
            .unwrap();

        assert_eq!(revoked, 2);
        assert!(store
            .find_active(&first.0.token_hash)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn purging_removes_only_expired_sessions() {
        let store = InMemorySessionStore::new();
        let live = new_session();
        let dead = new_session();
        let mut dead_new = dead.0;
        dead_new.expires_at = Utc::now() - Duration::minutes(1);
        let live_token = live.0.token_hash.clone();
        store.create(live.0).await.unwrap();
        store.create(dead_new).await.unwrap();

        let purged = store.purge_expired(Utc::now()).await.unwrap();

        assert_eq!(purged, 1);
        assert!(store.find_active(&live_token).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn an_oidc_state_is_taken_exactly_once() {
        let store = InMemoryOidcStateStore::new();
        let state = OidcState {
            state: "abc".to_string(),
            provider_id: "google".to_string(),
            nonce: "n".to_string(),
            code_verifier: "v".to_string(),
            redirect_to: None,
            expires_at: Utc::now() + Duration::minutes(10),
        };
        store.put(state).await.unwrap();
        assert!(store.take("abc").await.unwrap().is_some());
        assert!(store.take("abc").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn an_expired_oidc_state_is_not_taken() {
        let store = InMemoryOidcStateStore::new();
        store
            .put(OidcState {
                state: "abc".to_string(),
                provider_id: "google".to_string(),
                nonce: "n".to_string(),
                code_verifier: "v".to_string(),
                redirect_to: None,
                expires_at: Utc::now() - Duration::minutes(1),
            })
            .await
            .unwrap();
        assert!(store.take("abc").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn the_throttle_counts_only_recent_failures() {
        let store = InMemoryThrottleStore::new();
        store.record("A@B", false).await.unwrap();
        store.record("a@b", false).await.unwrap();
        store.record("a@b", true).await.unwrap();
        let count = store
            .failures_since("a@b", Utc::now() - Duration::minutes(15))
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn an_older_failure_is_outside_the_window() {
        let store = InMemoryThrottleStore::new();
        store.record("a@b", false).await.unwrap();
        let count = store
            .failures_since("a@b", Utc::now() + Duration::minutes(1))
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn a_duplicate_email_is_refused() {
        let store = InMemoryAccountStore::new();
        store.create("a@b", "A", None, false).await.unwrap();
        let again = store.create("A@B", "A", None, false).await;
        assert!(matches!(again, Err(StoreError::Duplicate(_))));
    }

    #[tokio::test]
    async fn an_identity_resolves_to_its_user() {
        let store = InMemoryAccountStore::new();
        let user = store.create("a@b", "A", None, true).await.unwrap();
        assert!(store
            .find_by_identity("google", "sub-1")
            .await
            .unwrap()
            .is_none());
        store
            .link_identity(IdentityLink {
                user_id: user.id,
                provider_id: "google".to_string(),
                subject: "sub-1".to_string(),
                email_at_link: None,
            })
            .await
            .unwrap();
        let found = store.find_by_identity("google", "sub-1").await.unwrap();
        assert_eq!(found.map(|u| u.id), Some(user.id));
    }

    #[tokio::test]
    async fn unlinking_removes_the_identity() {
        let store = InMemoryAccountStore::new();
        let user = store.create("a@b", "A", None, true).await.unwrap();
        store
            .link_identity(IdentityLink {
                user_id: user.id,
                provider_id: "google".to_string(),
                subject: "sub-1".to_string(),
                email_at_link: None,
            })
            .await
            .unwrap();
        assert!(store.unlink_identity(user.id, "google").await.unwrap());
        assert!(!store.unlink_identity(user.id, "google").await.unwrap());
    }

    #[tokio::test]
    async fn a_user_carries_no_organisation() {
        let store = InMemoryAccountStore::new();
        let user = store.create("a@b", "A", None, false).await.unwrap();
        assert_eq!(
            store.find_by_id(user.id).await.unwrap().unwrap().id,
            user.id
        );
    }

    #[test]
    fn the_password_policy_rejects_a_short_password() {
        assert_eq!(
            check_password_policy("short", 12, "a@b"),
            Err(PasswordPolicyError::TooShort { minimum: 12 })
        );
        assert!(check_password_policy("longenoughpw", 12, "a@b").is_ok());
    }

    #[test]
    fn the_password_policy_rejects_the_email_local_part() {
        assert_eq!(
            check_password_policy("bobsmithlong", 12, "BobSmithLong@example.com"),
            Err(PasswordPolicyError::MatchesEmail)
        );
        assert!(check_password_policy("somethingelse", 12, "bob@example.com").is_ok());
    }

    #[test]
    fn the_length_rule_applies_before_the_email_rule() {
        assert_eq!(
            check_password_policy("bob", 12, "bob@example.com"),
            Err(PasswordPolicyError::TooShort { minimum: 12 })
        );
    }

    #[test]
    fn a_dummy_hash_verifies_only_its_own_password() {
        let hash = dummy_password_hash();
        assert!(!hash.is_empty());
        assert!(crate::password::verify_stored(&hash, DUMMY_PASSWORD));
        assert!(!crate::password::verify_stored(&hash, "guess"));
    }
}
