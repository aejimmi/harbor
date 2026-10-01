use std::path::Path;

use anyhow::{Context, Result};

use super::{discover, output};
use crate::config::UserConfig;
use crate::dns::{self, DnsProvider};
use crate::provider::CloudProvider;
use crate::provision;

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    let (setup_config, _) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    let provider = super::remote::hetzner_provider(&user_config)?;

    output::header(&format!("Destroying {}", server.name));

    let existing = provider.get_server(&server.name).await?;
    provider.delete_server(&server.name).await?;

    if let Some(s) = &existing
        && let Some(ip) = s.ip
    {
        provision::remove_from_known_hosts(ip);
    }

    if dns::is_configured(&user_config)
        && let Some(ref h) = server.hostname
    {
        let full = dns::full_hostname(h, &user_config.dns.base_domain);
        if let Some(dns_provider) = dns::cloudflare::CloudflareProvider::from_config(&user_config)?
        {
            output::info(&format!("Removing DNS: {full}"));
            if let Err(e) = dns_provider.delete_a_record(&full).await {
                output::error(&format!("DNS cleanup failed for {full}: {e}"));
            }
        }
    }

    output::success(&format!("{} is down", server.name));
    Ok(())
}
