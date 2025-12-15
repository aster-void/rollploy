mod actors;
mod docker;
mod git;

use actors::{Deployer, DeployerArgs};
use clap::Parser;
use ractor::Actor;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "rollploy")]
#[command(about = "Pull-based rolling-release deployment system")]
struct Cli {
    /// Git repository URL
    #[arg(long)]
    repo: String,

    /// Branch to track
    #[arg(long, default_value = "main")]
    branch: String,

    /// Docker compose file path (relative to repo root)
    #[arg(long, default_value = "docker-compose.yml")]
    compose: String,

    /// Poll interval in seconds
    #[arg(long, default_value = "60")]
    interval: u64,

    /// Local directory to clone repo into
    #[arg(long)]
    dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let local_path = cli.dir.unwrap_or_else(|| {
        let repo_name = cli.repo.split('/').last().unwrap_or("repo");
        let repo_name = repo_name.trim_end_matches(".git");
        dirs::state_dir()
            .unwrap_or_else(|| PathBuf::from("/var/lib"))
            .join("rollploy")
            .join(repo_name)
    });

    let args = DeployerArgs {
        repo_url: cli.repo,
        branch: cli.branch,
        local_path,
        compose_file: cli.compose,
        interval: Duration::from_secs(cli.interval),
    };

    let (_actor, handle) = Actor::spawn(Some("deployer".to_string()), Deployer, args).await?;

    handle.await?;

    Ok(())
}
