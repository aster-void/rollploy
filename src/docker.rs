use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn compose_up(cwd: &Path, compose_file: &str) -> Result<()> {
    Command::new("docker")
        .args(["compose", "-f", compose_file, "up", "-d", "--pull", "always"])
        .current_dir(cwd)
        .status()
        .context("docker compose up failed")?;
    Ok(())
}
