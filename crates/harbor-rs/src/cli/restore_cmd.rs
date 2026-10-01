//! `harbor restore [--at <timestamp>] [--yes]`.
//!
//! Project resolved from `harbor.yaml` in cwd — matches the
//! no-argv shape of `harbor up` / `harbor status`. Interactive by
//! default — the operator confirms before the rendered restore
//! script replaces live data. `--yes` skips the prompt for
//! scripted flows. When `--at` is omitted, harbor resolves the
//! newest archive itself (closes the race window described in the
//! 016 threat model) and passes that explicit timestamp to the
//! server-side restore script.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{backup_cmd, output, prompt, remote};
use crate::provision::{Provisioner, Spinner};

pub async fn run(
    at: Option<String>,
    yes: bool,
    debug: bool,
    config_path: Option<&Path>,
) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    let Some(backup) = server.config.setup.backup.as_ref() else {
        bail!(
            "project '{}' has no backup: block in harbor.yaml — nothing to restore",
            server.config.name
        );
    };
    let name = server.config.name.clone();

    output::header(&format!(
        "Restore {name} on {} ({})",
        server.name, server.ip
    ));

    // Resolve the target timestamp client-side so the rendered
    // script always receives an explicit argument — no race between
    // `list` (below) and the server's "newest" resolver.
    let timestamp = resolve_timestamp(at.clone(), &name, &server)?;

    // Pre-flight summary — what is about to change on the server.
    print_preflight_summary(&name, &timestamp, &backup.stop_services, &backup.paths);

    if !yes {
        let proceed = prompt::confirm("Proceed? (this replaces live data)")?;
        if !proceed {
            output::subtle("restore cancelled");
            return Ok(());
        }
    }

    let script = build_restore_script(&name, &timestamp);
    let spinner = Spinner::start("Connecting via SSH...", debug);

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context("restore failed");
    }

    spinner.success(format!("Restored {name}"));
    Ok(())
}

/// Return the timestamp to restore — explicit when `--at` is set,
/// else the newest matching archive from the bucket.
///
/// An explicit `--at` must match the strict archive timestamp shape:
/// it is interpolated into the remote restore command.
fn resolve_timestamp(
    at: Option<String>,
    name: &str,
    server: &remote::ResolvedServer,
) -> Result<String> {
    if let Some(ts) = at {
        if !backup_cmd::is_valid_timestamp(&ts) {
            bail!("invalid --at '{ts}': expected YYYYMMDDTHHMMSSZ, e.g. 20260101T030000Z");
        }
        return Ok(ts);
    }
    // No `--at`: ask the server for the newest key via the list
    // helper. If the bucket is empty, fail before prompting.
    let raw = backup_cmd::run_ssh(server.ip, &backup_cmd::build_list_script())?;
    let rows = backup_cmd::parse_list_output(&raw, name);
    let newest = rows.into_iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "no backups to restore — run 'harbor backup' first, \
             or pass --at <timestamp> if the archive already exists"
        )
    })?;
    // `parse_list_output` only yields rows that match the strict
    // shape, so a missing timestamp here is an internal error.
    backup_cmd::key_timestamp(&newest.key).ok_or_else(|| {
        anyhow::anyhow!(
            "internal: parsed row key does not carry a timestamp: {}",
            newest.key
        )
    })
}

/// Pretty-print what's about to happen before the confirm prompt.
pub(super) fn print_preflight_summary(
    name: &str,
    timestamp: &str,
    stop_services: &[String],
    paths: &[String],
) {
    output::info(&format!("restoring {name}-{timestamp}.tar.gz"));
    if !stop_services.is_empty() {
        output::info(&format!("stopping services: {}", stop_services.join(", ")));
    }
    if !paths.is_empty() {
        output::info("replacing paths:");
        for p in paths {
            output::info(&format!("  {p}"));
        }
    }
    if !stop_services.is_empty() {
        output::info(&format!("starting services: {}", stop_services.join(", ")));
    }
}

/// Build the two-line SSH script that invokes the rendered
/// restore binary with an explicit timestamp.
pub(super) fn build_restore_script(name: &str, timestamp: &str) -> String {
    format!(
        "#!/bin/bash\n\
         set -e\n\
         /usr/local/bin/harbor-restore-{name} {timestamp}\n"
    )
}
