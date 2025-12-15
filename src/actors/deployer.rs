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
    pub service: String,
    pub domain: String,
    pub interval: Duration,
    pub health_timeout: Duration,
}

pub struct State {
    repo_url: String,
    local_path: PathBuf,
    compose_file: String,
    service: String,
    domain: String,
    active_slot: Slot,
    health_timeout: Duration,
    app_name: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    ForceRedeploy,
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
        info!(repo = %args.repo_url, "starting deployer");

        // Ensure traefik is running
        traefik::ensure_traefik()?;

        // Clone repo
        git::ensure_repo(&args.local_path, &args.repo_url, &args.branch)?;

        // Load persisted state
        let persisted = state::load(&args.local_path).unwrap_or_default();

        // Derive app name from repo
        let app_name = args
            .repo_url
            .split('/')
            .last()
            .unwrap_or("app")
            .trim_end_matches(".git")
            .to_string();

        let state = State {
            repo_url: args.repo_url,
            local_path: args.local_path,
            compose_file: args.compose_file,
            service: args.service,
            domain: args.domain,
            active_slot: persisted.active_slot,
            health_timeout: args.health_timeout,
            app_name,
        };

        // Initial deploy to active slot
        if let Err(e) = deploy_to_slot(&state, state.active_slot, true) {
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
                        info!(repo = %state.repo_url, "updates found, starting blue-green deploy");
                        if let Err(e) = blue_green_deploy(state) {
                            error!(error = %e, "blue-green deploy failed");
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
            Message::ForceRedeploy => {
                info!(repo = %state.repo_url, "force redeploying");
                if let Err(e) = blue_green_deploy(state) {
                    error!(error = %e, "force redeploy failed");
                }
            }
        }
        Ok(())
    }
}

fn deploy_to_slot(state: &State, slot: Slot, active: bool) -> anyhow::Result<()> {
    let project = state.project_name(slot);

    // Generate override file with traefik labels
    let override_file =
        traefik::generate_override(&state.local_path, &state.service, &state.domain, &project, active)?;

    let files = [state.compose_file.as_str(), override_file.as_str()];

    info!(project = %project, active = active, "deploying to slot");
    docker::compose_up(&state.local_path, &files, &project)?;

    Ok(())
}

fn blue_green_deploy(state: &mut State) -> anyhow::Result<()> {
    let new_slot = state.active_slot.other();
    let old_slot = state.active_slot;

    let new_project = state.project_name(new_slot);
    let old_project = state.project_name(old_slot);

    info!(
        old = %old_project,
        new = %new_project,
        "starting blue-green deployment"
    );

    // 1. Deploy to new slot (inactive)
    deploy_to_slot(state, new_slot, false)?;

    // 2. Wait for health check
    info!(project = %new_project, "waiting for health check");
    if let Err(e) = docker::wait_healthy(&new_project, &state.service, state.health_timeout) {
        error!(error = %e, "health check failed, keeping old slot active");
        // Clean up failed deployment
        let override_file = format!("docker-compose.{}.override.yml", new_project);
        let files = [state.compose_file.as_str(), override_file.as_str()];
        let _ = docker::compose_down(&state.local_path, &files, &new_project);
        return Err(e);
    }

    // 3. Switch routing: enable new, disable old
    info!("switching traffic to new slot");

    // Redeploy new slot with active labels
    deploy_to_slot(state, new_slot, true)?;

    // Redeploy old slot with inactive labels (keeps it as standby)
    if let Err(e) = deploy_to_slot(state, old_slot, false) {
        warn!(error = %e, "failed to disable old slot labels");
    }

    // 4. Update state
    state.active_slot = new_slot;
    state::save(
        &state.local_path,
        &state::PersistedState {
            active_slot: new_slot,
        },
    )?;

    info!(
        active = %state.project_name(new_slot),
        "blue-green deployment complete"
    );

    Ok(())
}
