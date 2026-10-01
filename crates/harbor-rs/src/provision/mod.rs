pub(crate) mod known_hosts;
pub(crate) mod output;
mod spinner;
mod ssh;

pub use spinner::Spinner;

#[cfg(test)]
mod known_hosts_test;
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

    #[error(
        "host key mismatch for {ip} — the server's key has changed since \
         last connection (possible MITM attack, or server was recreated); \
         run `harbor down` and `harbor up` to reset"
    )]
    HostKeyMismatch { ip: IpAddr },

    #[error("no SSH keys loaded in ssh-agent — run `ssh-add`")]
    NoSshKeys,

    #[error("cannot reach ssh-agent — is SSH_AUTH_SOCK set?")]
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
    ///
    /// Host key verification follows `accept-new` semantics: if a key
    /// for this IP exists in `~/.harbor/known_hosts` it must match the
    /// server's key, otherwise the connection is rejected. If no key
    /// exists the server's key is accepted and saved for future
    /// connections.
    ///
    /// If `spinner` is provided, status lines from the script update the spinner.
    /// If `None`, output is silent (or raw in debug mode).
    pub async fn provision(
        &self,
        ip: IpAddr,
        server_name: &str,
        script: &str,
        spinner: Option<&Spinner>,
    ) -> Result<(), ProvisionError> {
        let expected_key = known_hosts::lookup(ip)
            .map_err(|e| ProvisionError::Ssh(e.context("reading pinned host keys")))?;

        let (handle, accepted_key) = ssh::connect_with_retry(
            ip,
            server_name,
            expected_key.as_deref(),
            self.debug,
            self.quiet,
        )
        .await?;

        // Persist the accepted key so future connections can verify it.
        if let Err(e) = known_hosts::save(ip, &accepted_key) {
            tracing::warn!(ip = %ip, error = %e, "failed to save host key");
        }

        let wrapped_script = format!(
            "#!/bin/bash\n\
             exec 1> >(tee -a /var/log/setup-{server_name}.log)\n\
             exec 2> >(tee -a /var/log/setup-{server_name}.log >&2)\n\n\
             # Original script follows\n\
             {script}"
        );

        ssh::execute_script(
            handle,
            server_name,
            &wrapped_script,
            spinner,
            self.debug,
            self.quiet,
        )
        .await
    }
}

/// Remove an IP from both `~/.ssh/known_hosts` and `~/.harbor/known_hosts`.
///
/// Runs `ssh-keygen -R <ip>` for the system file. Non-fatal — logs a
/// warning on failure.
pub fn remove_from_known_hosts(ip: IpAddr) {
    // Remove from system known_hosts.
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

    // Remove from harbor's own known_hosts.
    if let Err(e) = known_hosts::remove(ip) {
        tracing::warn!(ip = %ip, error = %e, "failed to remove pinned host key");
    }
}
