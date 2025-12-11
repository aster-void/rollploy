use anyhow::Result;
use std::{fs, process};

fn main() {
    run("");
}

const BASE_DIR: &str = "~/.local/state/rollploy/repositories";

fn run(repository: &str) -> anyhow::Result<()> {
    let repo_dir = BASE_DIR.to_owned() + repository;
    fs::create_dir_all(repo_dir)?;
    Ok(())
}

// docker controls
fn docker_compose_up(cwd: &str, configFile: &str) -> Result<()> {
    process::Command::new("docker")
        .args(["compose", "up", "--detach", "--file", configFile])
        .output()?;

    Ok(())
}

// git controls

fn git_ensure_latest(target_dir: &str, url: &str) -> Result<()> {
    git_clone(target_dir, url)?;
    Ok(())
}

// returns (has_updates)
fn git_pull(cwd: &str, url: &str, branch: &str) -> Result<bool> {
    process::Command::new("git")
        .args(["pull", "origin", branch])
        .output()?;

    let has_updates = true; // for now it assumes it has updates, but in the future it will infer if it has updates or not
    Ok(has_updates)
}

fn git_clone(cwd: &str, url: &str) -> Result<()> {
    process::Command::new("git")
        .args(["clone", url, cwd])
        .output()?;

    Ok(())
}
