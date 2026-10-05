# Features

## Server Management

- Create server — spin up a Hetzner Cloud server with specified type, location, image, and SSH key.
- Delete server — tear down a server by name and clean up its SSH known_hosts entry.
- List servers — display all running servers with name, type, location, status, and IP.
- Idempotent creation — if a server with the same name already exists and is running, reuses it instead of failing.
- SSH key validation — verifies the named SSH key exists in Hetzner before attempting server creation.
- Status polling — waits for newly created servers to reach running status before proceeding (5s intervals, up to 5 minutes).
- Block volumes — `server.volumes:` creates a Hetzner volume attached to the server, or reattaches an existing one with the same name; `harbor down` detaches and keeps it.
- Volume preflight — a volume in another location or attached to another server is refused before any server is created.
- Volume mounts — formatted only when blank (ext4 default, xfs optional), mounted by UUID with `nofail` before directories, files, and services are set up.

## Fleet Management

- Fleet up — create and provision multiple servers from a `fleet.yaml` that composes role directories, each with its own `harbor.yaml` and `dist/` config files.
- Fleet naming — mandatory fleet name (e.g. staging, production) generates deterministic server names: `{role}-{name}-{N}`.
- Fleet down — tear down all servers in a named fleet, clean up DNS records and SSH known hosts.
- Fleet status — query Hetzner for each fleet server and display a table with name, role, status, IP, type, and location.
- Role shorthand — `collectors: 3` in fleet.yaml or long form `{ count: 3, path: ./custom-dir }` when directory name differs from role.
- Concurrent mode — fleet up creates all servers in parallel by default using async task groups.
- Sequential mode — optional `--sequential` flag to create servers one at a time.
- Idempotent — fleet up skips existing servers, fleet down skips missing ones.
- Fail-fast validation — all role directories and harbor.yaml files are checked before any servers are created.
- Deployment summary — table showing each server's name, IP, status (success/failed), and duration.

## Application Deployment

- Deploy command — `harbor deploy` pulls the latest code, rebuilds, and restarts services on the configured server.
- Rollback to previous — `harbor rollback` rewinds the running app to the prior deploy recorded in history.
- Rollback to SHA — `harbor rollback <sha>` checks out a specific git SHA and re-runs the deploy steps.
- Deploy lock — concurrent deploys are blocked by `~/.harbor/deploy.lock`; stale locks older than 30 minutes are auto-cleared.
- Deploy history — every deploy and rollback is recorded to `~/.harbor/deploys.log` with timestamp, user, and SHA.
- Health checks — after deploy and rollback, each started service is verified with `systemctl is-active`; failures dump recent journal logs and abort.
- Exec command — `harbor exec -- <cmd>` runs a one-off command on the server over SSH.
- Debug mode — `--debug` on `up`, `deploy`, and `rollback` streams raw remote output instead of the spinner.
- Deploy config validation — deploy names restricted to `[A-Za-z0-9_-]`, `binary:` must be repo-relative with no `..`, `install:` must be absolute, all checked at config load.

## Backups

- Declarative backup — `backup:` block in `harbor.yaml` archives configured paths on a timer, uploads to S3-compatible storage, and prunes old archives.
- Schedule — `hourly`, `daily`, or `weekly` systemd timer with a randomized 1-hour delay.
- S3-compatible destinations — `s3://<bucket>[/<prefix>]` URIs over HTTPS endpoints (e.g. Cloudflare R2).
- Transport choice — `rc` (default, pure-Rust, pinned version) or `rclone` (upstream install script), selected per project.
- On-demand backup — `harbor backup` triggers the oneshot backup service immediately and reports its result.
- Backup listing — `harbor backup list` shows archives in the remote bucket, newest first, filtered to the project's strict key pattern.
- Retention — archives older than `retention_days` (default 14) are pruned after each successful upload.
- Service-aware archiving — `stop_services` are stopped before archiving and restarted in reverse order afterward; a failed stop triggers best-effort restart rollback.
- Restore — `harbor restore [--at <timestamp>]` replaces live data from a backup; newest archive is used when `--at` is omitted.
- Restore pre-flight — archive, services, and paths are printed before prompting; `tar tzf` verifies archive integrity before any live path is touched.
- Atomic swap on restore — each live path is moved aside as `.old-<timestamp>` and replaced with restored content, with best-effort rollback on extract failure.
- Confirmation prompt — restore asks for explicit confirmation; `--yes` skips the prompt for scripted flows.
- Credential isolation — S3 keys written to `/etc/harbor/backup.env` with 0600 root-owned permissions; secret access keys redacted from debug output and panic messages.
- Backup config validation — destination shape, https-only endpoint, absolute paths without `..`, retention ≥ 1, and `stop_services` cross-referenced to declared services, all checked at config load.

## SSH Provisioning

- Pure Rust SSH — connects to servers over SSH without shelling out to the `ssh` binary.
- SSH agent authentication — uses your local ssh-agent keys, tries each key until one succeeds.
- Connection retry — retries SSH connections up to 30 times with 10-second delays for freshly booted servers.
- SSH keepalive — sends keepalive every 30 seconds with 10 allowed misses, preventing timeouts during long silent builds.
- Ticking spinner — live progress indicator with elapsed time that advances on harbor's own status lines and ignores unrelated apt or dpkg chatter.
- Spinner line truncation — long status lines are trimmed to terminal width so progress never wraps onto a new line.
- Server-side logging — setup output is tee'd to `/var/log/setup-<name>.log` on the remote server.
- Pinned host keys — server host keys are saved to `~/.harbor/known_hosts` on first connect; later connections reject a mismatched key (possible MITM or re-created server).
- Known hosts cleanup — automatically removes server IPs from `~/.ssh/known_hosts` and `~/.harbor/known_hosts` on deletion.
- Accept new host keys — `ssh`, `exec`, `logs`, `status`, `backup`, and `restore` use `StrictHostKeyChecking=accept-new` to auto-trust fresh servers on first connection.

