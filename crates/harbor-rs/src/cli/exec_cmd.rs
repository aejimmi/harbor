use std::path::Path;

use anyhow::{Context, Result};

use super::{output, remote};

/// Run a single command on the server via SSH.
pub async fn run(command: &[String], config_path: Option<&Path>) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;

    let cmd = command.join(" ");
    output::info(&format!("{} ({}) > {}", server.name, server.ip, cmd));

    let status = remote::ssh_command(server.ip)
        .arg(&cmd)
        .status()
        .context("failed to launch ssh")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
