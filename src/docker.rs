use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn compose_up(cwd: &Path, compose_files: &[&str], project: &str, network: &str) -> Result<()> {
    let mut args = vec!["compose", "-p", project];
    for f in compose_files {
        args.push("-f");
        args.push(f);
    }
    args.extend(["up", "-d", "--pull", "always"]);

    let status = Command::new("docker")
        .args(&args)
        .current_dir(cwd)
        .status()
        .context("docker compose up failed")?;

    if !status.success() {
        bail!("docker compose up exited with {}", status);
    }

    // Connect to network
    connect_to_network(project, network)?;

    Ok(())
}

pub fn compose_down(cwd: &Path, compose_files: &[&str], project: &str) -> Result<()> {
    let mut args = vec!["compose", "-p", project];
    for f in compose_files {
        args.push("-f");
        args.push(f);
    }
    args.push("down");

    let status = Command::new("docker")
        .args(&args)
        .current_dir(cwd)
        .status()
        .context("docker compose down failed")?;

    if !status.success() {
        bail!("docker compose down exited with {}", status);
    }
    Ok(())
}

fn connect_to_network(project: &str, network: &str) -> Result<()> {
    // Get all containers in the project
    let output = Command::new("docker")
        .args(["compose", "-p", project, "ps", "-q"])
        .output()
        .context("docker compose ps failed")?;

    let container_ids = String::from_utf8_lossy(&output.stdout);
    for id in container_ids.lines() {
        if id.is_empty() {
            continue;
        }
        // Connect container to network (ignore if already connected)
        let _ = Command::new("docker")
            .args(["network", "connect", network, id])
            .status();
    }
    Ok(())
}

pub fn wait_healthy(project: &str, timeout: Duration) -> Result<()> {
    let start = Instant::now();

    loop {
        if start.elapsed() > timeout {
            bail!("health check timeout for project {}", project);
        }

        // Get all containers in project with health status
        let output = Command::new("docker")
            .args([
                "compose",
                "-p",
                project,
                "ps",
                "--format",
                "{{.Health}}",
            ])
            .output()
            .context("docker compose ps failed")?;

        let statuses = String::from_utf8_lossy(&output.stdout);
        let statuses: Vec<&str> = statuses.lines().filter(|s| !s.is_empty()).collect();

        if statuses.is_empty() {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }

        let all_healthy = statuses.iter().all(|s| *s == "healthy" || s.is_empty());
        let any_unhealthy = statuses.iter().any(|s| *s == "unhealthy");

        if any_unhealthy {
            bail!("container in project {} is unhealthy", project);
        }

        if all_healthy {
            return Ok(());
        }

        std::thread::sleep(Duration::from_secs(2));
    }
}

pub fn network_exists(name: &str) -> bool {
    Command::new("docker")
        .args(["network", "inspect", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn create_network(name: &str) -> Result<()> {
    if network_exists(name) {
        return Ok(());
    }

    let status = Command::new("docker")
        .args(["network", "create", name])
        .status()
        .context("docker network create failed")?;

    if !status.success() {
        bail!("docker network create exited with {}", status);
    }
    Ok(())
}
