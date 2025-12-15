use crate::docker;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::Command;

const NETWORK_NAME: &str = "rollploy";

const TRAEFIK_COMPOSE: &str = r#"services:
  traefik:
    image: traefik:v3.0
    container_name: rollploy-traefik
    command:
      - --providers.docker=true
      - --providers.docker.exposedbydefault=false
      - --providers.docker.network=rollploy
      - --entrypoints.web.address=:80
    ports:
      - "80:80"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    networks:
      - rollploy
    restart: unless-stopped
networks:
  rollploy:
    external: true
"#;

pub fn ensure_network() -> Result<()> {
    if !docker::network_exists(NETWORK_NAME) {
        docker::create_network(NETWORK_NAME)?;
    }
    Ok(())
}

pub fn ensure_traefik() -> Result<()> {
    ensure_network()?;

    // Check if traefik is already running
    let output = Command::new("docker")
        .args(["ps", "-q", "-f", "name=rollploy-traefik"])
        .output()
        .context("docker ps failed")?;

    if !output.stdout.is_empty() {
        return Ok(()); // Already running
    }

    // Create temp compose file and start traefik
    let tmp_dir = std::env::temp_dir().join("rollploy-traefik");
    std::fs::create_dir_all(&tmp_dir)?;

    let compose_path = tmp_dir.join("docker-compose.yml");
    std::fs::write(&compose_path, TRAEFIK_COMPOSE)?;

    let status = Command::new("docker")
        .args([
            "compose",
            "-p",
            "rollploy-traefik",
            "-f",
            compose_path.to_str().unwrap(),
            "up",
            "-d",
        ])
        .status()
        .context("failed to start traefik")?;

    if !status.success() {
        anyhow::bail!("failed to start traefik");
    }

    Ok(())
}

pub fn generate_override(
    dir: &Path,
    service: &str,
    domain: &str,
    project: &str,
    active: bool,
) -> Result<String> {
    let override_file = format!("docker-compose.{}.override.yml", project);
    let override_path = dir.join(&override_file);

    let content = if active {
        format!(
            r#"services:
  {}:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.{}.rule=Host(`{}`)"
      - "traefik.http.routers.{}.entrypoints=web"
    networks:
      - rollploy
networks:
  rollploy:
    external: true
"#,
            service, project, domain, project
        )
    } else {
        format!(
            r#"services:
  {}:
    labels:
      - "traefik.enable=false"
    networks:
      - rollploy
networks:
  rollploy:
    external: true
"#,
            service
        )
    };

    let mut file = std::fs::File::create(&override_path)?;
    file.write_all(content.as_bytes())?;

    Ok(override_file)
}
