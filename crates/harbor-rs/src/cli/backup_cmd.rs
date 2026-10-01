//! `harbor backup` + `harbor backup list`.
//!
//! `run` triggers the server's oneshot backup service via
//! `systemctl start --wait` through the existing Provisioner +
//! Spinner flow. `list` issues a short-lived read-only SSH call
//! that matches `status_cmd.rs::fetch_app_state`'s shape —
//! no Provisioner, no spinner, just the raw command.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{output, remote};
use crate::provision::{Provisioner, Spinner};

/// Run an on-demand backup via SSH + `systemctl start --wait`.
///
/// Project name is resolved from `harbor.yaml` in cwd — matching
/// `harbor up` / `harbor deploy` / `harbor status`. No argv.
pub async fn run(debug: bool, config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    if server.config.setup.backup.is_none() {
        bail!(
            "project '{}' has no backup: block in harbor.yaml — nothing to run",
            server.config.name
        );
    }
    let name = server.config.name.clone();

    output::header(&format!(
        "Running backup for {name} on {} ({})",
        server.name, server.ip
    ));

    let script = build_backup_script(&name);
    let spinner = Spinner::start("Connecting via SSH...", debug);

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context("backup failed");
    }

    spinner.success(format!("Backup of {name} completed"));
    Ok(())
}

/// List existing backup archives in the remote bucket.
pub async fn list(config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    if server.config.setup.backup.is_none() {
        bail!(
            "project '{}' has no backup: block in harbor.yaml — nothing to list",
            server.config.name
        );
    }
    let name = server.config.name.clone();

    output::header(&format!("Backups for {name} on {}", server.name));

    let script = build_list_script();
    let raw = match run_ssh(server.ip, &script) {
        Ok(s) => s,
        Err(e) => {
            output::subtle("  (SSH unavailable)");
            return Err(e);
        }
    };

    let rows = parse_list_output(&raw, &name);
    if rows.is_empty() {
        output::subtle("  no backups found");
        return Ok(());
    }

    output::info(&format!("{:<24} {:<12} key", "timestamp", "size"));
    for row in rows {
        output::info(&format!(
            "{:<24} {:<12} {}",
            row.timestamp, row.size, row.key
        ));
    }
    Ok(())
}

/// Minimal bash script — trigger the oneshot unit, then explicitly
/// check the unit's final Result property. `systemctl status | head`
/// was masking failures: pipelines return the exit code of the LAST
/// command (head, which always succeeds), so a failed oneshot would
/// surface as ✓ success to the harbor CLI.
///
/// We read `Result` + `ExecMainStatus` directly and fail loud when
/// the oneshot didn't exit cleanly. Failure dumps the tail of the
/// service's journal so the operator sees the real error on the
/// laptop side without another SSH.
pub(super) fn build_backup_script(name: &str) -> String {
    format!(
        "#!/bin/bash\n\
         set -e\n\
         UNIT=harbor-backup-{name}.service\n\
         systemctl start --wait \"$UNIT\" || true\n\
         RESULT=$(systemctl show -p Result --value \"$UNIT\")\n\
         EXIT=$(systemctl show -p ExecMainStatus --value \"$UNIT\")\n\
         if [ \"$RESULT\" != \"success\" ]; then\n\
           echo \"Backup failed: Result=$RESULT ExecMainStatus=$EXIT\" >&2\n\
           journalctl -u \"$UNIT\" --no-pager -n 30 >&2\n\
           exit 1\n\
         fi\n\
         systemctl status --no-pager \"$UNIT\" | head -n 20 || true\n"
    )
}

/// Bash for `harbor backup list`. Uses the env-file transport
/// selector to branch between rc and rclone. The rc path asks for
/// JSON (`--json`) because rc switches to JSON automatically on
/// non-tty stdout anyway — being explicit keeps the contract with
/// the client parser stable across rc versions.
pub(super) fn build_list_script() -> String {
    r#"#!/bin/bash
set -e
set -a; source /etc/harbor/backup.env; set +a
if [ "$BACKUP_TRANSPORT" = "rclone" ]; then
  rclone lsl --s3-endpoint "$BACKUP_ENDPOINT" ":s3,provider=Other,env_auth=true:$BACKUP_BUCKET${BACKUP_PREFIX:+/$BACKUP_PREFIX}/"
else
  rc object list --json "harbor_backup/$BACKUP_BUCKET${BACKUP_PREFIX:+/$BACKUP_PREFIX}/"
fi
"#
    .to_owned()
}

/// One row of the parsed list table.
pub(super) struct ListRow {
    pub timestamp: String,
    pub size: String,
    pub key: String,
}

/// Parse list output and keep only rows whose key matches
/// `<name>-YYYYMMDDTHHMMSSZ.tar.gz`. Sorts newest-first on the
/// timestamp embedded in the key.
///
/// `rc object list --json` emits a structured object. We try JSON
/// first (the canonical rc output); if the input doesn't parse as
/// JSON — the rclone path or any future transport that stays with
/// human-readable output — we fall back to the whitespace-split
/// parser.
pub(super) fn parse_list_output(raw: &str, name: &str) -> Vec<ListRow> {
    let mut rows = parse_list_json(raw, name).unwrap_or_else(|| {
        raw.lines()
            .filter_map(|line| parse_list_line(line, name))
            .collect()
    });
    // Newest-first by the timestamp embedded in the key.
    rows.sort_by(|a, b| b.key.cmp(&a.key));
    rows
}

