use async_trait::async_trait;
use menzi_common::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const DEFAULT_LEASE_TTL_SECS: u64 = 30;

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lease {
    pub key: String,
    pub token: String,
    pub expires_at_unix_secs: u64,
}

impl Lease {
    pub fn new(key: &str, token: &str, now_unix_secs: u64, ttl_secs: u64) -> Self {
        Self {
            key: key.to_string(),
            token: token.to_string(),
            expires_at_unix_secs: now_unix_secs.saturating_add(ttl_secs),
        }
    }

    pub fn is_expired(&self, now_unix_secs: u64) -> bool {
        self.expires_at_unix_secs <= now_unix_secs
    }

    pub fn is_owned_by(&self, token: &str) -> bool {
        self.token == token
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLease {
    pub revision: u64,
    pub lease: Lease,
}

#[async_trait]
pub trait LeaseStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<StoredLease>>;
    async fn create(&self, key: &str, lease: &Lease) -> Result<bool>;
    async fn update(&self, key: &str, lease: &Lease, expected_revision: u64) -> Result<bool>;
    async fn delete(&self, key: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct InMemoryLeaseStore {
    entries: Mutex<HashMap<String, (u64, Lease)>>,
}

impl InMemoryLeaseStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LeaseStore for InMemoryLeaseStore {
    async fn get(&self, key: &str) -> Result<Option<StoredLease>> {
        let entries = self.entries.lock().expect("entries lock");
        Ok(entries.get(key).map(|(revision, lease)| StoredLease {
            revision: *revision,
            lease: lease.clone(),
        }))
    }

    async fn create(&self, key: &str, lease: &Lease) -> Result<bool> {
        let mut entries = self.entries.lock().expect("entries lock");
        if entries.contains_key(key) {
            return Ok(false);
        }
        entries.insert(key.to_string(), (1, lease.clone()));
        Ok(true)
    }

    async fn update(&self, key: &str, lease: &Lease, expected_revision: u64) -> Result<bool> {
        let mut entries = self.entries.lock().expect("entries lock");
        match entries.get(key) {
            Some((revision, _)) if *revision == expected_revision => {
                entries.insert(key.to_string(), (expected_revision + 1, lease.clone()));
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.entries.lock().expect("entries lock").remove(key);
        Ok(())
    }
}

pub struct LeaseCoordinator {
    store: Arc<dyn LeaseStore>,
    key: String,
    token: String,
    ttl_secs: u64,
}

impl LeaseCoordinator {
    pub fn new(store: Arc<dyn LeaseStore>, key: impl Into<String>, ttl_secs: u64) -> Self {
        Self {
            store,
            key: key.into(),
            token: Uuid::new_v4().to_string(),
            ttl_secs,
        }
    }

    pub async fn try_acquire(&self, now_unix_secs: u64) -> Result<bool> {
        match self.store.get(&self.key).await? {
            Some(stored) => {
                if !stored.lease.is_expired(now_unix_secs) {
                    return Ok(false);
                }
                let lease = Lease::new(&self.key, &self.token, now_unix_secs, self.ttl_secs);
                Ok(self
                    .store
                    .update(&self.key, &lease, stored.revision)
                    .await?)
            }
            None => {
                let lease = Lease::new(&self.key, &self.token, now_unix_secs, self.ttl_secs);
                Ok(self.store.create(&self.key, &lease).await?)
            }
        }
    }

    pub async fn renew(&self, now_unix_secs: u64) -> Result<bool> {
        let Some(stored) = self.store.get(&self.key).await? else {
            return Ok(false);
        };
        if !stored.lease.is_owned_by(&self.token) {
            return Ok(false);
        }
        let lease = Lease::new(&self.key, &self.token, now_unix_secs, self.ttl_secs);
        Ok(self
            .store
            .update(&self.key, &lease, stored.revision)
            .await?)
    }

    pub async fn release(&self) -> Result<()> {
        let Some(stored) = self.store.get(&self.key).await? else {
            return Ok(());
        };
        if stored.lease.is_owned_by(&self.token) {
            self.store.delete(&self.key).await?;
        }
        Ok(())
    }

    pub async fn is_leader(&self, now_unix_secs: u64) -> Result<bool> {
        let Some(stored) = self.store.get(&self.key).await? else {
            return Ok(false);
        };
        Ok(stored.lease.is_owned_by(&self.token) && !stored.lease.is_expired(now_unix_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Arc<InMemoryLeaseStore> {
        Arc::new(InMemoryLeaseStore::new())
    }

    #[test]
    fn lease_expires_at_ttl_boundary() {
        let lease = Lease::new("key", "tok", 100, 30);
        assert!(!lease.is_expired(129));
        assert!(lease.is_expired(130));
    }

    #[test]
    fn lease_owned_by_token() {
        let lease = Lease::new("key", "a", 100, 30);
        assert!(lease.is_owned_by("a"));
        assert!(!lease.is_owned_by("b"));
    }

    #[tokio::test]
    async fn acquire_succeeds_when_key_is_free() {
        let coordinator = LeaseCoordinator::new(store(), "work:env-1", 30);
        assert!(coordinator.try_acquire(100).await.unwrap());
        assert!(coordinator.is_leader(100).await.unwrap());
    }

    #[tokio::test]
    async fn second_holder_cannot_acquire() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let first = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        let second = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        assert!(first.try_acquire(100).await.unwrap());
        assert!(!second.try_acquire(100).await.unwrap());
        assert!(!second.is_leader(100).await.unwrap());
    }

    #[tokio::test]
    async fn expired_lease_can_be_taken_over() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let first = LeaseCoordinator::new(store.clone(), "work:env-1", 10);
        let second = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        assert!(first.try_acquire(100).await.unwrap());
        assert!(!second.try_acquire(109).await.unwrap());
        assert!(second.try_acquire(110).await.unwrap());
        assert!(!first.is_leader(110).await.unwrap());
        assert!(second.is_leader(110).await.unwrap());
    }

    #[tokio::test]
    async fn renew_extends_ownership() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let coordinator = LeaseCoordinator::new(store.clone(), "lead:orchestrator", 30);
        assert!(coordinator.try_acquire(100).await.unwrap());
        assert!(coordinator.renew(120).await.unwrap());
        assert!(coordinator.is_leader(140).await.unwrap());
    }

    #[tokio::test]
    async fn renew_fails_when_not_owner() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let first = LeaseCoordinator::new(store.clone(), "lead:orchestrator", 30);
        let second = LeaseCoordinator::new(store.clone(), "lead:orchestrator", 30);
        assert!(first.try_acquire(100).await.unwrap());
        assert!(!second.renew(120).await.unwrap());
    }

    #[tokio::test]
    async fn release_frees_the_key() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let first = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        let second = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        assert!(first.try_acquire(100).await.unwrap());
        first.release().await.unwrap();
        assert!(second.try_acquire(110).await.unwrap());
    }

    #[tokio::test]
    async fn release_by_non_owner_is_noop() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let first = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        let second = LeaseCoordinator::new(store.clone(), "work:env-1", 30);
        assert!(first.try_acquire(100).await.unwrap());
        second.release().await.unwrap();
        assert!(first.is_leader(100).await.unwrap());
    }

    #[tokio::test]
    async fn unowned_renew_returns_false() {
        let store = Arc::new(InMemoryLeaseStore::new());
        let coordinator = LeaseCoordinator::new(store, "work:env-1", 30);
        assert!(!coordinator.renew(100).await.unwrap());
    }
}
