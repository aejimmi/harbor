//! Render the server-side backup artifacts for a single project:
//! env file, backup script, restore script, systemd service + timer,
//! and the enable lines that activate the timer.
//!
//! Pure rendering — no filesystem, no network, no env reads. The
//! caller (spec 014) constructs this component from already-resolved
//! `SetupConfig.backup` + `BackupCredentials` inputs.

use crate::config::{BackupSchedule, BackupTransport};

use super::{ScriptComponent, status_echo};

/// Server-side backup component.
///
/// `Debug` is implemented manually so `secret_access_key` is never
/// logged — mirrors `ServiceSpec.env`. `access_key_id` is an
/// identifier, not credential material, so it prints in full.
#[derive(Clone)]
pub struct BackupComponent {
    pub project: String,
    pub transport: BackupTransport,
    pub destination: String,
    pub endpoint: String,
    pub schedule: BackupSchedule,
    pub retention_days: u32,
    pub stop_services: Vec<String>,
    pub paths: Vec<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl std::fmt::Debug for BackupComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secret_repr: &str = if self.secret_access_key.is_empty() {
            ""
        } else {
            "<redacted>"
        };
        f.debug_struct("BackupComponent")
            .field("project", &self.project)
            .field("transport", &self.transport)
            .field("destination", &self.destination)
            .field("endpoint", &self.endpoint)
            .field("schedule", &self.schedule)
            .field("retention_days", &self.retention_days)
            .field("stop_services", &self.stop_services)
            .field("paths", &self.paths)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &secret_repr)
            .finish()
    }
}

impl ScriptComponent for BackupComponent {
    fn render(&self) -> Vec<String> {
        let mut lines = vec![status_echo(&format!(
            "Configuring backup for {}",
            self.project
        ))];
        render_env_file(self, &mut lines);
        if matches!(self.transport, BackupTransport::Rc) {
            lines.push(RC_ALIAS_SETUP.to_owned());
        }
        lines.push(String::new());
        render_backup_script(self, &mut lines);
        lines.push(String::new());
        render_restore_script(self, &mut lines);
        lines.push(String::new());
        render_service_unit(self, &mut lines);
        lines.push(String::new());
        render_timer_unit(self, &mut lines);
        lines.push(String::new());
        render_enable(self, &mut lines);
        lines
    }
}

/// Split `s3://<bucket>[/<prefix>]` into `(bucket, prefix)`. The
/// prefix is returned without leading or trailing slashes so the
/// rendered bash can compose alias paths with `${PREFIX:+/$PREFIX}`
/// cleanly regardless of whether a prefix is present.
fn split_destination(dest: &str) -> (&str, &str) {
    let rest = dest.strip_prefix("s3://").unwrap_or(dest);
    match rest.split_once('/') {
        Some((bucket, prefix)) => (bucket, prefix.trim_matches('/')),
        None => (rest, ""),
    }
}

/// Write `/etc/harbor/backup.env` with mode 0600 root:root.
///
/// The heredoc is single-quoted (`'EOF'`) so bash never expands any
/// credential or path value. Values with `$`, backtick, or `"`
/// render literally.
///
/// `destination` is split at render time into `BACKUP_BUCKET` and
/// `BACKUP_PREFIX` so the rendered bash can build `alias/bucket/key`
/// paths for `rc object …` — the actual `rc` v0.1.11 CLI shape uses
/// nested subcommands against pre-configured aliases, not the
/// flat-command `--endpoint-url` form the mc-family tools mimic.
fn render_env_file(c: &BackupComponent, lines: &mut Vec<String>) {
    let transport = match c.transport {
        BackupTransport::Rc => "rc",
        BackupTransport::Rclone => "rclone",
    };
    let (bucket, prefix) = split_destination(&c.destination);
    lines.push("mkdir -p /etc/harbor".to_owned());
    lines.push("cat > /etc/harbor/backup.env << 'EOF'".to_owned());
    lines.push(format!("AWS_ACCESS_KEY_ID={}", c.access_key_id));
    lines.push(format!("AWS_SECRET_ACCESS_KEY={}", c.secret_access_key));
    lines.push(format!("BACKUP_BUCKET={bucket}"));
    lines.push(format!("BACKUP_PREFIX={prefix}"));
    lines.push(format!("BACKUP_ENDPOINT={}", c.endpoint));
    lines.push(format!("BACKUP_RETENTION_DAYS={}", c.retention_days));
    // Transport is written into the env file so `harbor backup list`
    // (spec 015) can branch without a second config round-trip.
    lines.push(format!("BACKUP_TRANSPORT={transport}"));
    lines.push("EOF".to_owned());
    lines.push("chmod 600 /etc/harbor/backup.env".to_owned());
    lines.push("chown root:root /etc/harbor/backup.env".to_owned());
}

