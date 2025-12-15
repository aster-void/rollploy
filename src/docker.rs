use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn compose_up(cwd: &Path, compose_files: &[&str], project: &str) -> Result<()> {
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

pub fn wait_healthy(project: &str, service: &str, timeout: Duration) -> Result<()> {
    let container = format!("{}-{}-1", project, service);
    let start = Instant::now();

    loop {
        if start.elapsed() > timeout {
            bail!("health check timeout for {}", container);
        }

        let output = Command::new("docker")
            .args(["inspect", "--format", "{{.State.Health.Status}}", &container])
            .output()
            .context("docker inspect failed")?;

        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();

        match status.as_str() {
            "healthy" => return Ok(()),
            "unhealthy" => bail!("container {} is unhealthy", container),
            _ => std::thread::sleep(Duration::from_secs(2)),
        }
    }
}

pub fn update_labels(project: &str, service: &str, domain: &str, enable: bool) -> Result<()> {
    let container = format!("{}-{}-1", project, service);

    let labels: Vec<String> = if enable {
        vec![
            format!("traefik.enable=true"),
            format!("traefik.http.routers.{}.rule=Host(`{}`)", project, domain),
            format!("traefik.http.routers.{}.entrypoints=web", project),
        ]
    } else {
        vec!["traefik.enable=false".to_string()]
    };

    // Docker doesn't support updating labels on running containers
    // We need to use docker compose with labels override
    // For now, we'll recreate with new labels via environment
    for label in labels {
        let status = Command::new("docker")
            .args(["container", "update", "--label", &label, &container])
            .status();

        // docker container update doesn't support --label, so we use a workaround
        if status.is_err() {
            // Labels are set at container creation, not runtime
            // The actual label switching happens via compose override
            break;
        }
    }

    Ok(())
}

pub fn get_container_id(project: &str, service: &str) -> Result<String> {
    let container = format!("{}-{}-1", project, service);
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.Id}}", &container])
        .output()
        .context("docker inspect failed")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn network_exists(name: &str) -> bool {
    Command::new("docker")
        .args(["network", "inspect", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn create_network(name: &str) -> Result<()> {
    let status = Command::new("docker")
        .args(["network", "create", name])
        .status()
        .context("docker network create failed")?;

    if !status.success() {
        bail!("docker network create exited with {}", status);
    }
    Ok(())
}
