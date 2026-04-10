use std::path::Path;

use anyhow::{Context, Result};

use super::{output, remote};
use crate::config::SetupConfig;
use crate::provision::{Provisioner, Spinner};
use crate::script::{DeployComponent, ScriptComponent};

/// Build a deploy script with lock, deploy steps, and health checks.
fn build_deploy_script(setup_config: &SetupConfig) -> Result<String> {
    let deploy = setup_config
        .setup
        .deploy
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'deploy:' section in harbor.yaml"))?;

    let deploy_lines = DeployComponent {
        repo: deploy.repo.clone(),
        steps: deploy.steps.clone(),
    }
    .render();

    let services = remote::started_services(setup_config);
    let svc_refs: Vec<&str> = services.iter().map(String::as_str).collect();

    let mut lines = vec!["#!/bin/bash".to_owned(), "set -e".to_owned(), String::new()];

    lines.extend(remote::lock_preamble());
    lines.push(String::new());
    lines.extend(deploy_lines);
    lines.extend(remote::health_check_lines(&svc_refs));

    Ok(lines.join("\n"))
}

/// Pull latest code, rebuild, and restart services.
pub async fn run(debug: bool, config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;

    output::header(&format!("Deploying to {} ({})", server.name, server.ip));

    let script = build_deploy_script(&server.config)?;
    let spinner = Spinner::start("Connecting via SSH...", debug);

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context("deploy failed");
    }

    spinner.success(format!("Deployed to {}", server.name));
    Ok(())
}
