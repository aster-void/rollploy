# Rollploy Design

Pull-based rolling-release deployment system using actor model (ractor).

## Architecture

**1 instance = 1 repository** (run multiple instances for multiple repos)

```
┌─────────────┐
│  Deployer   │  ← single actor with internal timer
└─────────────┘
```

## Actor: Deployer

- Single actor per process
- Internal timer for periodic checks (ractor's `send_interval`)
- State: `{ repo_url, branch, local_path, compose_file }`
- Messages:
  - `Tick` → git pull, if changed → deploy
  - `ForceRedeploy` → deploy without checking for updates

## File Structure

```
src/
├── main.rs              # CLI parsing, actor bootstrap
├── actors/
│   ├── mod.rs
│   └── deployer.rs
├── git.rs               # git clone/pull
└── docker.rs            # docker compose up
```

## Message Flow

```
1. main() parses CLI, spawns Deployer actor
2. Deployer clones repo (if not exists) and runs initial deploy
3. Deployer sets up send_interval for periodic Tick
4. On Tick → git pull → if updates → docker compose up
```

## CLI

```bash
rollploy --repo https://github.com/user/app \
         --branch main \
         --compose docker-compose.yml \
         --interval 60 \
         --dir /path/to/local/clone  # optional
```

## Dependencies

- `ractor` - Actor framework (Erlang-style)
- `tokio` - Async runtime
- `clap` - CLI parsing
- `tracing` - Logging
