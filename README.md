# Harbor

Server orchestration for bare metal and cloud. Describe your server, packages, services, and deploys in one `harbor.yaml`, then `harbor up` creates the server, provisions it over SSH, sets up DNS, and deploys your code. Single binary, nothing installed on the server.

## Install

```bash
cargo install --git https://github.com/aejimmi/harbor
```

## Quick start

```bash
harbor init                      # creates ~/.harbor/config.yaml — add your Hetzner token
```

Add a `harbor.yaml` to your repo:

```yaml
name: myapp

server:
  name: myapp-prod
  type: cax11
  location: nbg1
  ssh_key: my-key            # SSH key name in Hetzner

setup:
  packages: [build-essential, git]
  components:
    rust: { enabled: true }
  services:
    - { name: myapp, enabled: true, start: true, exec_start: /usr/local/bin/myapp }
  deploys:
    app:
      repo: github.com/you/myapp
      steps: [cargo build --release]
      binary: target/release/myapp
      install: /usr/local/bin/myapp
      services: [myapp]
```

```bash
harbor up                        # create, provision, deploy
harbor deploy app                # pull, build, restart
harbor rollback app              # back to the previous build, no rebuild
harbor down                      # destroy server and DNS record
```

More configs — container services, backups, fleets — are in [`examples/`](examples/).

## Fleet

Give each role a directory with its own `harbor.yaml`, and list them in `fleet.yaml`:

```yaml
roles:
  web: 2
  worker: 1
```

```bash
harbor fleet up staging          # web-staging-1, web-staging-2, worker-staging-1
harbor fleet status staging
harbor fleet down staging
```

## Commands

| Command | What it does |
|---|---|
| `harbor up` / `down` | Create and provision, or destroy |
| `harbor deploy <name>` | Pull, build, restart (`--all` for every deploy) |
| `harbor rollback <name> [sha]` | Switch back to an earlier build |
| `harbor status` | Server state, last deploy, service health |
| `harbor ssh` / `exec -- <cmd>` | Shell in, or run one command |
| `harbor logs [service]` | Stream logs |
| `harbor backup` / `restore` | Back up to S3-compatible storage, or restore |
| `harbor fleet up/down/status <name>` | Manage a named fleet |

## License

MIT
