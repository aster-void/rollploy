use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct CronConfig {
    pub jobs: Vec<Job>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Job {
    pub name: String,
    pub script: String,
    pub schedule: String,
}

const CONFIG_FILE: &str = "rollploy.cron.yml";

pub fn load(repo_path: &Path) -> Result<CronConfig> {
    let config_path = repo_path.join(CONFIG_FILE);
    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    serde_yaml::from_str(&content).context("failed to parse cron config")
}
