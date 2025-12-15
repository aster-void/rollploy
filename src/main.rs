mod actors;
mod cron;
mod docker;
mod git;
mod state;
mod traefik;

use actors::{Deployer, DeployerArgs};
use clap::{Parser, Subcommand};
use cron::{CronRunner, CronRunnerArgs};
use ractor::Actor;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "rollploy")]
#[command(about = "Pull-based deployment and cron runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deploy a docker-compose app with blue-green strategy
    Deploy {
        /// Git repository URL
        #[arg(long)]
        repo: String,

        /// Branch to track
        #[arg(long, default_value = "main")]
        branch: String,

        /// Docker compose file path (relative to repo root)
        #[arg(long, default_value = "docker-compose.yml")]
        compose: String,

        /// Port to expose the app on
        #[arg(long)]
        port: u16,

        /// Poll interval in seconds
        #[arg(long, default_value = "60")]
        interval: u64,

        /// Health check timeout in seconds
        #[arg(long, default_value = "120")]
        health_timeout: u64,

        /// Local directory to clone repo into
        #[arg(long)]
        dir: Option<PathBuf>,
    },

    /// Run cron jobs from a git repository
    Cron {
        /// Git repository URL
        #[arg(long)]
        repo: String,

        /// Branch to track
        #[arg(long, default_value = "main")]
        branch: String,

        /// Git pull interval in seconds
        #[arg(long, default_value = "60")]
        interval: u64,

        /// Local directory to clone repo into
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Deploy {
            repo,
            branch,
            compose,
            port,
            interval,
            health_timeout,
            dir,
        } => {
            let local_path = dir.unwrap_or_else(|| derive_local_path(&repo));

            let args = DeployerArgs {
                repo_url: repo,
                branch,
                local_path,
                compose_file: compose,
                port,
                interval: Duration::from_secs(interval),
                health_timeout: Duration::from_secs(health_timeout),
            };

            let (_actor, handle) =
                Actor::spawn(Some("deployer".to_string()), Deployer, args).await?;
            handle.await?;
        }

        Commands::Cron {
            repo,
            branch,
            interval,
            dir,
        } => {
            let local_path = dir.unwrap_or_else(|| derive_local_path(&repo));

            let args = CronRunnerArgs {
                repo_url: repo,
                branch,
                local_path,
                check_interval: Duration::from_secs(interval),
            };

            let (_actor, handle) =
                Actor::spawn(Some("cron-runner".to_string()), CronRunner, args).await?;
            handle.await?;
        }
    }

    Ok(())
}

fn derive_local_path(repo: &str) -> PathBuf {
    let repo_name = repo.split('/').last().unwrap_or("repo");
    let repo_name = repo_name.trim_end_matches(".git");
    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("/var/lib"))
        .join("rollploy")
        .join(repo_name)
}
