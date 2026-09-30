use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let opencode_url = std::env::var("MENZI_OPENCODE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:17999".to_string());

    let state = menzi_session_proxy::ProxyState::new(
        menzi_session_proxy::ProxyConfig::default_allowlist(),
        opencode_url,
    )
    .with_router(Arc::new(menzi_session_proxy::WorkspaceRouter::from_env()));

    let app = menzi_session_proxy::create_router(state);

    info!("Listening on {}", config.gateway_bind);

    let listener = TcpListener::bind(&config.gateway_bind)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
