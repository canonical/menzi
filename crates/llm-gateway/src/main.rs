use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    let factory = menzi_llm_gateway::ProviderAdapterFactory::default_for_env();
    let state = menzi_llm_gateway::GatewayState::new(
        factory,
        menzi_llm_gateway::PolicyEngine::new(),
        menzi_llm_gateway::BudgetManager::new(),
        menzi_llm_gateway::CostCalculator,
        menzi_llm_gateway::AuditLogger::new(),
        "openrouter",
    );

    let app = menzi_llm_gateway::create_router(state);

    info!("Listening on {}", config.gateway_bind);

    let listener = TcpListener::bind(&config.gateway_bind)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app).await.expect("Server failed");
}
