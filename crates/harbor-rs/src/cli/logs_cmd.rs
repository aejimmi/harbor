use std::path::Path;

use anyhow::{Context, Result};

use super::{output, remote};

pub async fn run(service: Option<&str>, config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    let ip = server.ip;

    let journal_cmd = match service {
        Some(svc) => format!("journalctl -u {svc} -f"),
        None => "journalctl -f".to_owned(),
    };

    output::info(&format!("Streaming logs from {} ({})", server.name, ip));

    let status = remote::ssh_command(ip)
        .arg(&journal_cmd)
        .status()
        .context("failed to launch ssh")?;

    std::process::exit(status.code().unwrap_or(1));
}
