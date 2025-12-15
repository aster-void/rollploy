use crate::{docker, git};
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info};

pub struct Deployer;

pub struct DeployerArgs {
    pub repo_url: String,
    pub branch: String,
    pub local_path: PathBuf,
    pub compose_file: String,
    pub interval: Duration,
}

pub struct State {
    repo_url: String,
    branch: String,
    local_path: PathBuf,
    compose_file: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    ForceRedeploy,
}

#[async_trait]
impl Actor for Deployer {
    type Msg = Message;
    type State = State;
    type Arguments = DeployerArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!(repo = %args.repo_url, "starting deployer");

        // Initial clone
        git::ensure_repo(&args.local_path, &args.repo_url, &args.branch)?;

        // Initial deploy
        docker::compose_up(&args.local_path, &args.compose_file)?;

        // Schedule periodic checks
        myself.send_interval(args.interval, || Message::Tick);

        Ok(State {
            repo_url: args.repo_url,
            branch: args.branch,
            local_path: args.local_path,
            compose_file: args.compose_file,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            Message::Tick => {
                info!(repo = %state.repo_url, "checking for updates");
                match git::pull(&state.local_path) {
                    Ok(true) => {
                        info!(repo = %state.repo_url, "updates found, deploying");
                        if let Err(e) = docker::compose_up(&state.local_path, &state.compose_file) {
                            error!(repo = %state.repo_url, error = %e, "deploy failed");
                        }
                    }
                    Ok(false) => {
                        info!(repo = %state.repo_url, "no updates");
                    }
                    Err(e) => {
                        error!(repo = %state.repo_url, error = %e, "git pull failed");
                    }
                }
            }
            Message::ForceRedeploy => {
                info!(repo = %state.repo_url, "force redeploying");
                if let Err(e) = docker::compose_up(&state.local_path, &state.compose_file) {
                    error!(repo = %state.repo_url, error = %e, "deploy failed");
                }
            }
        }
        Ok(())
    }
}
