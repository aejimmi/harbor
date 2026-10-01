use std::path::Path;

use anyhow::{Context, Result};

use super::{deploy_cmd, discover, output};
use crate::config::{self, UserConfig};
use crate::dns::{self, DnsProvider};
use crate::provider::CloudProvider;
use crate::provision::{Provisioner, Spinner};
use crate::script::ScriptBuilder;

pub async fn run(debug: bool, config_path: Option<&Path>) -> Result<()> {
    let (setup_config, yaml_path) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    // Fail fast when `backup:` is declared without matching creds —
    // matches the existing `hetzner.token is required` pattern
    // (spec 014 R2).
    config::require_backup_creds(&setup_config, &user_config)?;

    let config_dir = yaml_path.parent().unwrap_or(Path::new("."));
    let backup_creds = user_config.backup.for_project(&setup_config.name);
    let setup_script = ScriptBuilder::from_setup_config(
        &setup_config,
        user_config.github.token_for(&setup_config.name),
        config_dir,
        backup_creds,
    )
    .context("building setup script")?
    .build();

    let provider = super::remote::hetzner_provider(&user_config)?;
    let spec = config::ServerSpec {
        name: server.name.clone(),
        server_type: server.r#type.clone(),
        location: server.location.clone(),
        image: server.image.clone(),
    };

    output::header(&server.name);
    output::info(&format!(
        "{} · {} · {}",
        server.r#type, server.location, spec.image
    ));

    let spinner = Spinner::start("Creating server on Hetzner...", debug);

    let created = match provider.create_server(&spec, &server.ssh_key).await {
        Ok(s) => s,
        Err(e) => {
            spinner.fail();
            return Err(e).context("creating server");
        }
    };

    let Some(ip) = created.ip else {
        spinner.fail();
        anyhow::bail!("server created but no IP assigned");
    };

    spinner.set_step(format!("Server ready at {ip}"));

    if let Some(ref h) = server.hostname
        && dns::is_configured(&user_config)
    {
        let full = dns::full_hostname(h, &user_config.dns.base_domain);
        if let Some(dns_provider) = dns::cloudflare::CloudflareProvider::from_config(&user_config)?
        {
            spinner.set_step(format!("Creating DNS: {full} → {ip}"));
            if let Err(e) = dns_provider.upsert_a_record(&full, ip).await {
                spinner.fail();
                return Err(e).context("creating DNS record");
            }
        }
    }

    spinner.set_step("Connecting via SSH...");

    let provisioner = Provisioner::new(debug, false);
    if let Err(e) = provisioner
        .provision(ip, &server.name, &setup_script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context("provisioning");
    }

    spinner.success(format!("{} is up ({ip})", server.name));

    // Provisioning is done; run every configured deploy sequentially (alphabetical,
    // fail-fast) so `harbor up` leaves a fully-deployed server. Empty `deploys:`
    // skips this step silently. Same path as `harbor deploy --all`, reusing the
    // config and IP already in hand instead of looking the server up again.
    if !setup_config.setup.deploys.is_empty() {
        let resolved = super::remote::ResolvedServer {
            name: server.name.clone(),
            ip,
            config: setup_config,
        };
        let names = deploy_cmd::sorted_names(&resolved.config);
        deploy_cmd::run_all(&resolved, &names, debug).await?;
    }

    Ok(())
}
