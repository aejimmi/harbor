pub(crate) mod output;
mod ssh;

#[cfg(test)]
mod output_test;

use std::net::IpAddr;

/// Errors from provisioning operations.
#[derive(Debug, thiserror::Error)]
pub enum ProvisionError {
    #[error("SSH connection to {ip} failed after {attempts} attempts: {source}")]
    ConnectionFailed {
        ip: IpAddr,
        attempts: u32,
        source: anyhow::Error,
    },

    #[allow(dead_code)]
    #[error("no SSH keys available in agent")]
    NoSshKeys,

    #[allow(dead_code)]
    #[error("SSH_AUTH_SOCK not set")]
    NoSshAgent,

    #[error("setup script failed on {server_name} with exit code {code}")]
    ScriptFailed { server_name: String, code: u32 },

    #[error("SSH error: {0}")]
    Ssh(#[from] anyhow::Error),
}

/// Provisions servers by executing scripts over SSH.
pub struct Provisioner {
    debug: bool,
    quiet: bool,
}

impl Provisioner {
    /// Create a new provisioner.
    pub fn new(debug: bool, quiet: bool) -> Self {
        Self { debug, quiet }
    }

    /// Connect to a server via SSH and execute the setup script.
    pub async fn provision(
        &self,
        ip: IpAddr,
        server_name: &str,
        script: &str,
    ) -> Result<(), ProvisionError> {
        let handle = ssh::connect_with_retry(ip, server_name, self.debug, self.quiet).await?;

        let wrapped_script = format!(
            "#!/bin/bash\n\
             exec 1> >(tee -a /var/log/setup-{server_name}.log)\n\
             exec 2> >(tee -a /var/log/setup-{server_name}.log >&2)\n\n\
             # Original script follows\n\
             {script}"
        );

        ssh::execute_script(handle, server_name, &wrapped_script, self.debug, self.quiet).await
    }
}

/// Remove an IP from `~/.ssh/known_hosts`.
///
/// Runs `ssh-keygen -R <ip>`. Non-fatal — logs a warning on failure.
pub fn remove_from_known_hosts(ip: IpAddr) {
    let status = std::process::Command::new("ssh-keygen")
        .args(["-R", &ip.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            tracing::info!(ip = %ip, "removed from known_hosts");
        }
        Ok(s) => {
            tracing::warn!(ip = %ip, code = ?s.code(), "ssh-keygen -R failed");
        }
        Err(e) => {
            tracing::warn!(ip = %ip, error = %e, "failed to run ssh-keygen");
        }
    }
}
