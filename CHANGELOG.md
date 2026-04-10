# Changelog

## v0.2.0

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
