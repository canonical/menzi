use menzi_events::coordinator::{unix_now, LeaseCoordinator};
use menzi_events::nats_kv::NatsKvLeaseStore;
use std::sync::Arc;

fn nats_url() -> Option<String> {
    std::env::var("MENZI_TEST_NATS_URL").ok()
}

fn unique_bucket() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("mz_coord_{}_{}", std::process::id(), n)
}

async fn connect(bucket: &str) -> menzi_common::Result<Arc<NatsKvLeaseStore>> {
    let client = async_nats::connect(&nats_url().expect("nats url"))
        .await
        .expect("connect to nats");
    let jetstream = async_nats::jetstream::new(client);
    let store = NatsKvLeaseStore::connect(&jetstream, bucket)
        .await
        .expect("create kv bucket");
    Ok(Arc::new(store))
}

#[tokio::test]
async fn leader_election_has_single_leader() {
    let Some(_) = nats_url() else {
        eprintln!("skipping: MENZI_TEST_NATS_URL not set");
        return;
    };
    let store = connect(&unique_bucket()).await.expect("store");
    let first = LeaseCoordinator::new(store.clone(), "lead:orchestrator", 30);
    let second = LeaseCoordinator::new(store.clone(), "lead:orchestrator", 30);
    let now = unix_now();

    assert!(first.try_acquire(now).await.expect("first acquire"));
    assert!(!second.try_acquire(now).await.expect("second acquire"));
    assert!(first.is_leader(now).await.expect("first is leader"));
    assert!(!second.is_leader(now).await.expect("second is follower"));
}

#[tokio::test]
async fn work_lease_release_allows_takeover() {
    let Some(_) = nats_url() else {
        eprintln!("skipping: MENZI_TEST_NATS_URL not set");
        return;
    };
    let store = connect(&unique_bucket()).await.expect("store");
    let first = LeaseCoordinator::new(store.clone(), "work:env-dev", 30);
    let second = LeaseCoordinator::new(store.clone(), "work:env-dev", 30);
    let now = unix_now();

    assert!(first.try_acquire(now).await.expect("first acquire"));
    assert!(!second.try_acquire(now).await.expect("second blocked"));
    first.release().await.expect("release");
    assert!(second
        .try_acquire(now)
        .await
        .expect("second acquire after release"));
}

#[tokio::test]
async fn work_lease_renew_and_expiry() {
    let Some(_) = nats_url() else {
        eprintln!("skipping: MENZI_TEST_NATS_URL not set");
        return;
    };
    let store = connect(&unique_bucket()).await.expect("store");
    let coordinator = LeaseCoordinator::new(store.clone(), "work:env-dev", 1);
    let now = unix_now();

    assert!(coordinator.try_acquire(now).await.expect("acquire"));
    assert!(coordinator
        .renew(now + 4)
        .await
        .expect("renew before expiry"));
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert!(!coordinator.is_leader(unix_now()).await.expect("expired"));

    let other = LeaseCoordinator::new(store, "work:env-dev", 30);
    assert!(other
        .try_acquire(unix_now())
        .await
        .expect("takeover after expiry"));
}