/// Registers the `harbor_backup` rc alias once, at provision time. rc
/// persists it in `~/.config/rc/config.toml`, so the backup and restore
/// scripts never put credentials on a command line (visible in `ps`)
/// on every run. Credentials rotate with the next provision, which also
/// rewrites `backup.env`. Runs in a subshell so the sourced secrets
/// don't leak into the rest of the setup script.
const RC_ALIAS_SETUP: &str = "(source /etc/harbor/backup.env && \
     rc alias set harbor_backup \"$BACKUP_ENDPOINT\" \
     \"$AWS_ACCESS_KEY_ID\" \"$AWS_SECRET_ACCESS_KEY\" --region auto --quiet)";

/// Sources the env file with auto-export so rclone's `env_auth` can read
/// `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` from the environment.
const SOURCE_ENV: &str = "set -a; source /etc/harbor/backup.env; set +a";

/// Bash fragment that builds the alias-qualified path to the bucket
/// root, respecting an empty `BACKUP_PREFIX` without double-slashing.
const RC_BUCKET_ROOT: &str = "harbor_backup/$BACKUP_BUCKET${BACKUP_PREFIX:+/$BACKUP_PREFIX}";

/// Render `/usr/local/bin/harbor-backup-<project>` via heredoc.
fn render_backup_script(c: &BackupComponent, lines: &mut Vec<String>) {
    let project = &c.project;
    lines.push(format!(
        "cat > /usr/local/bin/harbor-backup-{project} << 'BACKUP_SCRIPT_EOF'"
    ));
    lines.extend(backup_script_body(c));
    lines.push("BACKUP_SCRIPT_EOF".to_owned());
    lines.push(format!("chmod 755 /usr/local/bin/harbor-backup-{project}"));
}

/// Body of `harbor-backup-<project>` — between the heredoc markers.
///
/// Transport is chosen at render time in Rust so the rendered bash
/// contains only the branch that matches `self.transport`.
fn backup_script_body(c: &BackupComponent) -> Vec<String> {
    let project = &c.project;
    let mut body = vec![
        "#!/bin/bash".to_owned(),
        "set -euo pipefail".to_owned(),
        String::new(),
        SOURCE_ENV.to_owned(),
    ];
    body.extend([
        "TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)".to_owned(),
        format!("ARCHIVE=/var/lib/harbor/backups/{project}-$TIMESTAMP.tar.gz"),
        format!("KEY={project}-$TIMESTAMP.tar.gz"),
        "mkdir -p /var/lib/harbor/backups".to_owned(),
        String::new(),
    ]);
    // Stop services in declared order and track which we stopped. The
    // EXIT trap restarts them (reverse order, best effort) if anything
    // below fails — a failed stop, a failed tar — so a broken backup
    // never leaves services down.
    body.extend([
        "STOPPED=()".to_owned(),
        "restart_stopped() {".to_owned(),
        "  for ((i=${#STOPPED[@]}-1; i>=0; i--)); do systemctl start \"${STOPPED[i]}\" || true; done".to_owned(),
        "}".to_owned(),
        "trap restart_stopped EXIT".to_owned(),
    ]);
    for svc in &c.stop_services {
        body.push(format!("echo 'Stopping service {svc}'"));
        body.push(format!(
            "systemctl stop '{svc}' || {{ echo 'Failed to stop {svc}' >&2; exit 1; }}"
        ));
        body.push(format!("STOPPED+=('{svc}')"));
    }
    // Archive. `paths` is validated by spec 011 as absolute without
    // traversal; the single-quote wrap here is defence in depth.
    let paths_joined: String = c
        .paths
        .iter()
        .map(|p| format!("'{p}'"))
        .collect::<Vec<_>>()
        .join(" ");
    body.push(format!("tar -czf \"$ARCHIVE\" {paths_joined}"));
    body.push(String::new());
    // Restart in reverse declared order so dependencies that were
    // stopped last come back up first.
    for svc in c.stop_services.iter().rev() {
        body.push(format!("systemctl start '{svc}'"));
    }
    body.push("STOPPED=()".to_owned());
    body.push(String::new());
    // Upload via the picked transport (alias was configured at the
    // top of the script for the rc path).
    match c.transport {
        BackupTransport::Rc => {
            body.push(format!(
                "if rc object copy \"$ARCHIVE\" \"{RC_BUCKET_ROOT}/$KEY\"; then \
                 rm -f \"$ARCHIVE\"; \
                 else \
                 echo 'Upload failed; local archive retained at '\"$ARCHIVE\" >&2; \
                 exit 1; fi",
            ));
        }
        BackupTransport::Rclone => {
            body.push(
                "if rclone copyto --s3-endpoint \"$BACKUP_ENDPOINT\" \"$ARCHIVE\" \
                 \":s3,provider=Other,env_auth=true:$BACKUP_BUCKET${BACKUP_PREFIX:+/$BACKUP_PREFIX}/$KEY\"; \
                 then rm -f \"$ARCHIVE\"; \
                 else \
                 echo 'Upload failed; local archive retained at '\"$ARCHIVE\" >&2; \
                 exit 1; fi"
                    .to_owned(),
            );
        }
    }
    body.push(String::new());
    // Prune — filter by the strict `<project>-YYYYMMDDTHHMMSSZ.tar.gz`
    // pattern before deleting so unrelated objects are never touched.
    body.extend(prune_block(c));
    body.push(String::new());
    body.push(format!("echo \"Backup of {project} uploaded: $KEY\""));
    body
}

