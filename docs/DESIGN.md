# Rollploy Design

Pull-based deployment and cron runner.

## Commands

- `rollploy deploy` - Blue-green deployment for docker-compose apps
- `rollploy cron` - Auto-updating cron job runner

---

# Deploy

Zero-downtime blue-green deployment.

## Architecture

Each rollploy deploy instance is fully isolated:

```
┌─────────────────────────────────────────────────────────┐
│                        Server                           │
│                                                         │
│  rollploy deploy #1             rollploy deploy #2      │
│  ┌───────────────────┐         ┌───────────────────┐   │
│  │ traefik :3001     │         │ traefik :3002     │   │
│  │     ↓             │         │     ↓             │   │
│  │ app1-blue/green   │         │ app2-blue/green   │   │
│  │ network: app1     │         │ network: app2     │   │
│  └───────────────────┘         └───────────────────┘   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**1 deploy = 1 Traefik = 1 app = 1 port**

## CLI

```bash
rollploy deploy --repo https://github.com/user/app --port 3001
```

### Flags

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--repo` | yes | - | Git repository URL |
| `--port` | yes | - | Port to expose |
| `--branch` | no | main | Branch to track |
| `--compose` | no | docker-compose.yml | Compose file |
| `--interval` | no | 60 | Poll interval (sec) |
| `--health-timeout` | no | 120 | Health timeout (sec) |

## User's docker-compose.yml

```yaml
services:
  web:
    build: .
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 5s
      timeout: 3s
      retries: 3
```

---

# Cron

Auto-updating cron job runner. Runs scripts on host.

## CLI

```bash
rollploy cron --repo https://github.com/user/scripts
```

### Flags

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--repo` | yes | - | Git repository URL |
| `--branch` | no | main | Branch to track |
| `--interval` | no | 60 | Git pull interval (sec) |

## Config File

Create `rollploy.cron.yml` in repo root:

```yaml
jobs:
  - name: backup
    script: ./scripts/backup.sh
    schedule: "0 0 * * *"    # daily at midnight

  - name: cleanup
    script: ./scripts/cleanup.sh
    schedule: "0 */6 * * *"  # every 6 hours
```

## Behavior

- Scripts run on host (not in container)
- If a job is still running when next scheduled, it's skipped
- Config reloads automatically on git pull
- Output goes to stdout

---

# File Structure

```
src/
├── main.rs
├── actors/
│   └── deployer.rs    # Deploy actor
├── cron/
│   ├── config.rs      # Config parsing
│   └── runner.rs      # Cron actor
├── docker.rs
├── git.rs
├── state.rs
└── traefik.rs
```
