use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};

use super::FleetAction;
use super::output::{self, DeployResult, DeployStatus};
use crate::config::{
    FleetConfig, FleetServer, ServerSpec, SetupConfig, UserConfig, expand_servers,
};
use crate::dns::{self, DnsProvider};
use crate::provider::{CloudProvider, Server};
use crate::provision::{self, Provisioner};
use crate::script::ScriptBuilder;

/// Shared context for fleet operations.
struct FleetContext {
    provider: Arc<dyn CloudProvider>,
    user_config: UserConfig,
    debug: bool,
    quiet: bool,
}

/// Run a fleet subcommand.
pub async fn run(action: FleetAction, config_path: Option<&Path>) -> Result<()> {
    match action {
        FleetAction::Up {
            name,
            file,
            sequential,
            debug,
            quiet,
        } => up(&name, &file, sequential, debug, quiet, config_path).await,
        FleetAction::Down {
            name,
            file,
            debug,
            quiet,
        } => down(&name, &file, debug, quiet, config_path).await,
        FleetAction::Status { name, file } => status(&name, &file, config_path).await,
    }
}

async fn up(
    fleet_name: &str,
    fleet_file: &Path,
    sequential: bool,
    debug: bool,
    quiet: bool,
    user_config_path: Option<&Path>,
) -> Result<()> {
    let base_dir = fleet_file
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .context("resolving fleet config directory")?;

    let fleet_config = FleetConfig::load(fleet_file).context("loading fleet config")?;
    fleet_config
        .validate(&base_dir)
        .context("validating fleet config")?;

    let servers = expand_servers(&fleet_config, fleet_name, &base_dir);

    let user_config = UserConfig::load(user_config_path).context("loading user config")?;
    let token = resolve_token(&user_config)?;

    let provider: Arc<dyn CloudProvider> =
        Arc::new(crate::provider::hetzner::HetznerProvider::new(&token));

    let ctx = Arc::new(FleetContext {
        provider,
        user_config,
        debug,
        quiet,
    });

    output::header("Fleet Up");
    output::info(&format!("Fleet: {fleet_name}"));
    output::info(&format!("{} servers to create", servers.len()));
    if sequential {
        output::info("Running in sequential mode");
    }

    let results = if sequential {
        up_sequential(&servers, &ctx).await
    } else {
        up_concurrent(&servers, &ctx).await
    };

    output::deployment_summary(&results);
    Ok(())
}

/// Resolve the Hetzner token from user config or env var.
fn resolve_token(user_config: &UserConfig) -> Result<String> {
    if !user_config.hetzner.token.is_empty() {
        return Ok(user_config.hetzner.token.clone());
    }
    if let Ok(token) = std::env::var("HCLOUD_TOKEN") {
        return Ok(token);
    }
    anyhow::bail!("no Hetzner Cloud token found in config or HCLOUD_TOKEN env var")
}

async fn up_sequential(servers: &[FleetServer], ctx: &Arc<FleetContext>) -> Vec<DeployResult> {
    let mut results = Vec::new();
    for server in servers {
        let start = Instant::now();
        let result = up_single(server, ctx).await;
        results.push(make_result(&server.name, result, start.elapsed()));
    }
    results
}

async fn up_concurrent(servers: &[FleetServer], ctx: &Arc<FleetContext>) -> Vec<DeployResult> {
    let mut set = tokio::task::JoinSet::new();

    for server in servers {
        let server = server.clone();
        let ctx = Arc::clone(ctx);
        set.spawn(async move {
            let start = Instant::now();
            let result = up_single(&server, &ctx).await;
            make_result(&server.name, result, start.elapsed())
        });
    }

    let mut results = Vec::new();
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(deploy_result) => results.push(deploy_result),
            Err(e) => results.push(DeployResult {
                name: "unknown".to_owned(),
                ip: None,
                status: DeployStatus::Failed(format!("task panicked: {e}")),
                duration: std::time::Duration::ZERO,
            }),
        }
    }
    results
}

/// Create and provision a single fleet server.
async fn up_single(fleet_server: &FleetServer, ctx: &FleetContext) -> Result<Server> {
    let harbor_yaml = fleet_server.role_dir.join("harbor.yaml");
    let setup_config = SetupConfig::load(&harbor_yaml).context("loading role harbor.yaml")?;

    let server_section = setup_config
        .server
        .as_ref()
        .context("role harbor.yaml missing 'server:' section")?;

    // Check if server already exists (idempotent).
    if let Some(existing) = ctx.provider.get_server(&fleet_server.name).await? {
        if !ctx.quiet {
            output::info(&format!(
                "Server '{}' already exists, skipping",
                fleet_server.name
            ));
        }
        return Ok(existing);
    }

    let spec = ServerSpec {
        name: fleet_server.name.clone(),
        server_type: server_section.r#type.clone(),
        location: server_section.location.clone(),
        image: server_section.image.clone(),
    };

    let server = ctx
        .provider
        .create_server(&spec, &server_section.ssh_key)
        .await?;

    if let Some(ip) = server.ip {
        // DNS
        if dns::is_configured(&ctx.user_config) {
            let hostname = dns::extract_hostname(&fleet_server.name);
            let full = dns::full_hostname(hostname, &ctx.user_config.dns.base_domain);
            if let Some(dns_provider) =
                dns::cloudflare::CloudflareProvider::from_config(&ctx.user_config)?
            {
                if !ctx.quiet {
                    output::info(&format!("Creating DNS: {full} -> {ip}"));
                }
                if let Err(e) = dns_provider.upsert_a_record(&full, ip).await {
                    output::error(&format!("DNS failed: {e}"));
                }
            }
        }

        // Provision
        let config_dir = fleet_server.role_dir.as_path();
        let github_token = ctx.user_config.github.token_for(&setup_config.name);
        let setup_script =
            ScriptBuilder::from_setup_config(&setup_config, github_token, config_dir)
                .context("building setup script")?
                .build();

        let provisioner = Provisioner::new(ctx.debug, ctx.quiet);
        provisioner
            .provision(ip, &fleet_server.name, &setup_script, None)
            .await?;
    }

    Ok(server)
}