/// Serde shape for `rc object list --json` output. Only the fields
/// we actually use. Extra fields (etag, storage_class, is_dir, etc.)
/// are ignored by serde's default behavior.
#[derive(serde::Deserialize)]
struct RcListResponse {
    items: Vec<RcListItem>,
}

#[derive(serde::Deserialize)]
struct RcListItem {
    key: String,
    #[serde(default)]
    size_human: String,
    #[serde(default)]
    last_modified: String,
}

/// Try to parse `raw` as the rc JSON response. Returns None when
/// the payload isn't JSON (e.g. rclone output) so the caller can
/// fall back to the line-based parser.
fn parse_list_json(raw: &str, name: &str) -> Option<Vec<ListRow>> {
    // rc JSON is the first `{ ... }` block in the payload; skip any
    // leading SSH banner lines ("Authorized access only.", etc.).
    let start = raw.find('{')?;
    let slice = &raw[start..];
    let resp: RcListResponse = serde_json::from_str(slice).ok()?;
    Some(
        resp.items
            .into_iter()
            .filter_map(|item| {
                let key = item.key.rsplit('/').next().unwrap_or(&item.key).to_owned();
                if !matches_project_key(&key, name) {
                    return None;
                }
                let timestamp = if item.last_modified.is_empty() {
                    key_timestamp(&key).unwrap_or_else(|| "unknown".to_owned())
                } else {
                    item.last_modified
                };
                Some(ListRow {
                    timestamp,
                    size: item.size_human,
                    key,
                })
            })
            .collect(),
    )
}

/// Parse one line of list output. Handles the two shapes we care
/// about:
///   `[YYYY-MM-DD HH:MM:SS]   <size> <unit> <bucket/prefix/key>` (rc)
///   `<size> <date> <time> <key>` (rclone lsl)
///
/// The key as printed can include a prefix directory (`blissd/foo.tar.gz`)
/// when the caller listed by prefix — we match on the leaf basename so
/// both shapes work. Size is the last two tokens before the key when
/// they look like `<number> <unit>`, falling back to the first token
/// when they don't.
fn parse_list_line(line: &str, name: &str) -> Option<ListRow> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let full_key = *parts.last()?;
    let key = full_key.rsplit('/').next().unwrap_or(full_key);
    if !matches_project_key(key, name) {
        return None;
    }
    let size = extract_size(&parts);
    let timestamp = key_timestamp(key).unwrap_or_else(|| "unknown".to_owned());
    Some(ListRow {
        timestamp,
        size,
        key: key.to_owned(),
    })
}

/// Best-effort size extraction across the two transports' formats.
/// When the two tokens immediately before the key look like
/// `<number> <unit>`, return them joined. Otherwise fall back to the
/// first non-empty token.
fn extract_size(parts: &[&str]) -> String {
    let n = parts.len();
    if let (Some(&maybe_num), Some(&maybe_unit)) =
        (parts.get(n.wrapping_sub(3)), parts.get(n.wrapping_sub(2)))
    {
        let is_num = maybe_num.chars().all(|c| c.is_ascii_digit() || c == '.');
        let is_unit = matches!(
            maybe_unit,
            "B" | "KB" | "MB" | "GB" | "TB" | "KiB" | "MiB" | "GiB" | "TiB"
        );
        if is_num && is_unit {
            return format!("{maybe_num} {maybe_unit}");
        }
    }
    parts.first().copied().unwrap_or("").to_owned()
}

/// Strict regex-ish match — `<name>-YYYYMMDDTHHMMSSZ.tar.gz`, with
/// digit counts enforced so adversarial keys can't slip through.
/// `key` here is the basename — callers strip any leading prefix dir.
pub(super) fn matches_project_key(key: &str, name: &str) -> bool {
    let prefix = format!("{name}-");
    let Some(ts_and_ext) = key.strip_prefix(&prefix) else {
        return false;
    };
    let Some(ts) = ts_and_ext.strip_suffix(".tar.gz") else {
        return false;
    };
    is_valid_timestamp(ts)
}

/// True when `ts` has the exact `YYYYMMDDTHHMMSSZ` archive shape.
pub(super) fn is_valid_timestamp(ts: &str) -> bool {
    // `YYYYMMDDTHHMMSSZ` — 8 digits, T, 6 digits, Z.
    if ts.len() != 16 {
        return false;
    }
    let bytes = ts.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        let ok = match i {
            0..=7 | 9..=14 => b.is_ascii_digit(),
            8 => *b == b'T',
            15 => *b == b'Z',
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Extract the timestamp from a validated key for display.
pub(super) fn key_timestamp(key: &str) -> Option<String> {
    let dash = key.rfind('-')?;
    let after = key.get(dash + 1..)?;
    let ts = after.strip_suffix(".tar.gz")?;
    if is_valid_timestamp(ts) {
        Some(ts.to_owned())
    } else {
        None
    }
}

/// One-shot SSH helper. Matches `status_cmd::fetch_app_state`'s
/// shape — no Provisioner, no Spinner — and returns stdout on
/// success, surfacing stderr/timeout errors via anyhow. Goes
/// through `remote::ssh_exec_script` so the script is interpreted
/// by bash regardless of the remote user's login shell.
pub(super) fn run_ssh(ip: std::net::IpAddr, script: &str) -> Result<String> {
    let out = super::remote::ssh_exec_script(ip, script).context("running ssh for backup list")?;
    if !out.status.success() {
        bail!(
            "ssh failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
