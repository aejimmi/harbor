# Harbor

Server orchestration for Hetzner Cloud. Harbor creates servers, provisions them over SSH, and wires up Cloudflare DNS records — all from declarative YAML configs. You describe the fleet, harbor builds it. Written in Rust, compiled to a single static binary that replaces ~3,800 lines of Go.

- **Concurrent** — deploys multiple servers in parallel via async task groups, with a `--sequential` flag when you need ordering
- **Declarative** — YAML configs define server specs, setup recipes (packages, Docker, Go, firewall, systemd services), and DNS settings
- **Hands-off provisioning** — connects via SSH using your agent keys, streams setup output to your terminal, and logs everything to `/var/log/setup-<name>.log` on the server

## Quick start

```bash
cargo install --path crates/harbor-rs
harbor init
```

This scaffolds `~/.harbor/` with template configs:

```
~/.harbor/
├── config.yaml                  # Credentials (Hetzner, Cloudflare, GitHub)
├── configs-deploy/
│   ├── production.yaml          # Server fleet definitions
│   ├── staging.yaml
│   └── development.yaml
└── configs-server/
    └── server-profile.yaml      # Provisioning recipe
```

Edit `config.yaml` with your API tokens, then deploy:

```bash
harbor env deploy configs-deploy/staging.yaml
```

## Commands

| Command | Description |
|---|---|
| `harbor init` | Scaffold `~/.harbor/` with template configs |
| `harbor server create <name> --ssh-key <key>` | Create and provision a single server |
| `harbor server delete <name>` | Delete a server and clean up DNS |
| `harbor server list` | List running servers |
| `harbor env deploy <config>` | Deploy a fleet from a deploy config |
| `harbor env destroy <config>` | Tear down all servers in a deploy config |
| `harbor generate <setup-config>` | Print the generated setup script without running it |
| `harbor config list` | Show available deploy configurations |
| `harbor completion <shell>` | Generate shell completions (bash, zsh, fish) |

## Configuration

### Credentials (`~/.harbor/config.yaml`)

```yaml
hetzner:
  token: "your_hetzner_cloud_token"

cloudflare:
  api_token: "your_cloudflare_api_token"
  zone_id: "your_cloudflare_zone_id"

dns:
  base_domain: ".example.com"
  provider: "cloudflare"

github:
  token: "your_github_token"  # For private repo access
```

The Hetzner token resolves through a fallback chain: deploy config → user config → `HCLOUD_TOKEN` env var.

### Deploy config (`configs-deploy/*.yaml`)

Defines which servers to create:

```yaml
hcloud:
  ssh_key: "my-key"

servers:
  - name: "app-01"
    type: "cax11"
    location: "nbg1"
    image: "ubuntu-24.04"
  - name: "app-02"
    type: "cax11"
    location: "fsn1"
    image: "ubuntu-24.04"
```

### Setup config (`configs-server/*.yaml`)

Defines what gets installed on each server:

```yaml
setup:
  packages: [ca-certificates, curl, git, jq]

  components:
    docker:
      enabled: true
    go:
      enabled: true
      version: "1.24.5"

  security:
    ufw:
      enabled: true
      allow_ports: [22, 50000]

  services:
    - name: "myapp"
      enabled: true
      user: "myapp"
      exec_start: "/usr/local/bin/myapp"
      restart: "always"
```

The setup config also supports system users, directories, environment variables, PATH modifications, DNS settings, GitHub repo cloning, hostname configuration, and kernel updates.

## How it works

```
YAML configs → validated config structs
                     │
    ┌────────────────┼────────────────┐
    ▼                ▼                ▼
CloudProvider   ScriptBuilder    DnsProvider
(create server) (generate bash)  (create A record)
    │                │
    ▼                ▼
 Provisioner ◄── setup script
(SSH + execute)
    │
    ▼
Deployment Summary
```

Each server goes through: create via Hetzner API → generate setup script from YAML → create DNS record via Cloudflare → connect over SSH → execute script with streamed output. In concurrent mode, all servers run this pipeline simultaneously.

## License

MIT
