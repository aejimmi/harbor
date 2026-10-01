use std::path::Path;

use anyhow::{Context, Result};

use super::{output, remote};

pub async fn run(config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    let ip = server.ip;

    output::info(&format!("Connecting to {} ({})", server.name, ip));

    let status = remote::ssh_command(ip)
        .status()
        .context("failed to launch ssh")?;

    std::process::exit(status.code().unwrap_or(1));
}
