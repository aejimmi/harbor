# Features

## Server Management

- Create server — spin up a Hetzner Cloud server with specified type, location, image, and SSH key.
- Delete server — tear down a server by name and clean up its SSH known_hosts entry.
- List servers — display all running servers with name, type, location, status, and IP.
- Idempotent creation — if a server with the same name already exists and is running, reuses it instead of failing.
- SSH key validation — verifies the named SSH key exists in Hetzner before attempting server creation.
- Status polling — waits for newly created servers to reach running status before proceeding (5s intervals, up to 5 minutes).

## Environment Deploys

- Fleet deploy — create and provision multiple servers from a single YAML deploy config.
- Concurrent mode — deploys all servers in parallel by default using async task groups.
- Sequential mode — optional `--sequential` flag to deploy servers one at a time.
- Fleet destroy — tear down all servers defined in a deploy config.
- Deployment summary — table showing each server's name, IP, status (success/failed), and duration.

## SSH Provisioning

- Pure Rust SSH — connects to servers over SSH without shelling out to the `ssh` binary.
- SSH agent authentication — uses your local ssh-agent keys, tries each key until one succeeds.
- Connection retry — retries SSH connections up to 30 times with 10-second delays for freshly booted servers.
- Streamed output — pipes remote stdout/stderr to your terminal in real time, prefixed with `[server-name]`.
- Filtered output — shows command lines (`$`/`#` prefixed) by default, full verbose output with `--debug`.
- Server-side logging — setup output is tee'd to `/var/log/setup-<name>.log` on the remote server.
- Known hosts cleanup — automatically removes server IPs from `~/.ssh/known_hosts` on deletion.

## Setup Script Generation

- Declarative provisioning — generates bash setup scripts from YAML config instead of manual scripting.
- Package installation — apt-get install of arbitrary package lists.
- Docker installation — automated Docker CE setup from official repository.
- Go installation — installs a specified Go version from the official tarball.
- System user creation — creates system users with custom home, shell, and group.
- Directory creation — creates directories with specified owner, group, and permissions.
- Environment variables — exports variables to `/etc/environment`.
- PATH configuration — prepend, append, or overwrite system PATH entries.
- GitHub repo cloning — clones, builds, and installs Go binaries from GitHub repos with deploy key support.
- Systemd services — generates and enables systemd unit files from config.
- UFW firewall — enables UFW and opens specified ports.
- System updates — optional unattended upgrades, kernel upgrades, and automatic reboot.
- Hostname configuration — sets server hostname.
- Timezone configuration — sets system timezone.
- Dry run — `harbor generate` prints the generated script without executing it.

## DNS Integration

- Automatic A records — creates Cloudflare DNS records pointing to new server IPs.
- Upsert — updates existing DNS records if the hostname already exists, creates if not.
- Cleanup on delete — removes DNS records when servers are destroyed.
- Hostname extraction — derives DNS hostname from server name pattern (`service-hostname-env-location`).
- Configurable base domain — DNS records use a configurable base domain suffix.

## Configuration

- Init scaffolding — `harbor init` creates `~/.harbor/` with template configs for credentials, deploys, and server setup.
- Credential config — centralized Hetzner, Cloudflare, and GitHub tokens in `~/.harbor/config.yaml`.
- Token fallback chain — Hetzner token resolves from deploy config, then user config, then `HCLOUD_TOKEN` env var.
- Deploy configs — YAML files defining server fleets (type, location, image per server).
- Setup configs — YAML files defining the full provisioning recipe.
- Config path override — `--config` flag to use a custom config file path.
- Secure defaults — credential files created with 0600 permissions.

## CLI

- Shell completions — generates completions for bash, zsh, and fish.
- Colored output — styled terminal output with color-coded success, error, info, and header messages.
- Quiet mode — `--quiet` flag suppresses non-essential output.
- Debug mode — `--debug` flag shows verbose SSH output and internal diagnostics.
- Graceful interrupt — Ctrl+C triggers clean shutdown instead of abrupt termination.
