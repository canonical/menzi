use async_trait::async_trait;
use menzi_common::Result;
use menzi_events::coordinator::{unix_now, LeaseCoordinator, LeaseStore, DEFAULT_LEASE_TTL_SECS};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait WorkGate: Send + Sync {
    async fn try_acquire(&self, work: &str) -> Result<bool>;
    async fn release(&self, work: &str) -> Result<()>;
}

pub struct AlwaysFreeGate;

#[async_trait]
impl WorkGate for AlwaysFreeGate {
    async fn try_acquire(&self, _work: &str) -> Result<bool> {
        Ok(true)
    }

    async fn release(&self, _work: &str) -> Result<()> {
        Ok(())
    }
}

pub struct CoordinatorGate {
    store: Arc<dyn LeaseStore>,
    coordinators: Mutex<HashMap<String, Arc<LeaseCoordinator>>>,
}

impl CoordinatorGate {
    pub fn new(store: Arc<dyn LeaseStore>) -> Self {
        Self {
            store,
            coordinators: Mutex::new(HashMap::new()),
        }
    }

    fn coordinator_for(&self, work: &str) -> Arc<LeaseCoordinator> {
        let mut coordinators = self.coordinators.lock().expect("coordinators lock");
        coordinators
            .entry(work.to_string())
            .or_insert_with(|| {
                Arc::new(LeaseCoordinator::new(
                    self.store.clone(),
                    format!("work:{work}"),
                    DEFAULT_LEASE_TTL_SECS,
                ))
            })
            .clone()
    }
}

#[async_trait]
impl WorkGate for CoordinatorGate {
    async fn try_acquire(&self, work: &str) -> Result<bool> {
        self.coordinator_for(work).try_acquire(unix_now()).await
    }

    async fn release(&self, work: &str) -> Result<()> {
        self.coordinator_for(work).release().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> CoordinatorGate {
        CoordinatorGate::new(Arc::new(
            menzi_events::coordinator::InMemoryLeaseStore::new(),
        ))
    }

    #[tokio::test]
    async fn free_gate_always_allows() {
        let gate = AlwaysFreeGate;
        assert!(gate.try_acquire("env-1").await.unwrap());
        gate.release("env-1").await.unwrap();
    }

    #[tokio::test]
    async fn coordinator_gate_allows_first_work() {
        let gate = gate();
        assert!(gate.try_acquire("env-1").await.unwrap());
    }

    #[tokio::test]
    async fn coordinator_gate_blocks_concurrent_same_work() {
        let gate = gate();
        assert!(gate.try_acquire("env-1").await.unwrap());
        assert!(!gate.try_acquire("env-1").await.unwrap());
    }

    #[tokio::test]
    async fn coordinator_gate_blocks_other_instances() {
        let store = Arc::new(menzi_events::coordinator::InMemoryLeaseStore::new());
        let first = CoordinatorGate::new(store.clone());
        let second = CoordinatorGate::new(store.clone());
        assert!(first.try_acquire("env-1").await.unwrap());
        assert!(!second.try_acquire("env-1").await.unwrap());
    }

    #[tokio::test]
    async fn coordinator_gate_release_frees_work() {
        let gate = gate();
        assert!(gate.try_acquire("env-1").await.unwrap());
        gate.release("env-1").await.unwrap();
        assert!(gate.try_acquire("env-1").await.unwrap());
    }

    #[tokio::test]
    async fn coordinator_gate_allows_different_work() {
        let gate = gate();
        assert!(gate.try_acquire("env-1").await.unwrap());
        assert!(gate.try_acquire("env-2").await.unwrap());
    }
}
