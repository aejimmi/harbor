# Changelog

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
