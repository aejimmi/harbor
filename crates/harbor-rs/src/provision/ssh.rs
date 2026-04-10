use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use russh::ChannelMsg;
use russh::client;
use russh::keys::agent::client::AgentClient;

use super::ProvisionError;
use super::output::FilteredOutput;

const MAX_ATTEMPTS: u32 = 30;
const RETRY_DELAY: Duration = Duration::from_secs(10);
/// How often to send SSH keepalive packets (prevents inactivity timeout
/// during long silent operations like `cargo build --release`).
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
/// Max missed keepalives before disconnecting. 10 × 30s = 5 minutes of truly
/// dead network before giving up.
const KEEPALIVE_MAX: usize = 10;

/// SSH client handler that accepts any host key (for fresh servers).
pub(crate) struct SshHandler;

impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept all keys — these are freshly created servers.
        Ok(true)
    }
}

/// Connect to a server via SSH with retry logic.
pub async fn connect_with_retry(
    ip: IpAddr,
    server_name: &str,
    debug: bool,
    quiet: bool,
) -> Result<client::Handle<SshHandler>, ProvisionError> {
    if !quiet {
        tracing::info!(ip = %ip, server = server_name, "connecting via SSH");
    }

    let config = Arc::new(client::Config {
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        keepalive_max: KEEPALIVE_MAX,
        ..Default::default()
    });

    let addr = format!("{ip}:22");
    let mut last_err = None;

    for attempt in 1..=MAX_ATTEMPTS {
        match try_connect(&config, &addr, debug).await {
            Ok(handle) => {
                if !quiet {
                    tracing::info!(ip = %ip, server = server_name, "SSH connection established");
                }
                return Ok(handle);
            }
            Err(e) => {
                last_err = Some(e);
                if debug {
                    tracing::debug!(
                        ip = %ip,
                        attempt,
                        max = MAX_ATTEMPTS,
                        "SSH connection failed, retrying..."
                    );
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }

    Err(ProvisionError::ConnectionFailed {
        ip,
        attempts: MAX_ATTEMPTS,
        source: last_err.map_or_else(
            || anyhow::anyhow!("unknown error"),
            |e| anyhow::anyhow!("{e}"),
        ),
    })
}

async fn try_connect(
    config: &Arc<client::Config>,
    addr: &str,
    _debug: bool,
) -> Result<client::Handle<SshHandler>, anyhow::Error> {
    let mut handle = client::connect(config.clone(), addr, SshHandler)
        .await
        .map_err(|e| anyhow::anyhow!("connect failed: {e}"))?;

    // Authenticate using ssh-agent
    let mut agent = AgentClient::connect_env()
        .await
        .map_err(|e| anyhow::anyhow!("ssh-agent: {e}"))?;

    let identities = agent
        .request_identities()
        .await
        .map_err(|e| anyhow::anyhow!("ssh-agent identities: {e}"))?;

    if identities.is_empty() {
        return Err(anyhow::anyhow!("no SSH keys in agent"));
    }

    // Try each key until one succeeds
    let mut authenticated = false;
    for identity in &identities {
        let pubkey = identity.public_key().into_owned();
        let result = handle
            .authenticate_publickey_with("root", pubkey, None, &mut agent)
            .await
            .map_err(|e| anyhow::anyhow!("auth failed: {e}"))?;

        if matches!(result, russh::client::AuthResult::Success) {
            authenticated = true;
            break;
        }
    }

    if !authenticated {
        return Err(anyhow::anyhow!("authentication failed with all keys"));
    }

    Ok(handle)
}

/// Execute a script on a remote server and stream output.
pub async fn execute_script(
    handle: client::Handle<SshHandler>,
    server_name: &str,
    script: &str,
    spinner: Option<&super::Spinner>,
    debug: bool,
    _quiet: bool,
) -> Result<(), ProvisionError> {
    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|e| ProvisionError::Ssh(anyhow::anyhow!("open session: {e}")))?;

    channel
        .exec(true, script.as_bytes().to_vec())
        .await
        .map_err(|e| ProvisionError::Ssh(anyhow::anyhow!("exec: {e}")))?;

    let mut output = FilteredOutput::new(server_name, spinner, debug);

    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { data }) => {
                output.write_stdout(&data);
            }
            Some(ChannelMsg::ExtendedData { data, ext: 1 }) => {
                output.write_stderr(&data);
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                if exit_status != 0 {
                    return Err(ProvisionError::ScriptFailed {
                        server_name: server_name.to_owned(),
                        code: exit_status,
                    });
                }
            }
            Some(ChannelMsg::Eof | ChannelMsg::Close) | None => break,
            _ => {}
        }
    }

    Ok(())
}