/// Render the prune block. Uses awk for date arithmetic so bash
/// stays portable across coreutils versions.
fn prune_block(c: &BackupComponent) -> Vec<String> {
    let project = &c.project;
    match c.transport {
        BackupTransport::Rc => vec![
            format!("CUTOFF=$(date -u -d \"$BACKUP_RETENTION_DAYS days ago\" +%Y%m%dT%H%M%SZ)"),
            // `rc object list harbor_backup/bucket/prefix/` emits
            // keys with the leading prefix directory (e.g.
            // `blissd/blissd-…tar.gz`). Strip to the basename so
            // the `${RC_BUCKET_ROOT}/$OBJ` construction below does
            // not double the prefix.
            //
            // awk replaces `grep` in the filter — grep exits 1 on no
            // match, which with `set -eo pipefail` would abort the
            // whole script on a fresh or empty bucket.
            format!(
                "rc object list \"{RC_BUCKET_ROOT}/\" \
                 | awk '{{ n = $NF; sub(/.*\\//, \"\", n); \
                 if (n ~ /^{project}-[0-9]{{8}}T[0-9]{{6}}Z\\.tar\\.gz$/) print n }}' \
                 | while read -r OBJ; do \
                 OBJTS=$(echo \"$OBJ\" | sed -E 's/^{project}-([0-9T]+Z)\\.tar\\.gz$/\\1/'); \
                 if [ \"$OBJTS\" \\< \"$CUTOFF\" ]; then \
                 rc object remove --force \"{RC_BUCKET_ROOT}/$OBJ\" || true; \
                 fi; \
                 done"
            ),
        ],
        BackupTransport::Rclone => vec![format!(
            "rclone delete --min-age \"${{BACKUP_RETENTION_DAYS}}d\" \
                 --include '{project}-*.tar.gz' \
                 --s3-endpoint \"$BACKUP_ENDPOINT\" \
                 \":s3,provider=Other,env_auth=true:$BACKUP_BUCKET${{BACKUP_PREFIX:+/$BACKUP_PREFIX}}/\" \
                 || true"
        )],
    }
}

/// Render `/usr/local/bin/harbor-restore-<project>` via heredoc.
fn render_restore_script(c: &BackupComponent, lines: &mut Vec<String>) {
    let project = &c.project;
    lines.push(format!(
        "cat > /usr/local/bin/harbor-restore-{project} << 'RESTORE_SCRIPT_EOF'"
    ));
    lines.extend(restore_script_body(c));
    lines.push("RESTORE_SCRIPT_EOF".to_owned());
    lines.push(format!("chmod 755 /usr/local/bin/harbor-restore-{project}"));
}

