use std::path::Path;

use anyhow::{Context, Result};

use super::{discover, output};
use crate::config::UserConfig;
use crate::provider::{CloudProvider, ServerStatus};

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    let (setup_config, _) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);

    match provider.get_server(&server.name).await? {
        Some(s) => {
            let status = match s.status {
                ServerStatus::Running => "running",
                ServerStatus::Off => "off",
                ServerStatus::Initializing => "initializing",
                _ => "unknown",
            };
            let ip = s.ip.map_or("-".to_owned(), |ip| ip.to_string());
            output::header(&server.name);
            output::info(&format!("Status:   {status}"));
            output::info(&format!("IP:       {ip}"));
            output::info(&format!(
                "Type:     {}, Location: {}",
                s.server_type, s.location
            ));
        }
        None => {
            output::subtle(&format!("{} does not exist", server.name));
        }
    }

    Ok(())
}
