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

## Volumes

Keep data off the root disk with a Hetzner volume:

```yaml
server:
  volumes:
    - { name: myapp-data, size: 50, mount: /opt/myapp }   # format: ext4 (default) | xfs

setup:
  system:
    journald_max_use: 1G     # cap the journal on the root disk
```

`harbor up` creates the volume, or reattaches an existing one with that name, and mounts it by UUID with `nofail` before any directories, files or services are set up. A volume that already has a filesystem is never reformatted. `harbor down` detaches the volume and keeps it, and `harbor status` shows its usage. Volumes work with `harbor up` only, not fleets.

Firewall rules can be limited to one source: `- { port: 5432, proto: tcp, from: 203.0.113.7 }` (an IP or CIDR).

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
| `harbor deploy <name> --binary <path>` | Ship a binary built locally (Linux ELF for the server's arch); rollback works across both |
| `harbor rollback <name> [sha]` | Switch back to an earlier build |
| `harbor status` | Server state, last deploy, service health |
| `harbor ssh` / `exec -- <cmd>` | Shell in, or run one command |
| `harbor logs [service]` | Stream logs |
| `harbor backup` / `restore` | Back up to S3-compatible storage, or restore |
| `harbor fleet up/down/status <name>` | Manage a named fleet |

## License

MIT
