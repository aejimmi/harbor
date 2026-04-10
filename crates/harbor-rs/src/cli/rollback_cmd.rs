use std::path::Path;

use anyhow::{Context, Result};

use super::{output, remote};
use crate::provision::{Provisioner, Spinner};
use crate::script::{DeployComponent, RollbackComponent, ScriptComponent};

/// Rollback to a specific version (or previous deploy).
pub async fn run(version: Option<String>, debug: bool, config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    let deploy = server
        .config
        .setup
        .deploy
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'deploy:' section in harbor.yaml"))?;

    let rollback_lines = if let Some(ref sha) = version {
        output::header(&format!(
            "Rolling back {} ({}) to {}",
            server.name, server.ip, sha
        ));
        RollbackComponent {
            repo: deploy.repo.clone(),
            version: sha.clone(),
            steps: deploy.steps.clone(),
        }
        .render()
    } else {
        output::header(&format!(
            "Rolling back {} ({}) to previous version",
            server.name, server.ip
        ));
        rollback_to_previous(&deploy.repo, &deploy.steps)
    };

    let services = remote::started_services(&server.config);
    let svc_refs: Vec<&str> = services.iter().map(String::as_str).collect();

    let mut lines = vec!["#!/bin/bash".to_owned(), "set -e".to_owned(), String::new()];

    lines.extend(remote::lock_preamble());
    lines.push(String::new());
    lines.extend(rollback_lines);
    lines.extend(remote::health_check_lines(&svc_refs));

    let script = lines.join("\n");
    let spinner = Spinner::start("Connecting via SSH...", debug);

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context("rollback failed");
    }

    spinner.success(format!("Rolled back {}", server.name));
    Ok(())
}

/// Build rollback lines that read the previous SHA from deploys.log on the server.
fn rollback_to_previous(repo: &str, steps: &[String]) -> Vec<String> {
    let repo_name = DeployComponent::repo_name(repo);

    let mut lines = vec![
        "if [ ! -f ~/.harbor/deploys.log ]; then".to_owned(),
        "  echo 'No deploy history found' >&2".to_owned(),
        "  exit 1".to_owned(),
        "fi".to_owned(),
        "PREV_SHA=$(tail -n 2 ~/.harbor/deploys.log | head -n 1 | awk '{print $3}')".to_owned(),
        "if [ -z \"$PREV_SHA\" ]; then".to_owned(),
        "  echo 'No previous version found in deploy history' >&2".to_owned(),
        "  exit 1".to_owned(),
        "fi".to_owned(),
        "echo \"Rolling back to $PREV_SHA\"".to_owned(),
        format!("cd $HOME/{repo_name}"),
        "git fetch --all".to_owned(),
        "git checkout $PREV_SHA".to_owned(),
    ];

    for step in steps {
        lines.push(step.clone());
    }

    // Record rollback
    lines.push("mkdir -p ~/.harbor".to_owned());
    lines.push(
        "echo \"$(date -u +%Y-%m-%dT%H:%M:%SZ) $(whoami) $(git rev-parse HEAD) rollback\" >> ~/.harbor/deploys.log"
            .to_owned(),
    );
    lines.push("echo 'Rollback complete'".to_owned());

    lines
}