fn make_result(name: &str, result: Result<Server>, duration: std::time::Duration) -> DeployResult {
    match result {
        Ok(server) => DeployResult {
            name: name.to_owned(),
            ip: server.ip,
            status: DeployStatus::Success,
            duration,
        },
        Err(e) => DeployResult {
            name: name.to_owned(),
            ip: None,
            status: DeployStatus::Failed(format!("{e}")),
            duration,
        },
    }
}

async fn down(
    fleet_name: &str,
    fleet_file: &Path,
    _debug: bool,
    quiet: bool,
    user_config_path: Option<&Path>,
) -> Result<()> {
    let base_dir = fleet_file
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .context("resolving fleet config directory")?;

    let fleet_config = FleetConfig::load(fleet_file).context("loading fleet config")?;
    let servers = expand_servers(&fleet_config, fleet_name, &base_dir);

    let user_config = UserConfig::load(user_config_path).context("loading user config")?;
    let token = resolve_token(&user_config)?;

    let provider = crate::provider::hetzner::HetznerProvider::new(&token);

    output::header("Fleet Down");
    output::info(&format!("Fleet: {fleet_name}"));
    output::info(&format!("{} servers to destroy", servers.len()));

    for fleet_server in &servers {
        if !quiet {
            output::info(&format!("Deleting server: {}", fleet_server.name));
        }

        let existing = provider.get_server(&fleet_server.name).await?;
        if existing.is_none() {
            if !quiet {
                output::subtle(&format!("  {} not found, skipping", fleet_server.name));
            }
            continue;
        }

        provider.delete_server(&fleet_server.name).await?;

        if !quiet {
            output::success(&format!("Deleted: {}", fleet_server.name));
        }

        if let Some(s) = &existing
            && let Some(ip) = s.ip
        {
            provision::remove_from_known_hosts(ip);
        }

        if dns::is_configured(&user_config) {
            let h = dns::extract_hostname(&fleet_server.name);
            let full = dns::full_hostname(h, &user_config.dns.base_domain);
            if let Some(dns_provider) =
                dns::cloudflare::CloudflareProvider::from_config(&user_config)?
            {
                let _ = dns_provider.delete_a_record(&full).await;
            }
        }
    }

    output::success("Fleet destroyed");
    Ok(())
}

async fn status(
    fleet_name: &str,
    fleet_file: &Path,
    user_config_path: Option<&Path>,
) -> Result<()> {
    let base_dir = fleet_file
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .context("resolving fleet config directory")?;

    let fleet_config = FleetConfig::load(fleet_file).context("loading fleet config")?;
    let servers = expand_servers(&fleet_config, fleet_name, &base_dir);

    let user_config = UserConfig::load(user_config_path).context("loading user config")?;
    let token = resolve_token(&user_config)?;

    let provider = crate::provider::hetzner::HetznerProvider::new(&token);

    output::header("Fleet Status");
    output::info(&format!("Fleet: {fleet_name}"));
    eprintln!();

    let name_w = 30;
    let role_w = 15;
    let status_w = 12;
    let ip_w = 16;
    let type_w = 10;
    let loc_w = 8;

    eprintln!(
        "{:<name_w$} {:<role_w$} {:<status_w$} {:<ip_w$} {:<type_w$} {:<loc_w$}",
        "Server", "Role", "Status", "IP", "Type", "Location"
    );
    eprintln!(
        "{}",
        "-".repeat(name_w + role_w + status_w + ip_w + type_w + loc_w)
    );

    let mut running = 0u32;
    let total = servers.len();

    for fleet_server in &servers {
        let server = provider.get_server(&fleet_server.name).await?;

        let (status_str, ip_str, type_str, loc_str) = match &server {
            Some(s) => {
                let st = format!("{:?}", s.status);
                if st == "Running" {
                    running += 1;
                }
                (
                    st,
                    s.ip.map_or("-".to_owned(), |ip| ip.to_string()),
                    s.server_type.clone(),
                    s.location.clone(),
                )
            }
            None => (
                "not found".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
                "-".to_owned(),
            ),
        };

        eprintln!(
            "{:<name_w$} {:<role_w$} {:<status_w$} {:<ip_w$} {:<type_w$} {:<loc_w$}",
            fleet_server.name, fleet_server.role, status_str, ip_str, type_str, loc_str
        );
    }

    eprintln!();
    output::info(&format!("{running}/{total} running"));

    Ok(())
}
