use std::path::Path;

use anyhow::{Context, Result};

use super::{discover, output};
use crate::config::UserConfig;
use crate::provider::CloudProvider;

pub async fn run(service: Option<&str>, config_path: Option<&Path>) -> Result<()> {
    let (setup_config, _) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);

    let existing = provider
        .get_server(&server.name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("server '{}' not found", server.name))?;
    let ip = existing
        .ip
        .ok_or_else(|| anyhow::anyhow!("server '{}' has no IP", server.name))?;

    let journal_cmd = match service {
        Some(svc) => format!("journalctl -u {svc} -f"),
        None => "journalctl -f".to_owned(),
    };

    output::info(&format!("Streaming logs from {} ({})", server.name, ip));

    let status = std::process::Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=accept-new",
            &format!("root@{ip}"),
            &journal_cmd,
        ])
        .status()
        .context("failed to launch ssh")?;

    std::process::exit(status.code().unwrap_or(1));
}
