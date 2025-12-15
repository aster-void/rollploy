# Rollploy Design

Pull-based rolling deployment with **zero-downtime blue-green strategy**.

## Architecture

Each rollploy instance is fully isolated:

```
┌─────────────────────────────────────────────────────────┐
│                        Server                           │
│                                                         │
│  rollploy #1                    rollploy #2             │
│  ┌───────────────────┐         ┌───────────────────┐   │
│  │ traefik :3001     │         │ traefik :3002     │   │
│  │     ↓             │         │     ↓             │   │
│  │ app1-blue/green   │         │ app2-blue/green   │   │
│  │ network: app1     │         │ network: app2     │   │
│  └───────────────────┘         └───────────────────┘   │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**1 rollploy = 1 Traefik = 1 app = 1 port**

No shared resources. Complete isolation.

## Blue-Green Deployment Flow

```
State: app-blue running

1. git pull → updates found
2. docker compose -p app-green up
3. wait for healthcheck
4. docker compose -p app-blue down

State: app-green running (ZDT achieved)
```

During steps 2-4, both are running → Traefik routes to both → no downtime.

## CLI

```bash
rollploy --repo https://github.com/user/app --port 3001
```

### Required Flags

| Flag | Description |
|------|-------------|
| `--repo` | Git repository URL |
| `--port` | Port to expose the app on |

### Optional Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--branch` | main | Branch to track |
| `--compose` | docker-compose.yml | Compose file path |
| `--interval` | 60 | Poll interval (seconds) |
| `--health-timeout` | 120 | Health check timeout (seconds) |
| `--dir` | auto | Local clone directory |

## User's docker-compose.yml

Just a normal compose file with healthcheck:

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

No special labels needed. Rollploy handles everything.

## File Structure

```
src/
├── main.rs           # CLI
├── actors/
│   └── deployer.rs   # Blue-green logic
├── docker.rs         # Docker compose operations
├── git.rs            # Git operations
├── state.rs          # Slot persistence
└── traefik.rs        # Traefik management
```

## State Persistence

Active slot persisted to `{repo}/.rollploy-state.json`:

```json
{
  "active_slot": "blue"
}
```

## Multiple Apps

Run multiple instances on different ports:

```bash
rollploy --repo .../app1 --port 3001 &
rollploy --repo .../app2 --port 3002 &
rollploy --repo .../app3 --port 3003 &
```

Optional: Add nginx/Traefik in front for SSL and domain routing:

```
:443 → nginx (user-managed)
         ├──► localhost:3001 (app1)
         ├──► localhost:3002 (app2)
         └──► localhost:3003 (app3)
```
