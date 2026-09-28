use tracing::info;

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let config = menzi_common::config::Config::from_env().expect("Failed to load config");

    info!("Supervisor starting for session");

    if let Err(error) = menzi_supervisor::run(&config).await {
        tracing::error!("supervisor failed: {error}");
        std::process::exit(1);
    }
}
