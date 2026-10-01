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
    let provider = super::remote::hetzner_provider(&user_config)?;

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

                let backup_project = setup_config
                    .setup
                    .backup
                    .as_ref()
                    .map(|_| setup_config.name.as_str());
                fetch_app_state(ip, &services, backup_project);
            }
        }
        None => {
            output::subtle(&format!("{} does not exist", server.name));
        }
    }

    Ok(())
}

/// SSH into the server to gather app state (deploy version, services, uptime, disk).
fn fetch_app_state(ip: std::net::IpAddr, services: &[&str], backup_project: Option<&str>) {
    let script = build_status_script(services, backup_project);
    let result = super::remote::ssh_exec_script(ip, &script);

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
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            output::subtle(&format!("  (could not fetch app state: {})", stderr.trim()));
        }
        Err(e) => {
            output::subtle(&format!("  (SSH unavailable: {e})"));
        }
    }
}

/// Build a bash snippet that gathers deploy + service info.
///
/// When `backup_project` is `Some`, append a terminal block that
/// queries the systemd timer state — adds three lines of output
/// without a second SSH round-trip (spec 017 R2).
pub(super) fn build_status_script(services: &[&str], backup_project: Option<&str>) -> String {
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
        // Disk. Raw string so the nested `"` quoting reaches bash
        // verbatim — an unterminated quote aborts the whole status probe.
        r#"echo "Disk:     $(df -h / | awk 'NR==2{print $3 "/" $2 " (" $5 " used)"}')""#.to_owned(),
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

    // Backup timer state (spec 017 R1). Terminal block — appended
    // after services so the existing output order is preserved.
    if let Some(project) = backup_project {
        parts.push("echo ''".to_owned());
        parts.push("echo 'Backup:'".to_owned());
        parts.push(format!(
            r#"TIMER=harbor-backup-{project}.timer
if systemctl is-enabled "$TIMER" >/dev/null 2>&1; then
  LAST=$(systemctl show "$TIMER" -p LastTriggerUSec --value)
  NEXT=$(systemctl show "$TIMER" -p NextElapseUSecRealtime --value)
  STATE=$(systemctl show "$TIMER" -p ActiveState --value)
  echo "  state: $STATE"
  echo "  last:  ${{LAST:-never}}"
  echo "  next:  ${{NEXT:-unknown}}"
else
  echo '  (backup timer not enabled on server)'
fi"#
        ));
    }

    parts.join("\n")
}
