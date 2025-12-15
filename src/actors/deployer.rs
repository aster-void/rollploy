use crate::state::Slot;
use crate::{docker, git, state, traefik};
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct Deployer;

pub struct DeployerArgs {
    pub repo_url: String,
    pub branch: String,
    pub local_path: PathBuf,
    pub compose_file: String,
    pub port: u16,
    pub interval: Duration,
    pub health_timeout: Duration,
}

pub struct State {
    repo_url: String,
    local_path: PathBuf,
    compose_file: String,
    active_slot: Slot,
    health_timeout: Duration,
    app_name: String,
    network: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
}

impl State {
    fn project_name(&self, slot: Slot) -> String {
        format!("{}-{}", self.app_name, slot.as_str())
    }
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
        info!(repo = %args.repo_url, port = args.port, "starting deployer");

        // Derive app name from repo
        let app_name = args
            .repo_url
            .split('/')
            .last()
            .unwrap_or("app")
            .trim_end_matches(".git")
            .to_string();

        let network = format!("rollploy-{}", app_name);

        // Setup infrastructure
        docker::create_network(&network)?;
        traefik::start(&app_name, args.port, &network)?;

        // Clone repo
        git::ensure_repo(&args.local_path, &args.repo_url, &args.branch)?;

        // Load persisted state
        let persisted = state::load(&args.local_path).unwrap_or_default();

        let state = State {
            repo_url: args.repo_url,
            local_path: args.local_path,
            compose_file: args.compose_file,
            active_slot: persisted.active_slot,
            health_timeout: args.health_timeout,
            app_name,
            network,
        };

        // Initial deploy
        if let Err(e) = deploy(&state, state.active_slot) {
            error!(error = %e, "initial deploy failed");
        }

        myself.send_interval(args.interval, || Message::Tick);

        Ok(state)
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
                        if let Err(e) = blue_green_deploy(state) {
                            error!(error = %e, "deploy failed");
                        }
                    }
                    Ok(false) => {
                        info!(repo = %state.repo_url, "no updates");
                    }
                    Err(e) => {
                        error!(error = %e, "git pull failed");
                    }
                }
            }
        }
        Ok(())
    }
}

fn deploy(state: &State, slot: Slot) -> anyhow::Result<()> {
    let project = state.project_name(slot);
    let files = [state.compose_file.as_str()];

    info!(project = %project, "deploying");
    docker::compose_up(&state.local_path, &files, &project, &state.network)?;

    Ok(())
}

fn blue_green_deploy(state: &mut State) -> anyhow::Result<()> {
    let new_slot = state.active_slot.other();
    let old_slot = state.active_slot;

    let new_project = state.project_name(new_slot);
    let old_project = state.project_name(old_slot);

    info!(old = %old_project, new = %new_project, "starting blue-green deploy");

    // 1. Deploy new slot
    deploy(state, new_slot)?;

    // 2. Wait for health
    info!(project = %new_project, "waiting for health check");
    if let Err(e) = docker::wait_healthy(&new_project, state.health_timeout) {
        error!(error = %e, "health check failed, rolling back");
        let files = [state.compose_file.as_str()];
        let _ = docker::compose_down(&state.local_path, &files, &new_project);
        return Err(e);
    }

    // 3. Stop old slot
    info!(project = %old_project, "stopping old slot");
    let files = [state.compose_file.as_str()];
    if let Err(e) = docker::compose_down(&state.local_path, &files, &old_project) {
        warn!(error = %e, "failed to stop old slot");
    }

    // 4. Update state
    state.active_slot = new_slot;
    state::save(
        &state.local_path,
        &state::PersistedState {
            active_slot: new_slot,
        },
    )?;

    info!(active = %new_project, "deploy complete");

    Ok(())
}
