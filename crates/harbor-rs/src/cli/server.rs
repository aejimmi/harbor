use std::path::Path;

use anyhow::{Context, Result};

use super::ServerAction;
use super::output;
use crate::config::{self, UserConfig};
use crate::dns::{self, DnsProvider};
use crate::provider::{CloudProvider, ServerStatus};
use crate::provision::{self, Provisioner};
use crate::script::ScriptBuilder;

pub async fn run(action: ServerAction, config_path: Option<&Path>) -> Result<()> {
    match action {
        ServerAction::Create { .. } => create(action, config_path).await,
        ServerAction::Delete {
            name,
            hostname,
            quiet,
            ..
        } => delete(&name, hostname.as_deref(), quiet, config_path).await,
        ServerAction::List => list(config_path).await,
    }
}

async fn create(action: ServerAction, config_path: Option<&Path>) -> Result<()> {
    let ServerAction::Create {
        name,
        ssh_key,
        r#type: server_type,
        location,
        image,
        hostname,
        setup_config: setup_config_path,
        debug,
        quiet,
    } = action
    else {
        unreachable!()
    };

    let user_config = UserConfig::load(config_path).context("loading user config")?;

    anyhow::ensure!(
        !user_config.hetzner.token.is_empty(),
        "hetzner.token is required in config file"
    );

    let setup_path = match setup_config_path {
        Some(p) => p,
        None => config::default_server_config_path()?,
    };
    let setup_config = config::SetupConfig::load(&setup_path).context("loading setup config")?;
    let config_dir = setup_path.parent().unwrap_or(Path::new("."));
    let setup_script = ScriptBuilder::from_setup_config(
        &setup_config,
        user_config.github.token_for(&setup_config.name),
        config_dir,
    )
    .build();

    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);
    let spec = config::ServerSpec {
        name: name.clone(),
        server_type: server_type.clone(),
        location: location.clone(),
        image,
    };

    if !quiet {
        output::header(&format!("Provisioning server: {name}"));
        output::info(&format!(
            "Type: {server_type}, Location: {location}, Image: {}",
            spec.image
        ));
    }

    let server = provider
        .create_server(&spec, &ssh_key)
        .await
        .context("creating server")?;

    if let Some(ip) = server.ip {
        if let Some(h) = &hostname
            && dns::is_configured(&user_config)
        {
            let full = dns::full_hostname(h, &user_config.dns.base_domain);
            if let Some(dns_provider) =
                dns::cloudflare::CloudflareProvider::from_config(&user_config)?
            {
                if !quiet {
                    output::info(&format!("Creating DNS: {full} → {ip}"));
                }
                dns_provider.upsert_a_record(&full, ip).await?;
                if !quiet {
                    output::success(&format!("DNS record created: {full}"));
                }
            }
        }

        let provisioner = Provisioner::new(debug, quiet);
        provisioner
            .provision(ip, &name, &setup_script, None)
            .await
            .context("provisioning server")?;
    }

    if !quiet {
        output::success(&format!("Server {name} provisioned successfully!"));
    }

    Ok(())
}

async fn delete(
    name: &str,
    hostname: Option<&str>,
    quiet: bool,
    config_path: Option<&Path>,
) -> Result<()> {
    let user_config = UserConfig::load(config_path).context("loading user config")?;

    anyhow::ensure!(
        !user_config.hetzner.token.is_empty(),
        "hetzner.token is required in config file"
    );

    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);
    let server = provider.get_server(name).await?;

    if !quiet {
        output::header(&format!("Deleting server: {name}"));
    }

    provider.delete_server(name).await?;

    if !quiet {
        output::success(&format!("Server {name} deleted"));
    }

    if let Some(s) = &server
        && let Some(ip) = s.ip
    {
        provision::remove_from_known_hosts(ip);
    }

    if dns::is_configured(&user_config) {
        let h = hostname.unwrap_or_else(|| dns::extract_hostname(name));
        let full = dns::full_hostname(h, &user_config.dns.base_domain);

        if let Some(dns_provider) = dns::cloudflare::CloudflareProvider::from_config(&user_config)?
        {
            if !quiet {
                output::info(&format!("Deleting DNS: {full}"));
            }
            match dns_provider.delete_a_record(&full).await {
                Ok(()) if !quiet => {
                    output::success(&format!("DNS record deleted: {full}"));
                }
                Err(e) => output::error(&format!("Failed to delete DNS: {e}")),
                _ => {}
            }
        }
    }

    Ok(())
}

async fn list(config_path: Option<&Path>) -> Result<()> {
    let user_config = UserConfig::load(config_path).context("loading user config")?;

    anyhow::ensure!(
        !user_config.hetzner.token.is_empty(),
        "hetzner.token is required in config file"
    );

    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);
    let servers = provider.list_servers().await?;

    if servers.is_empty() {
        output::subtle("No servers found");
        return Ok(());
    }

    output::header("Running Servers");
    eprintln!();

    for s in &servers {
        let status_str = match s.status {
            ServerStatus::Running => "running",
            ServerStatus::Off => "off",
            ServerStatus::Initializing => "initializing",
            _ => "other",
        };
        let ip_str = s.ip.map_or("-".to_owned(), |ip| ip.to_string());
        output::info(&format!(
            "{} ({}, {}) — {} [{}]",
            s.name, s.server_type, s.location, status_str, ip_str
        ));
    }

    output::subtle(&format!("\nTotal: {} server(s)", servers.len()));
    Ok(())
}
