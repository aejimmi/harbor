use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};

use super::EnvAction;
use super::output::{self, DeployResult, DeployStatus};
use crate::config::{DeployConfig, ServerSpec, UserConfig};
use crate::dns::{self, DnsProvider};
use crate::provider::{CloudProvider, Server};
use crate::provision::{self, Provisioner};
use crate::script::ScriptBuilder;

/// Shared context for a deployment operation.
struct DeployContext {
    provider: Arc<dyn CloudProvider>,
    setup_script: String,
    ssh_key: String,
    user_config: UserConfig,
    debug: bool,
    quiet: bool,
}

pub async fn run(action: EnvAction, config_path: Option<&Path>) -> Result<()> {
    match action {
        EnvAction::Deploy {
            config_file,
            sequential,
            debug,
            quiet,
        } => deploy(&config_file, sequential, debug, quiet, config_path).await,
        EnvAction::Destroy {
            config_file,
            debug,
            quiet,
        } => destroy(&config_file, debug, quiet, config_path).await,
        EnvAction::List => {
            output::info("This feature is coming soon!");
            output::subtle("For now, use 'harbor server list' to see running servers");
            Ok(())
        }
    }
}

async fn deploy(
    config_file: &Path,
    sequential: bool,
    debug: bool,
    quiet: bool,
    user_config_path: Option<&Path>,
) -> Result<()> {
    let user_config = UserConfig::load(user_config_path).context("loading user config")?;
    let mut deploy_config = DeployConfig::load(config_file).context("loading deploy config")?;
    deploy_config.resolve_token(Some(&user_config));

    anyhow::ensure!(
        !deploy_config.hcloud.token.is_empty(),
        "no Hetzner Cloud token provided"
    );

    let setup_path = crate::config::default_server_config_path()?;
    let setup_config =
        crate::config::SetupConfig::load(&setup_path).context("loading setup config")?;
    let config_dir = setup_path.parent().unwrap_or(std::path::Path::new("."));
    let setup_script = ScriptBuilder::from_setup_config(
        &setup_config,
        user_config.github.token_for(&setup_config.name),
        config_dir,
    )
    .build();

    let provider: Arc<dyn CloudProvider> = Arc::new(
        crate::provider::hetzner::HetznerProvider::new(&deploy_config.hcloud.token),
    );

    let ctx = Arc::new(DeployContext {
        provider,
        setup_script,
        ssh_key: deploy_config.hcloud.ssh_key.clone(),
        user_config,
        debug,
        quiet,
    });

    output::header("Starting Deployment");
    output::info(&format!(
        "{} servers to deploy",
        deploy_config.servers.len()
    ));
    if sequential {
        output::info("Running in sequential mode");
    } else {
        output::info("Running in concurrent mode");
    }

    let results = if sequential {
        deploy_sequential(&deploy_config.servers, &ctx).await
    } else {
        deploy_concurrent(&deploy_config.servers, &ctx).await
    };

    output::deployment_summary(&results);
    Ok(())
}

async fn deploy_sequential(servers: &[ServerSpec], ctx: &Arc<DeployContext>) -> Vec<DeployResult> {
    let mut results = Vec::new();
    for spec in servers {
        let start = Instant::now();
        let result = deploy_single_server(spec, ctx).await;
        results.push(make_result(&spec.name, result, start.elapsed()));
    }
    results
}

async fn deploy_concurrent(servers: &[ServerSpec], ctx: &Arc<DeployContext>) -> Vec<DeployResult> {
    let mut set = tokio::task::JoinSet::new();

    for spec in servers {
        let spec = spec.clone();
        let ctx = Arc::clone(ctx);
        set.spawn(async move {
            let start = Instant::now();
            let result = deploy_single_server(&spec, &ctx).await;
            make_result(&spec.name, result, start.elapsed())
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

async fn deploy_single_server(spec: &ServerSpec, ctx: &DeployContext) -> Result<Server> {
    let server = ctx.provider.create_server(spec, &ctx.ssh_key).await?;

    if let Some(ip) = server.ip {
        // DNS
        if dns::is_configured(&ctx.user_config) {
            let hostname = dns::extract_hostname(&spec.name);
            let full = dns::full_hostname(hostname, &ctx.user_config.dns.base_domain);
            if let Some(dns_provider) =
                dns::cloudflare::CloudflareProvider::from_config(&ctx.user_config)?
            {
                if !ctx.quiet {
                    output::info(&format!("Creating DNS: {full} → {ip}"));
                }
                if let Err(e) = dns_provider.upsert_a_record(&full, ip).await {
                    output::error(&format!("DNS failed: {e}"));
                }
            }
        }

        // Provision
        let provisioner = Provisioner::new(ctx.debug, ctx.quiet);
        provisioner
            .provision(ip, &spec.name, &ctx.setup_script, None)
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

async fn destroy(
    config_file: &Path,
    _debug: bool,
    quiet: bool,
    user_config_path: Option<&Path>,
) -> Result<()> {
    let user_config = UserConfig::load(user_config_path).context("loading user config")?;
    let mut deploy_config = DeployConfig::load(config_file).context("loading deploy config")?;
    deploy_config.resolve_token(Some(&user_config));

    anyhow::ensure!(
        !deploy_config.hcloud.token.is_empty(),
        "no Hetzner Cloud token provided"
    );

    let provider = crate::provider::hetzner::HetznerProvider::new(&deploy_config.hcloud.token);

    output::header("Destroying servers");

    for spec in &deploy_config.servers {
        if !quiet {
            output::info(&format!("Deleting server: {}", spec.name));
        }

        let server = provider.get_server(&spec.name).await?;
        provider.delete_server(&spec.name).await?;

        if !quiet {
            output::success(&format!("Deleted: {}", spec.name));
        }

        if let Some(s) = &server
            && let Some(ip) = s.ip
        {
            provision::remove_from_known_hosts(ip);
        }

        if dns::is_configured(&user_config) {
            let h = dns::extract_hostname(&spec.name);
            let full = dns::full_hostname(h, &user_config.dns.base_domain);
            if let Some(dns_provider) =
                dns::cloudflare::CloudflareProvider::from_config(&user_config)?
            {
                let _ = dns_provider.delete_a_record(&full).await;
            }
        }
    }

    Ok(())
}
