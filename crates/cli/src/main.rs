use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser)]
#[command(name = "menzi")]
#[command(about = "Menzi CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Login,
    Launch {
        project_id: String,
        #[arg(short, long)]
        branch: Option<String>,
        #[arg(short, long)]
        task: Option<String>,
    },
    Preview {
        project_id: String,
        #[arg(short, long)]
        commit_sha: Option<String>,
        #[arg(short, long)]
        branch: Option<String>,
        #[arg(short, long)]
        mode: Option<String>,
    },
    Forward {
        session_id: String,
        exposure_name: String,
        #[arg(short, long)]
        local_port: u16,
    },
    Design {
        project_id: String,
        #[arg(short, long)]
        revision: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    menzi_common::telemetry::init("info");

    let cli = Cli::parse();

    match cli.command {
        Commands::Login => {
            info!("Login command");
        }
        Commands::Launch { project_id, .. } => {
            info!("Launch command for project {}", project_id);
        }
        Commands::Preview { project_id, .. } => {
            info!("Preview command for project {}", project_id);
        }
        Commands::Forward { session_id, .. } => {
            info!("Forward command for session {}", session_id);
        }
        Commands::Design { project_id, .. } => {
            info!("Design command for project {}", project_id);
        }
    }
}
