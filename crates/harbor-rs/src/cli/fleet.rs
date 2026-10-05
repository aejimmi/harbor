use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, bail};

use super::FleetAction;
use super::output::{self, DeployResult, DeployStatus};
use crate::config::{
    FleetConfig, FleetServer, ServerSpec, SetupConfig, UserConfig, expand_servers,
};
use crate::dns::{self, DnsProvider};
use crate::provider::{CloudProvider, Server, ServerStatus};
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
        // `--debug` is accepted for symmetry with `up`; teardown has no
        // provisioning output to stream.
        FleetAction::Down {
            name,
            file,
            debug: _,
            quiet,
        } => down(&name, &file, quiet, config_path).await,
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

    let provider: Arc<dyn CloudProvider> = Arc::new(super::remote::hetzner_provider(&user_config)?);

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
    let mut names = std::collections::HashMap::new();

    for server in servers {
        let server = server.clone();
        let ctx = Arc::clone(ctx);
        let name = server.name.clone();
        let handle = set.spawn(async move {
            let start = Instant::now();
            let result = up_single(&server, &ctx).await;
            make_result(&server.name, result, start.elapsed())
        });
        names.insert(handle.id(), name);
    }

    let mut results = Vec::new();
    while let Some(join_result) = set.join_next().await {
        match join_result {
            Ok(deploy_result) => results.push(deploy_result),
            Err(e) => results.push(DeployResult {
                name: names
                    .get(&e.id())
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_owned()),
                ip: None,
                status: DeployStatus::Failed(format!("task panicked: {e}")),
                duration: std::time::Duration::ZERO,
            }),
        }
    }
    results
}

/// Create and provision a single fleet server.
///
/// The setup script is built before the server is created so that a
/// config or credentials error never leaves a paid, unprovisioned server.
async fn up_single(fleet_server: &FleetServer, ctx: &FleetContext) -> Result<Server> {
    let harbor_yaml = fleet_server.role_dir.join("harbor.yaml");
    let setup_config = SetupConfig::load(&harbor_yaml).context("loading role harbor.yaml")?;

    let server_section = setup_config
        .server
        .as_ref()
        .context("role harbor.yaml missing 'server:' section")?;
    if !server_section.volumes.is_empty() {
        bail!(
            "role {}: server.volumes is not supported in fleets — one named volume \
             cannot back N servers; use `harbor up` for a single node",
            fleet_server.role_dir.display()
        );
    }

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

    let setup_script = build_setup_script(&setup_config, fleet_server, ctx)?;

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
    let Some(ip) = server.ip else {
        bail!("server '{}' created but no IP assigned", fleet_server.name);
    };

    upsert_dns(&fleet_server.name, ip, ctx).await?;

    let provisioner = Provisioner::new(ctx.debug, ctx.quiet);
    provisioner
        .provision(ip, &fleet_server.name, &setup_script, None)
        .await?;

    Ok(server)
}

/// Render the role's setup script, failing on missing credentials.
fn build_setup_script(
    setup_config: &SetupConfig,
    fleet_server: &FleetServer,
    ctx: &FleetContext,
) -> Result<String> {
    crate::config::require_backup_creds(setup_config, &ctx.user_config)?;
    let github_token = ctx.user_config.github.token_for(&setup_config.name);
    let backup_creds = ctx.user_config.backup.for_project(&setup_config.name);
    Ok(ScriptBuilder::from_setup_config(
        setup_config,
        github_token,
        fleet_server.role_dir.as_path(),
        backup_creds,
    )
    .context("building setup script")?
    .build())
}

