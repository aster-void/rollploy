use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn start(app_name: &str, port: u16, network: &str) -> Result<()> {
    let container_name = format!("rollploy-{}-traefik", app_name);

    // Check if already running (idempotent)
    let output = Command::new("docker")
        .args(["ps", "-q", "-f", &format!("name={}", container_name)])
        .output()
        .context("docker ps failed")?;

    if !output.stdout.is_empty() {
        return Ok(());
    }

    let status = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            &container_name,
            "--network",
            network,
            "-p",
            &format!("{}:80", port),
            "-v",
            "/var/run/docker.sock:/var/run/docker.sock:ro",
            "--restart",
            "unless-stopped",
            "traefik:v3.0",
            "--providers.docker=true",
            "--providers.docker.exposedbydefault=false",
            &format!("--providers.docker.network={}", network),
            "--entrypoints.web.address=:80",
        ])
        .status()
        .context("failed to start traefik")?;

    if !status.success() {
        bail!("failed to start traefik container");
    }

    Ok(())
}
