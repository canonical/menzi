use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let factory = menzi_llm_gateway::ProviderAdapterFactory::default_for_env();
    let mut state = menzi_llm_gateway::GatewayState::new(
        factory,
        menzi_llm_gateway::PolicyEngine::new(),
        menzi_llm_gateway::BudgetManager::new(),
        menzi_llm_gateway::CostCalculator,
        menzi_llm_gateway::AuditLogger::new(),
        "openrouter",
    );

    if std::env::var("MENZI_LLM_STATE_BACKEND")
        .unwrap_or_else(|_| "postgres".to_string())
        == "postgres"
    {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&config.database_url)
            .await
        {
            Ok(pool) => {
                let store = menzi_llm_gateway::PostgresGatewayStateStore::new(pool);
                if let Err(error) = store.migrate().await {
                    tracing::warn!("llm gateway migration failed: {error}");
                } else {
                    state = state.with_store(std::sync::Arc::new(store));
                }
            }
            Err(error) => tracing::warn!("database unavailable, llm state stays in memory: {error}"),
        }
    }

    let app = menzi_llm_gateway::create_router(state);

    info!("Listening on {}", config.gateway_bind);

    let listener = TcpListener::bind(&config.gateway_bind)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
