use std::path::Path;

use anyhow::{Context, Result, bail};

use super::{output, remote};
use crate::config::SetupConfig;
use crate::config::setup::DeployConfig;
use crate::provision::{Provisioner, Spinner};
use crate::script::{DeployComponent, ScriptComponent};

/// Build a deploy script for a single named entry. Health checks are
/// emitted between the symlink swap and the deploys.log write so a
/// broken build fails before the record lands; empty `services` skips
/// the health-check block entirely.
pub(super) fn build_deploy_script(name: &str, deploy: &DeployConfig) -> String {
    let svc_refs: Vec<&str> = deploy.services.iter().map(String::as_str).collect();
    let health_check = remote::health_check_lines(&svc_refs);

    let deploy_lines = DeployComponent {
        name: name.to_owned(),
        repo: deploy.repo.clone(),
        steps: deploy.steps.clone(),
        binary: deploy.binary.clone(),
        install: deploy.install.clone(),
        services: deploy.services.clone(),
        health_check,
    }
    .render();

    let mut lines = vec!["#!/bin/bash".to_owned(), "set -e".to_owned(), String::new()];
    lines.extend(remote::lock_preamble());
    lines.push(String::new());
    lines.extend(deploy_lines);

    lines.join("\n")
}

/// Return the names in `deploys` sorted alphabetically — used for both the
/// `--all` iteration order and user-facing error messages.
pub(super) fn sorted_names(config: &SetupConfig) -> Vec<String> {
    let mut names: Vec<String> = config.setup.deploys.keys().cloned().collect();
    names.sort();
    names
}

/// Format the available-deploys hint appended to lookup errors.
pub(super) fn available_hint(names: &[String]) -> String {
    if names.is_empty() {
        "no deploys configured".to_owned()
    } else {
        format!("available deploys: {}", names.join(", "))
    }
}

/// Resolve a deploy entry by name, returning a helpful error if missing.
pub(super) fn resolve_entry<'a>(
    config: &'a SetupConfig,
    name: &str,
    names: &[String],
) -> Result<&'a DeployConfig> {
    config
        .setup
        .deploys
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("deploy '{name}' not found — {}", available_hint(names)))
}

/// Run a single named deploy end-to-end (build script + SSH + spinner).
async fn run_one(
    name: &str,
    deploy: &DeployConfig,
    server: &remote::ResolvedServer,
    debug: bool,
) -> Result<()> {
    output::header(&format!(
        "Deploying {name} to {} ({})",
        server.name, server.ip
    ));

    let script = build_deploy_script(name, deploy);
    let spinner = Spinner::start("Connecting via SSH...", debug);

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context(format!("deploy '{name}' failed"));
    }

    spinner.success(format!("Deployed {name} to {}", server.name));
    Ok(())
}

/// Pull latest code, rebuild, and restart services for either a single
/// named entry or every entry (alphabetical, sequential, fail-fast).
pub async fn run(
    name: Option<String>,
    all: bool,
    debug: bool,
    config_path: Option<&Path>,
) -> Result<()> {
    // Clap's ArgGroup enforces exactly-one; defend in depth anyway.
    if name.is_some() && all {
        bail!("cannot combine <name> with --all");
    }

    let server = remote::resolve_server(config_path).await?;
    let names = sorted_names(&server.config);

    if all {
        return run_all(&server, &names, debug).await;
    }

    let Some(target) = name else {
        bail!(
            "deploy requires a <name> or --all — {}",
            available_hint(&names)
        );
    };

    let deploy = resolve_entry(&server.config, &target, &names)?;
    run_one(&target, deploy, &server, debug).await
}

/// Iterate every configured deploy in alphabetical order. First failure
/// aborts the loop; otherwise prints a final "N deploys succeeded" line.
/// Run every named deploy in `names` order, stopping at the first failure.
pub(super) async fn run_all(
    server: &remote::ResolvedServer,
    names: &[String],
    debug: bool,
) -> Result<()> {
    if names.is_empty() {
        output::subtle("no deploys configured");
        return Ok(());
    }

    for name in names {
        let deploy = resolve_entry(&server.config, name, names)?;
        run_one(name, deploy, server, debug).await?;
    }

    output::success(&format!("{} deploys succeeded", names.len()));
    Ok(())
}
