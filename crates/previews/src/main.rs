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

    let driver = Arc::new(menzi_previews::LxdPreviewDriver::new(lxd));
    let store: Arc<dyn menzi_previews::PreviewStore> = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&config.database_url)
        .await
    {
        Ok(pool) => {
            let store = menzi_previews::PostgresPreviewStore::new(pool);
            if let Err(error) = store.migrate().await {
                tracing::warn!("preview migration failed: {error}");
                Arc::new(menzi_previews::InMemoryPreviewStore::new())
            } else {
                Arc::new(store)
            }
        }
        Err(error) => {
            tracing::warn!("database unavailable, previews stay in memory: {error}");
            Arc::new(menzi_previews::InMemoryPreviewStore::new())
        }
    };
    let manager = Arc::new(menzi_previews::PreviewManager::new(driver, store));

    let source_instance =
        std::env::var("MENZI_SOURCE_INSTANCE").unwrap_or_else(|_| "mz-workspace".to_string());
    let bind = std::env::var("MENZI_PREVIEWS_BIND").unwrap_or_else(|_| "0.0.0.0:8095".to_string());
    let state = menzi_previews::PreviewApiState::new(manager, source_instance);

    let app = menzi_previews::create_router(state);

    info!("Listening on {bind}");

    let listener = TcpListener::bind(&bind).await.expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