/// Point the fleet server's DNS record at `ip`. DNS failures are
/// reported but don't fail the server.
async fn upsert_dns(name: &str, ip: std::net::IpAddr, ctx: &FleetContext) -> Result<()> {
    if !dns::is_configured(&ctx.user_config) {
        return Ok(());
    }
    let Some(dns_provider) = dns::cloudflare::CloudflareProvider::from_config(&ctx.user_config)?
    else {
        return Ok(());
    };
    let full = dns::full_hostname(
        dns::extract_hostname(name),
        &ctx.user_config.dns.base_domain,
    );
    if !ctx.quiet {
        output::info(&format!("Creating DNS: {full} -> {ip}"));
    }
    if let Err(e) = dns_provider.upsert_a_record(&full, ip).await {
        output::error(&format!("DNS failed: {e}"));
    }
    Ok(())
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

    let provider = super::remote::hetzner_provider(&user_config)?;

    output::header("Fleet Down");
    output::info(&format!("Fleet: {fleet_name}"));
    output::info(&format!("{} servers to destroy", servers.len()));

    // Keep going past individual failures so one API error doesn't leave
    // the fleet half torn down without a summary.
    let mut failed = Vec::new();
    for fleet_server in &servers {
        if let Err(e) = down_single(&fleet_server.name, &provider, &user_config, quiet).await {
            output::error(&format!("{}: {e:#}", fleet_server.name));
            failed.push(fleet_server.name.as_str());
        }
    }

    if !failed.is_empty() {
        bail!(
            "failed to destroy {} server(s): {}",
            failed.len(),
            failed.join(", ")
        );
    }
    output::success("Fleet destroyed");
    Ok(())
}

/// Delete one fleet server, its pinned host key, and its DNS record.
async fn down_single(
    name: &str,
    provider: &crate::provider::hetzner::HetznerProvider,
    user_config: &UserConfig,
    quiet: bool,
) -> Result<()> {
    if !quiet {
        output::info(&format!("Deleting server: {name}"));
    }
    let Some(existing) = provider.get_server(name).await? else {
        if !quiet {
            output::subtle(&format!("  {name} not found, skipping"));
        }
        return Ok(());
    };

    provider.delete_server(name).await?;
    if !quiet {
        output::success(&format!("Deleted: {name}"));
    }
    if let Some(ip) = existing.ip {
        provision::remove_from_known_hosts(ip);
    }

    if !dns::is_configured(user_config) {
        return Ok(());
    }
    let full = dns::full_hostname(dns::extract_hostname(name), &user_config.dns.base_domain);
    if let Some(dns_provider) = dns::cloudflare::CloudflareProvider::from_config(user_config)?
        && let Err(e) = dns_provider.delete_a_record(&full).await
    {
        output::error(&format!("DNS cleanup failed for {full}: {e}"));
    }
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

    let provider = super::remote::hetzner_provider(&user_config)?;

    output::header("Fleet Status");
    output::info(&format!("Fleet: {fleet_name}"));
    eprintln!();

    print_status_row(["Server", "Role", "Status", "IP", "Type", "Location"]);
    eprintln!(
        "{}",
        "-".repeat(STATUS_WIDTHS.iter().sum::<usize>() + STATUS_WIDTHS.len())
    );

    let mut running = 0u32;
    let total = servers.len();
    for fleet_server in &servers {
        let server = provider.get_server(&fleet_server.name).await?;
        if server
            .as_ref()
            .is_some_and(|s| s.status == ServerStatus::Running)
        {
            running += 1;
        }
        let [status, ip, server_type, location] = status_columns(server.as_ref());
        print_status_row([
            &fleet_server.name,
            &fleet_server.role,
            &status,
            &ip,
            &server_type,
            &location,
        ]);
    }

    eprintln!();
    output::info(&format!("{running}/{total} running"));

    Ok(())
}

/// Column widths for `fleet status`: server, role, status, IP, type, location.
const STATUS_WIDTHS: [usize; 6] = [30, 15, 12, 16, 10, 8];

fn print_status_row(cells: [&str; 6]) {
    let row: Vec<String> = cells
        .iter()
        .zip(STATUS_WIDTHS)
        .map(|(cell, w)| format!("{cell:<w$}"))
        .collect();
    eprintln!("{}", row.join(" "));
}

/// Status, IP, type, and location cells for a server (or "not found").
fn status_columns(server: Option<&Server>) -> [String; 4] {
    let Some(s) = server else {
        return [
            "not found".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
            "-".to_owned(),
        ];
    };
    [
        format!("{:?}", s.status),
        s.ip.map_or_else(|| "-".to_owned(), |ip| ip.to_string()),
        s.server_type.clone(),
        s.location.clone(),
    ]
}
