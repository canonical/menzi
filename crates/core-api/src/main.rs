use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let db = menzi_db::Database::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    let previews = Arc::new(menzi_core_api::modules::previews_proxy::PreviewProxy::from_env());
    let workspaces =
        Arc::new(menzi_core_api::modules::workspaces_proxy::WorkspaceProxy::from_env());

    let app = menzi_core_api::create_router()
        .layer(axum::extract::Extension(workspaces.caller_resolver()))
        .layer(axum::extract::Extension(previews))
        .layer(axum::extract::Extension(workspaces))
        .with_state(db.pool().clone());

    info!("Listening on {}", config.gateway_bind);

    let listener = TcpListener::bind(&config.gateway_bind)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
