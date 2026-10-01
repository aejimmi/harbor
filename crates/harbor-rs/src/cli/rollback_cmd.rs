use std::path::Path;

use anyhow::{Context, Result};

use super::deploy_cmd::{resolve_entry, sorted_names};
use super::{output, remote};
use crate::config::setup::DeployConfig;
use crate::provision::{Provisioner, Spinner};
use crate::script::{RollbackComponent, ScriptComponent};

/// Rollback a named deploy to a specific SHA, or to the previous deploy
/// recorded in `~/.harbor/deploys.log` for that name if no SHA is given.
///
/// No git, no rebuild. The versioned binary preserved by the original
/// forward deploy is promoted back via an atomic symlink swap and the
/// services are restarted. A SHA without a preserved versioned dir
/// (pre-feature or already garbage-collected) fails loud.
pub async fn run(
    name: String,
    version: Option<String>,
    debug: bool,
    config_path: Option<&Path>,
) -> Result<()> {
    if let Some(sha) = version.as_deref() {
        anyhow::ensure!(
            is_full_sha(sha),
            "invalid version '{sha}': expected a full 40-character git SHA"
        );
    }
    let server = remote::resolve_server(config_path).await?;
    let names = sorted_names(&server.config);
    let deploy = resolve_entry(&server.config, &name, &names)?;

    if let Some(sha) = version.as_deref() {
        output::header(&format!(
            "Rolling back {name} on {} ({}) to {sha}",
            server.name, server.ip
        ));
    } else {
        output::header(&format!(
            "Rolling back {name} on {} ({}) to previous version",
            server.name, server.ip
        ));
    }

    let script = build_rollback_script(&name, deploy, version.as_deref());
    let spinner = Spinner::start("Connecting via SSH...", debug);

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context(format!("rollback '{name}' failed"));
    }

    spinner.success(format!("Rolled back {name} on {}", server.name));
    Ok(())
}

/// Render the full rollback bash script: lock preamble plus the
/// `RollbackComponent` block. Health-check lines are embedded inside
/// the component so they fire between the symlink swap and the
/// deploys.log write.
pub(super) fn build_rollback_script(
    name: &str,
    deploy: &DeployConfig,
    version: Option<&str>,
) -> String {
    let svc_refs: Vec<&str> = deploy.services.iter().map(String::as_str).collect();
    let health_check = remote::health_check_lines(&svc_refs);

    let rollback_lines = RollbackComponent {
        name: name.to_owned(),
        version: version.map(str::to_owned),
        binary: deploy.binary.clone(),
        install: deploy.install.clone(),
        services: deploy.services.clone(),
        health_check,
    }
    .render();

    let mut lines = vec!["#!/bin/bash".to_owned(), "set -e".to_owned(), String::new()];
    lines.extend(remote::lock_preamble());
    lines.push(String::new());
    lines.extend(rollback_lines);

    lines.join("\n")
}

/// True for a full lowercase-hex git SHA — the form versioned build
/// directories are named by. Also keeps CLI input out of the remote
/// script unless it is plain hex.
pub(super) fn is_full_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
