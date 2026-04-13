# Harbor

Server orchestration for bare metal and cloud. Define your server, packages, services, and deploy steps in a single `harbor.yaml`, then run `harbor up` — Harbor creates the server, provisions it over SSH, wires up DNS, and deploys your code. For multi-server setups, a 4-line `fleet.yaml` composes role directories into named fleets that spin up concurrently. Written in Rust, single static binary, no agents on the server.

- **One-command lifecycle** — `harbor up` creates, provisions, and deploys. `harbor down` destroys the server and cleans DNS. No intermediate steps, no state to manage.
- **Declarative provisioning** — packages, Docker, Go, Rust, Caddy, systemd services, firewall rules, SSH hardening, file deployment, container services — all from YAML. Harbor generates and executes the setup script over SSH, no agent required.
- **Fleet orchestration** — `harbor fleet up production` creates an entire fleet from role directories, each with its own `harbor.yaml`. Concurrent by default, idempotent, deterministic server naming.

```bash
harbor up                        # create server, provision, deploy
harbor deploy                    # git pull, rebuild, restart
harbor fleet up staging          # spin up a named fleet
harbor down                      # tear it all down
```

## Install

```bash
cargo install --path crates/harbor-rs
```

## Quick start

**1. Set up credentials**

```bash
harbor init                      # scaffolds ~/.harbor/config.yaml
nano ~/.harbor/config.yaml       # add your Hetzner token
```

**2. Add a `harbor.yaml` to your project**

```yaml
name: myapp

server:
  name: myapp-prod
  type: cax11
  location: nbg1
  ssh_key: my-key
  hostname: myapp           # optional: creates myapp.i.example.com

setup:
  packages: [ca-certificates, curl, git]
  components:
    docker:
      enabled: true
  services:
    - name: myapp
      enabled: true
      exec_start: /usr/local/bin/myapp
      restart: always
  deploy:
    repo: github.com/you/myapp
    steps:
      - make build
      - sudo systemctl restart myapp
```

**3. Ship it**

```bash
harbor up       # server created, provisioned, deployed
harbor deploy   # subsequent deploys — pull, build, restart
harbor down     # done — server and DNS removed
```

## Fleet

For multi-server setups, organize each service as a directory with its own `harbor.yaml` and `dist/` config files:

```
cloud/
├── fleet.yaml
├── clickhouse/
│   ├── harbor.yaml
│   └── dist/
├── collectors/
│   ├── harbor.yaml
│   └── dist/
└── platform/
    ├── harbor.yaml
    └── dist/
```

The `fleet.yaml` composes roles and counts:

```yaml
roles:
  clickhouse: 1
  collectors: 3
  platform: 2
```

The fleet name is mandatory — it generates deterministic server names and identifies the fleet instance:

```bash
harbor fleet up staging          # creates clickhouse-staging-1, collectors-staging-{1,2,3}, platform-staging-{1,2}
harbor fleet status staging      # table: name, role, status, IP, type, location
harbor fleet down staging        # destroys all staging servers, cleans DNS
```

Same config, different fleets. `harbor fleet up production` creates a separate set of servers from the same role directories. Each role's `harbor.yaml` defines its server type, location, packages, services, and deploy steps — fleet just orchestrates.

## Configuration

Harbor uses two config files: **user config** for credentials (once per machine) and **project config** for what to build and deploy (per repo).

### User config (`~/.harbor/config.yaml`)

Created by `harbor init`. Stores API tokens — never committed to a repo.

```yaml
hetzner:
  token: "..."
cloudflare:           # optional — only needed for DNS
  api_token: "..."
  zone_id: "..."
dns:
  base_domain: ".i.example.com"
github:
  token: "..."        # optional — only needed for private repos
```

The Hetzner token resolves through a fallback chain: user config, then `HCLOUD_TOKEN` env var.

### Project config (`harbor.yaml`)

Lives in your repo root. Discovered automatically by `harbor up/down/deploy/status/ssh/logs` by walking up from the current directory. See the [quick start](#quick-start) example for the structure.

## Commands

| Command | What it does |
|---|---|
| `harbor up` | Create server, provision, deploy |
| `harbor down` | Destroy server, clean DNS |
| `harbor deploy` | Pull, rebuild, restart services |
| `harbor rollback [sha]` | Roll back to previous deploy or specific SHA |
| `harbor status` | Server state, last deploy, service health, disk |
| `harbor ssh` | Shell into the server |
| `harbor exec -- <cmd>` | Run a one-off command on the server |
| `harbor logs [service]` | Stream journald logs |
| `harbor fleet up <name>` | Create and provision a named fleet |
| `harbor fleet down <name>` | Destroy a named fleet |
| `harbor fleet status <name>` | Show fleet server status |

## License

MIT
