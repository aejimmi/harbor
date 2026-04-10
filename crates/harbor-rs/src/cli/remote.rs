//! Shared helpers for commands that operate on a remote server.

use std::net::IpAddr;
use std::path::Path;

use anyhow::{Context, Result};

use super::discover;
use crate::config::{SetupConfig, UserConfig};
use crate::provider::CloudProvider;

/// Resolved server: config + IP + name.
pub struct ResolvedServer {
    pub config: SetupConfig,
    pub ip: IpAddr,
    pub name: String,
}

/// Load project config, look up the server in Hetzner, return IP + name.
pub async fn resolve_server(config_path: Option<&Path>) -> Result<ResolvedServer> {
    let (setup_config, _) = discover::load_project_config()?;
    let server = setup_config
        .server
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no 'server:' section in harbor.yaml"))?;

    let user_config = UserConfig::load(config_path).context("loading user config")?;
    let provider = crate::provider::hetzner::HetznerProvider::new(&user_config.hetzner.token);

    let existing = provider.get_server(&server.name).await?.ok_or_else(|| {
        anyhow::anyhow!("server '{}' not found — run `harbor up` first", server.name)
    })?;
    let ip = existing
        .ip
        .ok_or_else(|| anyhow::anyhow!("server '{}' has no IP", server.name))?;

    let name = server.name.clone();
    Ok(ResolvedServer {
        config: setup_config,
        ip,
        name,
    })
}

/// Generate the bash preamble that acquires a deploy lock with stale detection.
///
/// Uses `mkdir` for atomicity. If an existing lock is older than 30 minutes,
/// it is assumed stale and automatically removed.
pub fn lock_preamble() -> Vec<String> {
    vec![
        "mkdir -p ~/.harbor".to_owned(),
        // Stale lock detection: if lock dir is older than 30 min, remove it
        "if [ -d ~/.harbor/deploy.lock ]; then".to_owned(),
        "  LOCK_AGE=$(( $(date +%s) - $(stat -c %Y ~/.harbor/deploy.lock 2>/dev/null || echo 0) ))"
            .to_owned(),
        "  if [ \"$LOCK_AGE\" -gt 1800 ]; then".to_owned(),
        "    echo 'Removing stale deploy lock (>30 min old)'".to_owned(),
        "    rm -rf ~/.harbor/deploy.lock".to_owned(),
        "  else".to_owned(),
        "    echo 'Deploy already in progress:'".to_owned(),
        "    cat ~/.harbor/deploy.lock/info 2>/dev/null || true".to_owned(),
        "    exit 1".to_owned(),
        "  fi".to_owned(),
        "fi".to_owned(),
        "mkdir ~/.harbor/deploy.lock".to_owned(),
        "trap 'rm -rf ~/.harbor/deploy.lock' EXIT".to_owned(),
        "echo \"$(date -u +%Y-%m-%dT%H:%M:%SZ) $(whoami)\" > ~/.harbor/deploy.lock/info".to_owned(),
    ]
}

/// Generate bash lines that check whether each service is healthy.
pub fn health_check_lines(services: &[&str]) -> Vec<String> {
    if services.is_empty() {
        return Vec::new();
    }

    let mut lines = vec![
        String::new(),
        "echo 'Checking service health...'".to_owned(),
    ];

    for svc in services {
        lines.push("sleep 2".to_owned());
        lines.push(format!("if systemctl is-active --quiet {svc}; then"));
        lines.push(format!("  echo 'Service {svc}: healthy'"));
        lines.push("else".to_owned());
        lines.push(format!("  echo 'Service {svc}: UNHEALTHY' >&2"));
        lines.push(format!("  journalctl -u {svc} --no-pager -n 20 >&2"));
        lines.push("  exit 1".to_owned());
        lines.push("fi".to_owned());
    }

    lines
}

/// Collect service names that have `start: true` in config.
pub fn started_services(config: &SetupConfig) -> Vec<String> {
    config
        .setup
        .services
        .iter()
        .filter(|s| s.start)
        .map(|s| s.name.clone())
        .collect()
}
