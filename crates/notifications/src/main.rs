use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let app = menzi_notifications::create_router();

    info!("Listening on {}", config.gateway_bind);

    let listener = TcpListener::bind(&config.gateway_bind)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
