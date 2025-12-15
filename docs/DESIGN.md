# Rollploy Design

Pull-based rolling-release deployment with **zero-downtime blue-green strategy**.

## Architecture

```
                    ┌──────────────┐
        :80/:443 ──►│   Traefik    │  (managed by rollploy)
                    └──────┬───────┘
                           │ Docker provider
              ┌────────────┴────────────┐
              │                         │
       ┌──────▼──────┐          ┌───────▼─────┐
       │  app-blue   │          │  app-green  │
       │  (active)   │          │  (standby)  │
       └─────────────┘          └─────────────┘
```

**1 instance = 1 repository** (run multiple instances for multiple repos)

## Blue-Green Deployment Flow

1. Determine inactive slot (if blue active → deploy to green)
2. `docker compose -p {app}-{slot} up -d` with inactive labels
3. Wait for Docker healthcheck to pass
4. Redeploy with active Traefik labels (switches routing)
5. Old slot stays running (instant rollback possible)

## File Structure

```
src/
├── main.rs              # CLI parsing, actor bootstrap
├── actors/
│   ├── mod.rs
│   └── deployer.rs      # Blue-green deployment logic
├── docker.rs            # docker compose operations
├── git.rs               # git clone/pull
├── state.rs             # Slot enum, state persistence
└── traefik.rs           # Traefik management
```

## CLI

```bash
rollploy --repo https://github.com/user/app \
         --branch main \
         --compose docker-compose.yml \
         --service web \
         --domain app.example.com \
         --interval 60 \
         --health-timeout 120
```

### Required Flags

| Flag | Description |
|------|-------------|
| `--repo` | Git repository URL |
| `--service` | Main service name in docker-compose.yml |
| `--domain` | Domain for Traefik routing |

### Optional Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--branch` | main | Branch to track |
| `--compose` | docker-compose.yml | Compose file path |
| `--interval` | 60 | Poll interval (seconds) |
| `--health-timeout` | 120 | Health check timeout (seconds) |
| `--dir` | auto | Local clone directory |

## User Requirements

### docker-compose.yml

Must define healthcheck and connect to rollploy network:

```yaml
services:
  web:
    image: myapp:latest
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 5s
      timeout: 3s
      retries: 3
    networks:
      - rollploy

networks:
  rollploy:
    external: true
```

## State Persistence

Active slot is persisted to `{repo}/.rollploy-state.json`:

```json
{
  "active_slot": "blue"
}
```

## Dependencies

- `ractor` - Actor framework (Erlang-style)
- `tokio` - Async runtime
- `clap` - CLI parsing
- `serde` - State serialization
- `tracing` - Logging
