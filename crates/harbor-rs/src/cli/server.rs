use std::path::Path;

use anyhow::{Context, Result};

use super::output;
use super::{CreateArgs, ServerAction};
use crate::config::{self, UserConfig};
use crate::dns::{self, DnsProvider};
use crate::provider::{CloudProvider, ServerStatus};
use crate::provision::{self, Provisioner};
use crate::script::ScriptBuilder;

pub async fn run(action: ServerAction, config_path: Option<&Path>) -> Result<()> {
    match action {
        ServerAction::Create(args) => create(args, config_path).await,
        ServerAction::Delete {
            name,
            hostname,
            quiet,
            ..
        } => delete(&name, hostname.as_deref(), quiet, config_path).await,
        ServerAction::List => list(config_path).await,
    }
}

async fn create(args: CreateArgs, config_path: Option<&Path>) -> Result<()> {
    let user_config = UserConfig::load(config_path).context("loading user config")?;
    let setup_script = build_setup_script(args.setup_config.as_deref(), &user_config)?;

    let provider = super::remote::hetzner_provider(&user_config)?;
    let spec = config::ServerSpec {
        name: args.name.clone(),
        server_type: args.r#type.clone(),
        location: args.location.clone(),
        image: args.image.clone(),
    };
    if !args.quiet {
        output::header(&format!("Provisioning server: {}", spec.name));
        output::info(&format!(
            "Type: {}, Location: {}, Image: {}",
            spec.server_type, spec.location, spec.image
        ));
    }

    let server = provider
        .create_server(&spec, &args.ssh_key)
        .await
        .context("creating server")?;
    let Some(ip) = server.ip else {
        anyhow::bail!("server {} created but no IP assigned", spec.name);
    };
    if let Some(h) = &args.hostname {
        upsert_dns(h, ip, &user_config, args.quiet).await?;
    }

    Provisioner::new(args.debug, args.quiet)
        .provision(ip, &spec.name, &setup_script, None)
        .await
        .context("provisioning server")?;
    if !args.quiet {
        output::success(&format!("Server {} provisioned successfully!", spec.name));
    }
    Ok(())
}

/// Render the setup script from `--setup-config`, or the discovered
/// harbor.yaml. Runs before the server exists so config errors cost nothing.
fn build_setup_script(setup_path: Option<&Path>, user_config: &UserConfig) -> Result<String> {
    let setup_path = match setup_path {
        Some(p) => p.to_path_buf(),
        None => super::discover::find_config()?,
    };
    let setup_config = config::SetupConfig::load(&setup_path).context("loading setup config")?;
    config::require_backup_creds(&setup_config, user_config)?;
    let config_dir = setup_path.parent().unwrap_or(Path::new("."));
    Ok(ScriptBuilder::from_setup_config(
        &setup_config,
        user_config.github.token_for(&setup_config.name),
        config_dir,
        user_config.backup.for_project(&setup_config.name),
    )
    .context("building setup script")?
    .build())
}

/// Point `hostname` at `ip` when DNS is configured.
async fn upsert_dns(
    hostname: &str,
    ip: std::net::IpAddr,
    user_config: &UserConfig,
    quiet: bool,
) -> Result<()> {
    if !dns::is_configured(user_config) {
        return Ok(());
    }
    let Some(dns_provider) = dns::cloudflare::CloudflareProvider::from_config(user_config)? else {
        return Ok(());
    };
    let full = dns::full_hostname(hostname, &user_config.dns.base_domain);
    if !quiet {
        output::info(&format!("Creating DNS: {full} → {ip}"));
    }
    dns_provider.upsert_a_record(&full, ip).await?;
    if !quiet {
        output::success(&format!("DNS record created: {full}"));
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

    let provider = super::remote::hetzner_provider(&user_config)?;
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

    let provider = super::remote::hetzner_provider(&user_config)?;
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
