//! Shared helpers for commands that operate on a remote server.

use std::io::Write;
use std::net::IpAddr;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result};

use super::discover;
use crate::config::{SetupConfig, UserConfig};
use crate::provider::CloudProvider;
use crate::provider::hetzner::HetznerProvider;

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
    let provider = hetzner_provider(&user_config)?;

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

/// Hetzner provider from the user config token, falling back to the
/// `HCLOUD_TOKEN` env var. Fails up front instead of with an API 401.
pub fn hetzner_provider(user_config: &UserConfig) -> Result<HetznerProvider> {
    if !user_config.hetzner.token.is_empty() {
        return Ok(HetznerProvider::new(&user_config.hetzner.token));
    }
    match std::env::var("HCLOUD_TOKEN") {
        Ok(token) if !token.is_empty() => Ok(HetznerProvider::new(&token)),
        _ => anyhow::bail!(
            "no Hetzner token — set hetzner.token in ~/.harbor/config.yaml or HCLOUD_TOKEN"
        ),
    }
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

/// Path to harbor's known_hosts file as a string, for passing to the
/// `ssh` binary via `-o UserKnownHostsFile=`.
pub fn harbor_known_hosts_path() -> String {
    crate::config::harbor_dir().map_or_else(
        |_| "~/.harbor/known_hosts".to_owned(),
        |d| d.join("known_hosts").to_string_lossy().into_owned(),
    )
}

/// Base `ssh root@<ip>` command with harbor's host-key pinning options.
/// Callers append the remote command (if any) and choose stdio.
pub fn ssh_command(ip: IpAddr) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-o",
        &format!("UserKnownHostsFile={}", harbor_known_hosts_path()),
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "HashKnownHosts=no",
        "-o",
        "ConnectTimeout=5",
        &format!("root@{ip}"),
    ]);
    cmd
}

/// Run a bash script on a remote host via the system `ssh` binary.
///
/// Always invokes `bash -s` remotely and pipes the script to its
/// stdin — bypassing the remote user's login shell. Without this,
/// a root user with fish or zsh as their login shell breaks on the
/// bash-only syntax harbor renders (process substitution, heredocs,
/// `${var:+...}` expansion). Mirrors the fix in `provision/ssh.rs`
/// for the russh path; every ssh-based command in the CLI should
/// go through this helper.
///
/// Stdin is written from a separate thread while output is drained,
/// so a chatty remote can't deadlock against a full pipe buffer.
pub fn ssh_exec_script(ip: IpAddr, script: &str) -> std::io::Result<Output> {
    let mut child = ssh_command(ip)
        .arg("bash -s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin = child.stdin.take();
    std::thread::scope(|scope| {
        let writer = scope.spawn(move || match stdin {
            Some(mut stdin) => stdin.write_all(script.as_bytes()),
            None => Ok(()),
        });
        let output = child.wait_with_output()?;
        writer
            .join()
            .map_err(|_| std::io::Error::other("ssh stdin writer panicked"))??;
        Ok(output)
    })
}