/// Body of `harbor-restore-<project>` — handles both $1-present and
/// $1-absent branches, runs `tar tzf` pre-flight before touching
/// live data, uses `mv -T` for every atomic swap.
fn restore_script_body(c: &BackupComponent) -> Vec<String> {
    let project = &c.project;
    let mut body = vec![
        "#!/bin/bash".to_owned(),
        "set -euo pipefail".to_owned(),
        String::new(),
        SOURCE_ENV.to_owned(),
    ];
    body.push(String::new());
    // Resolve the target archive key.
    body.extend([
        "if [ -n \"${1:-}\" ]; then".to_owned(),
        format!("  KEY={project}-$1.tar.gz"),
        "else".to_owned(),
    ]);
    body.extend(resolve_newest_key_lines(c));
    body.push("fi".to_owned());
    body.push(String::new());
    body.push("WORK=$(mktemp -d /var/lib/harbor/restore.XXXXXX)".to_owned());
    body.push("trap 'rm -rf \"$WORK\"' EXIT".to_owned());
    body.push(String::new());
    // Download (alias was configured at the top of the script for
    // the rc path).
    match c.transport {
        BackupTransport::Rc => {
            body.push(format!(
                "rc object copy \"{RC_BUCKET_ROOT}/$KEY\" \"$WORK/archive.tar.gz\"",
            ));
        }
        BackupTransport::Rclone => body.push(
            "rclone copyto --s3-endpoint \"$BACKUP_ENDPOINT\" \
             \":s3,provider=Other,env_auth=true:$BACKUP_BUCKET${BACKUP_PREFIX:+/$BACKUP_PREFIX}/$KEY\" \
             \"$WORK/archive.tar.gz\""
                .to_owned(),
        ),
    }
    body.push(String::new());
    // Pre-flight: verify the archive parses BEFORE touching live data.
    body.push("tar tzf \"$WORK/archive.tar.gz\" > \"$WORK/manifest.txt\"".to_owned());
    body.push(String::new());
    // Stop services in declared order.
    for svc in &c.stop_services {
        body.push(format!("systemctl stop '{svc}'"));
    }
    body.push("RESTORE_TS=$(date -u +%Y%m%dT%H%M%SZ)".to_owned());
    body.push("RESTORED=()".to_owned());
    body.extend(restore_rollback_fn(c));
    body.push(String::new());
    // Extract + atomic swap per path.
    body.extend(swap_paths_lines(c));
    body.push(String::new());
    // Restart services in reverse declared order.
    for svc in c.stop_services.iter().rev() {
        body.push(format!("systemctl start '{svc}'"));
    }
    body.push(String::new());
    // Cleanup previous live data.
    for path in &c.paths {
        body.push(format!("rm -rf '{path}.old-'*"));
    }
    body.push(format!("echo \"Restore of {project} from $KEY completed\""));
    body
}

/// List + filter + sort newest-first; fail when the bucket has no
/// matching archive. Emits bash into the `else` branch of the
/// `[ -n \"$1\" ]` check.
fn resolve_newest_key_lines(c: &BackupComponent) -> Vec<String> {
    let project = &c.project;
    match c.transport {
        BackupTransport::Rc => vec![
            // Strip any leading prefix directory on the key and
            // filter on basename — same rationale as prune_block.
            format!(
                "  KEY=$(rc object list \"{RC_BUCKET_ROOT}/\" \
                 | awk '{{ n = $NF; sub(/.*\\//, \"\", n); \
                 if (n ~ /^{project}-[0-9]{{8}}T[0-9]{{6}}Z\\.tar\\.gz$/) print n }}' \
                 | sort -r | head -n 1)"
            ),
            "  if [ -z \"$KEY\" ]; then echo 'No backups found' >&2; exit 1; fi".to_owned(),
        ],
        BackupTransport::Rclone => vec![
            format!(
                "  KEY=$(rclone lsf --s3-endpoint \"$BACKUP_ENDPOINT\" \
                 \":s3,provider=Other,env_auth=true:$BACKUP_BUCKET${{BACKUP_PREFIX:+/$BACKUP_PREFIX}}/\" \
                 | awk '$0 ~ /^{project}-[0-9]{{8}}T[0-9]{{6}}Z\\.tar\\.gz$/' \
                 | sort -r | head -n 1)"
            ),
            "  if [ -z \"$KEY\" ]; then echo 'No backups found' >&2; exit 1; fi".to_owned(),
        ],
    }
}

