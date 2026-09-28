use async_nats::jetstream::kv::{Config, Operation, Store};
use async_nats::jetstream::Context;
use async_trait::async_trait;
use menzi_common::{MenziError, Result};

use crate::coordinator::{Lease, LeaseStore, StoredLease};

pub struct NatsKvLeaseStore {
    store: Store,
}

impl NatsKvLeaseStore {
    pub async fn connect(context: &Context, bucket: &str) -> Result<Self> {
        let store = context
            .create_key_value(Config {
                bucket: bucket.to_string(),
                ..Default::default()
            })
            .await
            .map_err(kv_error)?;
        Ok(Self { store })
    }
}

fn kv_error(error: impl std::fmt::Display) -> MenziError {
    MenziError::Internal(anyhow::Error::msg(format!("nats kv: {error}")))
}

#[async_trait]
impl LeaseStore for NatsKvLeaseStore {
    async fn get(&self, key: &str) -> Result<Option<StoredLease>> {
        match self.store.entry(key).await {
            Ok(Some(entry)) if entry.operation == Operation::Put => {
                let lease = serde_json::from_slice(&entry.value).map_err(kv_error)?;
                Ok(Some(StoredLease {
                    revision: entry.revision,
                    lease,
                }))
            }
            Ok(_) => Ok(None),
            Err(error) => Err(kv_error(error)),
        }
    }

    async fn create(&self, key: &str, lease: &Lease) -> Result<bool> {
        let payload: bytes::Bytes = serde_json::to_vec(&lease).map_err(kv_error)?.into();
        match self.store.create(key, payload).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn update(&self, key: &str, lease: &Lease, expected_revision: u64) -> Result<bool> {
        let payload: bytes::Bytes = serde_json::to_vec(&lease).map_err(kv_error)?.into();
        match self.store.update(key, payload, expected_revision).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.store.delete(key).await.map_err(kv_error)
    }
}
