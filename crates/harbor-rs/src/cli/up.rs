use std::path::Path;

use anyhow::{Context, Result};

use super::{discover, output};
use crate::config::{self, UserConfig};
use crate::dns::{self, DnsProvider};
use crate::provider::CloudProvider;
use crate::provision::Provisioner;
use crate::script::ScriptBuilder;

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    let (setup_config, yaml_path) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    anyhow::ensure!(
        !user_config.hetzner.token.is_empty(),
        "hetzner.token is required"
    );

    let config_dir = yaml_path.parent().unwrap_or(Path::new("."));
    let setup_script =
        ScriptBuilder::from_setup_config(&setup_config, &user_config.github.token, config_dir)
            .build();

    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);
    let spec = config::ServerSpec {
        name: server.name.clone(),
        server_type: server.r#type.clone(),
        location: server.location.clone(),
        image: server.image.clone(),
    };

    output::header(&format!("Creating {}", server.name));
    output::info(&format!(
        "Type: {}, Location: {}",
        server.r#type, server.location
    ));

    let created = provider
        .create_server(&spec, &server.ssh_key)
        .await
        .context("creating server")?;

    if let Some(ip) = created.ip {
        if let Some(ref h) = server.hostname {
            if dns::is_configured(&user_config) {
                let full = dns::full_hostname(h, &user_config.dns.base_domain);
                if let Some(dns_provider) =
                    dns::cloudflare::CloudflareProvider::from_config(&user_config)?
                {
                    output::info(&format!("Creating DNS: {full} → {ip}"));
                    dns_provider.upsert_a_record(&full, ip).await?;
                }
            }
        }

        let provisioner = Provisioner::new(false, false);
        provisioner
            .provision(ip, &server.name, &setup_script)
            .await
            .context("provisioning")?;
    }

    output::success(&format!("{} is up", server.name));
    Ok(())
}