/// Bash `restore_rollback <msg>`: put every swapped path back (removing
/// the partially restored copy first, since `mv -T` can't replace a
/// non-empty directory), restart services, and exit 1.
fn restore_rollback_fn(c: &BackupComponent) -> Vec<String> {
    let services = c
        .stop_services
        .iter()
        .rev()
        .map(|s| format!("'{s}'"))
        .collect::<Vec<_>>()
        .join(" ");
    vec![
        "restore_rollback() {".to_owned(),
        "  echo \"$1\" >&2".to_owned(),
        "  for p in \"${RESTORED[@]}\"; do".to_owned(),
        "    rm -rf \"$p\"".to_owned(),
        "    if [ -e \"$p.old-$RESTORE_TS\" ]; then mv -T \"$p.old-$RESTORE_TS\" \"$p\" || true; fi"
            .to_owned(),
        "  done".to_owned(),
        format!("  for svc in {services}; do systemctl start \"$svc\" || true; done"),
        "  exit 1".to_owned(),
        "}".to_owned(),
    ]
}

/// For each declared path: extract into `$WORK/extracted`, move the
/// live path aside as `.old-$RESTORE_TS` (if it exists — a fresh server
/// has nothing to move), then move the restored content in. Every step
/// is checked explicitly: `set -e` does not fire inside `a && b` lists.
fn swap_paths_lines(c: &BackupComponent) -> Vec<String> {
    let mut lines = Vec::new();
    for path in &c.paths {
        // Leading slash is already included in `path`. The restore
        // script uses it verbatim — spec 011 validated absolute +
        // traversal-free at config load.
        let trimmed = path.trim_start_matches('/');
        lines.extend([
            "mkdir -p \"$WORK/extracted\"".to_owned(),
            format!(
                "tar -xzf \"$WORK/archive.tar.gz\" -C \"$WORK/extracted\" '{trimmed}' \
                 || restore_rollback 'Extract failed for {path}'"
            ),
            format!(
                "if [ -e '{path}' ]; then mv -T '{path}' '{path}.old-'\"$RESTORE_TS\" \
                 || restore_rollback 'Failed to move aside {path}'; fi"
            ),
            format!("mkdir -p \"$(dirname '{path}')\""),
            format!("RESTORED+=('{path}')"),
            format!(
                "mv -T \"$WORK/extracted/{trimmed}\" '{path}' \
                 || restore_rollback 'Failed to swap in {path}'"
            ),
        ]);
    }
    lines
}

/// Write `/etc/systemd/system/harbor-backup-<project>.service`.
fn render_service_unit(c: &BackupComponent, lines: &mut Vec<String>) {
    let project = &c.project;
    lines.push(format!(
        "cat > /etc/systemd/system/harbor-backup-{project}.service << 'EOF'"
    ));
    lines.extend([
        "[Unit]".to_owned(),
        format!("Description=Harbor backup for {project}"),
        "After=network-online.target".to_owned(),
        "Wants=network-online.target".to_owned(),
        String::new(),
        "[Service]".to_owned(),
        "Type=oneshot".to_owned(),
        format!("ExecStart=/usr/local/bin/harbor-backup-{project}"),
        "StandardOutput=journal".to_owned(),
        "StandardError=journal".to_owned(),
    ]);
    lines.push("EOF".to_owned());
}

/// Write `/etc/systemd/system/harbor-backup-<project>.timer`.
fn render_timer_unit(c: &BackupComponent, lines: &mut Vec<String>) {
    let project = &c.project;
    let oncal = match c.schedule {
        BackupSchedule::Hourly => "hourly",
        BackupSchedule::Daily => "daily",
        BackupSchedule::Weekly => "weekly",
    };
    lines.push(format!(
        "cat > /etc/systemd/system/harbor-backup-{project}.timer << 'EOF'"
    ));
    lines.extend([
        "[Unit]".to_owned(),
        format!("Description=Harbor backup timer for {project}"),
        String::new(),
        "[Timer]".to_owned(),
        format!("OnCalendar={oncal}"),
        "RandomizedDelaySec=1h".to_owned(),
        "Persistent=yes".to_owned(),
        format!("Unit=harbor-backup-{project}.service"),
        String::new(),
        "[Install]".to_owned(),
        "WantedBy=timers.target".to_owned(),
    ]);
    lines.push("EOF".to_owned());
}

/// `daemon-reload` + `enable --now` the timer. The service is NEVER
/// enabled — it runs only when the timer fires or `harbor backup run`
/// (spec 015) triggers it via `systemctl start`.
fn render_enable(c: &BackupComponent, lines: &mut Vec<String>) {
    let project = &c.project;
    lines.push("systemctl daemon-reload".to_owned());
    lines.push(format!(
        "systemctl enable --now harbor-backup-{project}.timer"
    ));
}
