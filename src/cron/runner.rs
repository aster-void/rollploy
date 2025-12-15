use super::config::{self, Job};
use crate::git;
use chrono::Utc;
use cron::Schedule;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{error, info, warn};

pub struct CronRunner;

pub struct CronRunnerArgs {
    pub repo_url: String,
    pub branch: String,
    pub local_path: PathBuf,
    pub check_interval: Duration,
}

pub struct State {
    repo_url: String,
    local_path: PathBuf,
    jobs: Vec<JobState>,
    running: Arc<Mutex<HashMap<String, bool>>>,
}

struct JobState {
    job: Job,
    schedule: Schedule,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    GitPull,
}

#[async_trait]
impl Actor for CronRunner {
    type Msg = Message;
    type State = State;
    type Arguments = CronRunnerArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!(repo = %args.repo_url, "starting cron runner");

        // Clone repo
        git::ensure_repo(&args.local_path, &args.repo_url, &args.branch)?;

        // Load config
        let config = config::load(&args.local_path)?;
        let jobs = parse_jobs(config.jobs)?;

        info!(job_count = jobs.len(), "loaded cron jobs");

        let state = State {
            repo_url: args.repo_url,
            local_path: args.local_path,
            jobs,
            running: Arc::new(Mutex::new(HashMap::new())),
        };

        // Check every second for due jobs
        myself.send_interval(Duration::from_secs(1), || Message::Tick);

        // Pull git periodically for updates
        myself.send_interval(args.check_interval, || Message::GitPull);

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
                let now = Utc::now();
                for job_state in &state.jobs {
                    if should_run(&job_state.schedule, now) {
                        let job_name = job_state.job.name.clone();

                        // Check if already running
                        {
                            let running = state.running.lock().unwrap();
                            if *running.get(&job_name).unwrap_or(&false) {
                                warn!(job = %job_name, "skipping, already running");
                                continue;
                            }
                        }

                        // Mark as running
                        {
                            let mut running = state.running.lock().unwrap();
                            running.insert(job_name.clone(), true);
                        }

                        // Run in background
                        let script = state.local_path.join(&job_state.job.script);
                        let cwd = state.local_path.clone();
                        let running = Arc::clone(&state.running);

                        std::thread::spawn(move || {
                            info!(job = %job_name, "running");

                            let result = Command::new(&script)
                                .current_dir(&cwd)
                                .stdout(Stdio::inherit())
                                .stderr(Stdio::inherit())
                                .status();

                            match result {
                                Ok(status) => {
                                    if status.success() {
                                        info!(job = %job_name, "completed successfully");
                                    } else {
                                        error!(job = %job_name, code = ?status.code(), "failed");
                                    }
                                }
                                Err(e) => {
                                    error!(job = %job_name, error = %e, "failed to execute");
                                }
                            }

                            // Mark as not running
                            let mut running = running.lock().unwrap();
                            running.insert(job_name, false);
                        });
                    }
                }
            }
            Message::GitPull => {
                info!(repo = %state.repo_url, "checking for updates");
                match git::pull(&state.local_path) {
                    Ok(true) => {
                        info!("updates found, reloading config");
                        match config::load(&state.local_path) {
                            Ok(config) => match parse_jobs(config.jobs) {
                                Ok(jobs) => {
                                    state.jobs = jobs;
                                    info!(job_count = state.jobs.len(), "reloaded cron jobs");
                                }
                                Err(e) => error!(error = %e, "failed to parse jobs"),
                            },
                            Err(e) => error!(error = %e, "failed to load config"),
                        }
                    }
                    Ok(false) => {
                        info!("no updates");
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

fn parse_jobs(jobs: Vec<Job>) -> anyhow::Result<Vec<JobState>> {
    let mut result = Vec::new();
    for job in jobs {
        let schedule = Schedule::from_str(&job.schedule)
            .map_err(|e| anyhow::anyhow!("invalid cron expression for {}: {}", job.name, e))?;
        result.push(JobState { job, schedule });
    }
    Ok(result)
}

fn should_run(schedule: &Schedule, now: chrono::DateTime<Utc>) -> bool {
    // Check if current second matches a scheduled time
    if let Some(next) = schedule.upcoming(Utc).next() {
        let diff = (next - now).num_seconds();
        diff == 0
    } else {
        false
    }
}
