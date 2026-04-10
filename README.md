# Harbor

Go from `harbor.yaml` to a running server in one command. Harbor creates Hetzner Cloud servers, provisions them over SSH, wires up Cloudflare DNS, and deploys your code — all from a single config file you keep in your repo.

```bash
harbor up      # create server, provision, deploy
harbor deploy  # git pull, rebuild, restart
harbor down    # tear it all down
```

## Features

- **Server orchestration** — `harbor up` creates a server, provisions it, and deploys your code. `harbor down` destroys the server and cleans up DNS. One command each way.
- **Zero-downtime deploys** — `harbor deploy` SSHes into the server, pulls your repo, runs your build steps, and restarts services. Idempotent clone-or-pull.
- **Provisioning from YAML** — packages, Docker, Go, Rust, Caddy, Fish shell, systemd services, firewall rules, SSH hardening, kernel hardening, swap, NTP, system users, directories, environment variables, file deployment — all declared in config.
- **DNS automation** — creates Cloudflare A records automatically when a hostname is set. Cleans up on teardown.
- **Fleet management** — `harbor env deploy` spins up multiple servers concurrently from a single config. `--sequential` flag when you need ordering.
- **Live output** — streams SSH provisioning output to your terminal in real time and logs to `/var/log/setup-<name>.log` on the server.
- **Operational shortcuts** — `harbor status`, `harbor ssh`, and `harbor logs [service]` for day-to-day server management without leaving the CLI.

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

The Hetzner token resolves through a fallback chain: deploy config, user config, then `HCLOUD_TOKEN` env var.

### Project config (`harbor.yaml`)

Lives in your repo root. Discovered automatically by `harbor up/down/deploy/status/ssh/logs` by walking up from the current directory.

See the [quick start](#quick-start) example above for the full structure. The `setup:` block supports:

| Section | Examples |
|---|---|
| `packages` | apt packages to install |
| `components` | docker, go, rust, caddy, fish, chrony-nts, fail2ban-rs, swap |
| `security` | ufw firewall rules, ssh hardening, kernel hardening |
| `services` | systemd units with restart policies |
| `deploy` | repo URL + build/restart steps |
| `github_repos` | clone, build, and install binaries from GitHub |
| `system_user` | create a dedicated service user |
| `directories` | create directories with ownership/permissions |
| `files` | deploy config files from repo to server |
| `environment` | environment variables |
| `path` | PATH modifications (prepend, append, overwrite) |
| `updates` | auto-upgrade, kernel updates, reboot policy |

## License

MIT
