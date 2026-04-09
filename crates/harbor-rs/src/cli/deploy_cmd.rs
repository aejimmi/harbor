use std::path::Path;

use anyhow::{Context, Result};

use super::{discover, output};
use crate::config::UserConfig;
use crate::provider::CloudProvider;
use crate::provision::Provisioner;

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    let (setup_config, _) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;
    let deploy = setup_config
        .setup
        .deploy
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'deploy:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);

    let existing = provider
        .get_server(&server.name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("server '{}' not found — run `harbor up` first", server.name))?;
    let ip = existing
        .ip
        .ok_or_else(|| anyhow::anyhow!("server '{}' has no IP", server.name))?;

    output::header(&format!("Deploying to {} ({})", server.name, ip));

    let repo_name = deploy
        .repo
        .rsplit('/')
        .next()
        .unwrap_or(&deploy.repo)
        .trim_end_matches(".git");
    let clone_url = if deploy.repo.starts_with("http") {
        deploy.repo.clone()
    } else {
        format!("https://{}", deploy.repo)
    };

    let mut script_lines = vec![
        "#!/bin/bash".to_owned(),
        "set -e".to_owned(),
        String::new(),
        format!("if [ -d \"$HOME/{repo_name}\" ]; then"),
        format!("  cd $HOME/{repo_name} && git pull"),
        "else".to_owned(),
        format!("  cd $HOME && git clone {clone_url} {repo_name}"),
        "fi".to_owned(),
        format!("cd $HOME/{repo_name}"),
    ];
    for step in &deploy.steps {
        script_lines.push(step.clone());
    }
    script_lines.push("echo 'Deploy complete'".to_owned());

    let script = script_lines.join("\n");
    let provisioner = Provisioner::new(false, false);
    provisioner
        .provision(ip, &server.name, &script)
        .await
        .context("deploy failed")?;

    output::success(&format!("Deployed to {}", server.name));
    Ok(())
}
