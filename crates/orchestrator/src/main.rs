use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let lxd = Arc::new(
        menzi_lxd::HttpLxdClient::connect(
            &config.lxd_url,
            "default",
            config.lxd_cert_path.as_deref(),
            config.lxd_key_path.as_deref(),
        )
        .expect("Failed to connect to LXD"),
    );

    let driver = Arc::new(menzi_orchestrator::CollocateDriver::new(lxd));

    let nats = async_nats::connect(&config.nats_url)
        .await
        .expect("Failed to connect to NATS");
    let jetstream = async_nats::jetstream::new(nats);
    let lease_store = menzi_events::NatsKvLeaseStore::connect(&jetstream, "menzi_leases")
        .await
        .expect("Failed to open lease store");
    let gate = Arc::new(menzi_orchestrator::CoordinatorGate::new(Arc::new(
        lease_store,
    )));

    let state = menzi_orchestrator::OrchestratorState::with_gate(driver, gate);

    let app = menzi_orchestrator::create_router(state);

    info!("Listening on {}", config.gateway_bind);

    let listener = TcpListener::bind(&config.gateway_bind)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
