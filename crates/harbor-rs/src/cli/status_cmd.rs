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
            let ip_str = s.ip.map_or("-".to_owned(), |ip| ip.to_string());
            output::header(&server.name);
            output::info(&format!("Status:   {status}"));
            output::info(&format!("IP:       {ip_str}"));
            output::info(&format!(
                "Type:     {}, Location: {}",
                s.server_type, s.location
            ));

            // Fetch app state from the server
            if let Some(ip) = s.ip
                && status == "running"
            {
                let services: Vec<&str> = setup_config
                    .setup
                    .services
                    .iter()
                    .map(|svc| svc.name.as_str())
                    .collect();

                fetch_app_state(ip, &services);
            }
        }
        None => {
            output::subtle(&format!("{} does not exist", server.name));
        }
    }

    Ok(())
}

/// SSH into the server to gather app state (deploy version, services, uptime, disk).
fn fetch_app_state(ip: std::net::IpAddr, services: &[&str]) {
    let script = build_status_script(services);

    let result = std::process::Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ConnectTimeout=5",
            &format!("root@{ip}"),
            &script,
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                if line.is_empty() {
                    continue;
                }
                output::info(line);
            }
        }
        Ok(_) => {
            output::subtle("  (could not fetch app state)");
        }
        Err(_) => {
            output::subtle("  (SSH unavailable)");
        }
    }
}

/// Build a bash snippet that gathers deploy + service info.
fn build_status_script(services: &[&str]) -> String {
    let mut parts = vec![
        // Last deploy
        r#"if [ -f ~/.harbor/deploys.log ]; then
  LAST=$(tail -n 1 ~/.harbor/deploys.log)
  echo "Deploy:   $LAST"
else
  echo "Deploy:   (no deploy history)"
fi"#
        .to_owned(),
        // Uptime
        r#"echo "Uptime:   $(uptime -p 2>/dev/null || uptime)""#.to_owned(),
        // Disk
        r#"echo "Disk:     $(df -h / | awk 'NR==2{print $3 "/" $2 " (" $5 " used)"}')"#.to_owned(),
    ];

    // Service statuses
    if !services.is_empty() {
        parts.push("echo ''".to_owned());
        parts.push("echo 'Services:'".to_owned());
        for svc in services {
            parts.push(format!(
                r#"STATUS=$(systemctl is-active {svc} 2>/dev/null || echo "not-found"); echo "  {svc}: $STATUS""#
            ));
        }
    }

    parts.join("\n")
}
