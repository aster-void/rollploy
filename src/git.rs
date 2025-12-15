use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn ensure_repo(local_path: &Path, url: &str, branch: &str) -> Result<()> {
    if local_path.exists() {
        return Ok(());
    }
    Command::new("git")
        .args(["clone", "--branch", branch, "--single-branch", url])
        .arg(local_path)
        .status()
        .context("git clone failed")?;
    Ok(())
}

/// Returns true if there were updates
pub fn pull(local_path: &Path) -> Result<bool> {
    let before = get_head(local_path)?;

    Command::new("git")
        .args(["pull", "--ff-only"])
        .current_dir(local_path)
        .status()
        .context("git pull failed")?;

    let after = get_head(local_path)?;
    Ok(before != after)
}

fn get_head(local_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(local_path)
        .output()
        .context("git rev-parse failed")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
