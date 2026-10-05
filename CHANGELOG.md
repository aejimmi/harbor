# Changelog

## v0.4.0

New:
- volumes: server.volumes: attaches Hetzner volumes — harbor up creates one, or reattaches an existing one with the same name
- volumes: formatted only when blank (ext4 default, xfs optional), mounted by UUID with nofail before any directories, files or services are set up
- volumes: harbor down detaches and keeps the volume; harbor status shows usage, or NOT MOUNTED
- volumes: a volume in another location or attached to another server is refused before any server is created
- volumes: fleets and harbor server create refuse a config that declares volumes
- system: system.journald_max_use: caps the journal on the root disk

Fix:
- up: harbor up no longer fails after creating the server — Hetzner dropped datacenter from server responses (hcloud 0.26)

## v0.3.0

New:
- backup: declarative backup: block archives paths on a schedule (hourly/daily/weekly with 1h jitter), ships to S3-compatible storage, and prunes by retention_days
- backup: harbor backup runs on demand; harbor backup list shows archives newest-first
- backup: transport choice between pinned rc (pure Rust, default) and upstream rclone; services are stopped for the archive and always restarted, even if the backup fails; S3 credentials never appear on a command line
- restore: harbor restore replaces live data with a pre-flight summary, confirmation (--yes skips), and per-path swap that rolls back on any failure; works on a fresh server; --at picks a specific archive
- deploy: named deploys under deploys: — run one with harbor deploy <name>, or all with --all
- deploy: builds are kept under /opt/harbor/<name>/<sha>/ so rollback is a symlink swap with no rebuild; last five kept
- up: harbor up runs every configured deploy after provisioning
- services: container security controls — cap_drop, cap_add, read_only, pids_limit; no-new-privileges and pids_limit=256 by default
- security: mount hardening remounts /tmp, /var/tmp, /dev/shm with noexec,nosuid,nodev
- ssh: host keys are pinned in ~/.harbor/known_hosts; a changed key is rejected
- status: shows backup timer state, last run, and next run
- examples: ready-to-copy configs in examples/ — minimal app, container service with backups, two-role fleet

Breaking:
- deploy: the deploy: block is replaced by the deploys: map; harbor deploy and harbor rollback now take a <name>
- deploy: each deploys: entry requires binary (repo-relative) and install (absolute symlink path)
- config: harbor.yaml is validated at load — unknown setup: keys, unsafe names, relative or traversing paths, non-octal modes, and multi-line env values are rejected
- config: setup.github_repos removed — use deploys:
- init: harbor init writes only ~/.harbor/config.yaml; the configs-deploy/ and configs-server/ templates are gone
- server: harbor server create uses the discovered harbor.yaml when --setup-config is omitted
- dns: no default base_domain — DNS is managed only when cloudflare credentials and dns.base_domain are set

Fix:
- provision: failing setup and deploy scripts are reported as failed — previously some failures showed as success
- provision: a missing or empty ssh-agent fails immediately instead of retrying for 5 minutes
- rollback: steps back through deploy history from the live version — no longer a silent no-op with one deploy, or flipping between two versions when repeated; a given SHA must be a full 40-char SHA
- fleet: config and credentials are checked before any server is created, so an error can't leave a paid server unprovisioned
- fleet: fleet down keeps going past individual failures and reports them
- fleet: role harbor.yaml files no longer need server.name
- up: a server created without an IP fails instead of reporting success
- security: ufw always allows SSH (22/tcp), so a rules list without it can't lock you out
- config: a missing files: source fails setup instead of being skipped with a warning
- config: HCLOUD_TOKEN works for every command, not just fleet
- deploy: ssh-style git remotes (git@host:path) are no longer rewritten with https://
- deps: russh 0.60.3 and h2 0.4.19 for RUSTSEC-2026-0153, -0154, -0258

## v0.2.0

New:
- fleet: compose role directories into named server groups with fleet up, fleet down, and fleet status
- fleet: mandatory fleet name generates deterministic server names as role-name-N
- fleet: short form in fleet.yaml for roles (collectors: 3) and long form when directory name differs
- fleet: fail-fast validation checks all role directories and harbor.yaml files before creating any servers
- fleet: concurrent creation by default, sequential flag available

Breaking:
- cli: harbor env replaced by harbor fleet with new subcommands (up, down, status instead of deploy, destroy, list)
- cli: harbor env still works as an alias during transition

New:
- services: run any Docker image as a managed service with ports, volumes, and env vars — Harbor handles pull, run, lifecycle, and logs
- services: Podman Quadlet as an opt-in runtime for daemonless container execution, selected per service
- services: container env vars stored in 0600 env files on the server instead of inline in world-readable unit files
- services: Docker or Podman auto-installed based on which runtimes the config references
- provision: spinner truncates long status lines so they fit the terminal instead of wrapping

Fix:
- services: secret env values redacted from debug logs and panic output
- services: config load fails with a clear error when a service sets both image and exec_start
- services: config load fails with a clear error when image is empty or whitespace-only
- provision: spinner only advances on harbor's own status lines, ignoring arbitrary apt and dpkg chatter

## v0.1.0

New:
- rollback: roll back to the previous deploy or a specific git SHA
- exec: run a one-off command on the server
- deploy: concurrent deploys are blocked by a lock file, stale locks auto-cleared after 30 min
- deploy: history is recorded to ~/.harbor/deploys.log on every deploy and rollback
- deploy: services are health-checked after deploy and rollback
- status: shows app state — last deploy, service health, uptime, disk
- provision: ticking spinner with elapsed time for up/deploy/rollback, --debug flag streams raw output
- config: per-project GitHub tokens under github.tokens.<project-name>

Fix:
- provision: SSH keepalive prevents timeout during long silent builds (cargo build --release)
- script: apt upgrades run non-interactive and keep existing config files — no more dpkg prompts
- script: services restart instead of start on redeploy, so config changes actually apply
- script: git HTTPS auth uses x-access-token format for fine-grained GitHub tokens
- script: system user creation is idempotent and fails loud if the user is missing afterwards
- ssh: accept new host keys on first connection for ssh, exec, logs, and status