## Setup Script Generation

- Declarative provisioning — generates bash setup scripts from YAML config instead of manual scripting.
- Non-interactive apt — setup scripts run apt with `DEBIAN_FRONTEND=noninteractive` and `--force-confold`, so dpkg never prompts and existing config files are preserved.
- Package installation — apt-get install of arbitrary package lists.
- Docker installation — automated Docker CE setup from official repository.
- Go installation — installs a specified Go version from the official tarball.
- Rust installation — installs the Rust toolchain via rustup.
- Caddy web server — installs Caddy from the official repository.
- Chrony NTS — installs Chrony with Network Time Security for accurate, authenticated time sync.
- fail2ban-rs — installs fail2ban-rs for intrusion prevention.
- Swap file — creates a swap file of configurable size.
- Fish shell — installs Fish shell.
- System user creation — idempotent creation with custom home, shell, and group; fails loud if the user is missing afterwards.
- Directory creation — creates directories with specified owner, group, and permissions.
- Environment variables — exports variables to `/etc/environment`.
- PATH configuration — prepend, append, or overwrite system PATH entries.
- File deployment — copies local config files to the server with specified owner, group, and permissions.
- Systemd services — generates, enables, and restarts systemd units so config changes apply on redeploy.
- Container services via Docker — run any OCI image as a managed service by setting image on a service spec; Harbor handles pull, run, and systemd lifecycle.
- Container services via Podman — opt-in daemonless runtime selected with runtime: podman, rendered as Quadlet .container files under /etc/containers/systemd/.
- Container env files — per-service env vars written to /etc/harbor/env/<name>.env with 0600 perms instead of inline in world-readable unit files.
- Container runtime auto-install — Docker or Podman installed automatically based on which runtimes the config references; nothing installed if no service declares an image.
- Container config validation — services mixing image and exec_start, or declaring empty image, are rejected at config load with clear errors.
- Container capability controls — per-service `cap_drop` and `cap_add` shape the Linux capabilities allowed inside the container.
- Container read-only root — opt-in `read_only: true` renders a container with a read-only rootfs and an automatic 64 MB writable `/tmp` tmpfs.
- Container PID limit — per-service `pids_limit` caps the number of processes inside the container (default 256, `0` disables).
- Service env secret redaction — service environment values are redacted from debug output and panic messages so secrets never reach trace logs.
- UFW firewall — enables UFW and opens specified ports, with optional rate limiting per rule.
- SSH hardening — disables password auth, root login, and enforces key-only access.
- Kernel hardening — applies sysctl security settings for network and memory protection.
- Mount hardening — `/tmp`, `/var/tmp`, and `/dev/shm` are mounted with `noexec,nosuid,nodev` to block binary execution from world-writable directories.
- System updates — optional unattended upgrades, kernel upgrades, and automatic reboot.
- Hostname configuration — sets server hostname.
- Timezone configuration — sets system timezone.
- Journal cap — `system.journald_max_use` sets journald `SystemMaxUse` so logs can't fill the root disk.
- Dry run — `harbor generate` prints the generated script without executing it.

## DNS Integration

- Automatic A records — creates Cloudflare DNS records pointing to new server IPs.
- Upsert — updates existing DNS records if the hostname already exists, creates if not.
- Cleanup on delete — removes DNS records when servers are destroyed.
- Hostname extraction — derives DNS hostname from server name pattern (`service-hostname-env-location`).
- Configurable base domain — DNS records use a configurable base domain suffix.

## Configuration

- Init scaffolding — `harbor init` creates `~/.harbor/config.yaml` (mode 0600) with the Hetzner token and commented optional sections for DNS, GitHub tokens, and backup credentials.
- Credential config — centralized Hetzner, Cloudflare, and GitHub tokens in `~/.harbor/config.yaml`.
- Per-project GitHub tokens — `github.tokens.<project-name>` maps fine-grained tokens to projects so each deploy uses the right credentials.
- Per-project backup credentials — `backup.projects.<project-name>` maps S3-compatible access keys to projects, mirroring the GitHub token layout.
- Token fallback chain — Hetzner token resolves from user config, then `HCLOUD_TOKEN` env var.
- Fleet configs — `fleet.yaml` composes role directories into named server groups; each role has its own `harbor.yaml`.
- Setup configs — YAML files defining the full provisioning recipe.
- Config path override — `--config` flag to use a custom config file path.
- Secure defaults — credential files created with 0600 permissions.

## CLI

- Shell completions — generates completions for bash, zsh, and fish.
- Colored output — styled terminal output with color-coded success, error, info, and header messages.
- Quiet mode — `--quiet` flag suppresses non-essential output.
- Debug mode — `--debug` flag shows verbose SSH output and internal diagnostics.
- Status command — `harbor status` shows server type, location, IP, last deploy SHA, service health, uptime, disk usage, volume usage (or NOT MOUNTED), and (when configured) the backup timer's active state, last trigger, and next fire time.
- Logs command — `harbor logs [service]` streams journald logs from the server.
- SSH shell — `harbor ssh` opens an interactive shell on the configured server.
- Graceful interrupt — Ctrl+C triggers clean shutdown instead of abrupt termination.
