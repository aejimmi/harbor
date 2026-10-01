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

/// SSH client handler that verifies host keys.
///
/// When `expected_host_key` is `Some`, the server's key must match
/// exactly or the connection is rejected. When `None`, any key is
/// accepted (first connection to a new server).
///
/// The accepted key is stored in `accepted_key` so the caller can
/// persist it for future verification.
pub(crate) struct SshHandler {
    /// Expected key in OpenSSH format (`ssh-ed25519 AAAA...`).
    expected_host_key: Option<String>,
    /// Slot for the accepted key — read by caller after connection.
    accepted_key: Arc<std::sync::Mutex<Option<String>>>,
    /// Set to `true` when the server presented a key that did not match
    /// `expected_host_key`. Lets `connect_with_retry` distinguish a
    /// genuine mismatch from a network timeout.
    key_mismatch: Arc<std::sync::Mutex<bool>>,
}

impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let Ok(openssh_str) = server_public_key.to_openssh() else {
            // Serialization failure is extremely rare — reject.
            return Ok(false);
        };

        if let Some(ref expected) = self.expected_host_key
            && openssh_str != *expected
        {
            if let Ok(mut flag) = self.key_mismatch.lock() {
                *flag = true;
            }
            return Ok(false);
        }

        if let Ok(mut slot) = self.accepted_key.lock() {
            *slot = Some(openssh_str);
        }
        Ok(true)
    }
}

/// Connect to a server via SSH with retry logic.
///
/// When `expected_host_key` is `Some`, the server's host key must match
/// or the connection fails immediately with `HostKeyMismatch`. When
/// `None`, any key is accepted (first connection).
///
/// Returns the connection handle and the accepted host key in OpenSSH
/// format so the caller can persist it.
pub async fn connect_with_retry(
    ip: IpAddr,
    server_name: &str,
    expected_host_key: Option<&str>,
    debug: bool,
    quiet: bool,
) -> Result<(client::Handle<SshHandler>, String), ProvisionError> {
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

    let accepted_key: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
    let key_mismatch: Arc<std::sync::Mutex<bool>> = Arc::new(std::sync::Mutex::new(false));

    for attempt in 1..=MAX_ATTEMPTS {
        // Reset mismatch flag for each attempt.
        if let Ok(mut flag) = key_mismatch.lock() {
            *flag = false;
        }

        match try_connect(
            &config,
            &addr,
            expected_host_key,
            &accepted_key,
            &key_mismatch,
            debug,
        )
        .await
        {
            Ok(handle) => {
                if !quiet {
                    tracing::info!(ip = %ip, server = server_name, "SSH connection established");
                }
                let key = accepted_key
                    .lock()
                    .ok()
                    .and_then(|g| g.clone())
                    .ok_or_else(|| ProvisionError::Ssh(anyhow::anyhow!("host key not captured")))?;
                return Ok((handle, key));
            }
            Err(e) => {
                // Host key mismatch — no point retrying.
                if *key_mismatch
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                {
                    return Err(ProvisionError::HostKeyMismatch { ip });
                }
                // Local agent problems won't fix themselves — fail fast.
                match e.downcast_ref::<ProvisionError>() {
                    Some(ProvisionError::NoSshAgent) => return Err(ProvisionError::NoSshAgent),
                    Some(ProvisionError::NoSshKeys) => return Err(ProvisionError::NoSshKeys),
                    _ => {}
                }

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
    expected_host_key: Option<&str>,
    accepted_key: &Arc<std::sync::Mutex<Option<String>>>,
    key_mismatch: &Arc<std::sync::Mutex<bool>>,
    _debug: bool,
) -> Result<client::Handle<SshHandler>, anyhow::Error> {
    let handler = SshHandler {
        expected_host_key: expected_host_key.map(ToOwned::to_owned),
        accepted_key: Arc::clone(accepted_key),
        key_mismatch: Arc::clone(key_mismatch),
    };

    let mut handle = client::connect(config.clone(), addr, handler)
        .await
        .map_err(|e| anyhow::anyhow!("connect failed: {e}"))?;

    // Authenticate using ssh-agent
    let mut agent = AgentClient::connect_env()
        .await
        .map_err(|_| ProvisionError::NoSshAgent)?;

    let identities = agent
        .request_identities()
        .await
        .map_err(|e| anyhow::anyhow!("ssh-agent identities: {e}"))?;

    if identities.is_empty() {
        return Err(ProvisionError::NoSshKeys.into());
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

    // Always interpret the script via bash, regardless of the remote
    // user's login shell. `exec` hands the command to the target's
    // login shell by default — on a box where root uses fish/zsh, the
    // bash script we render here (process substitution, heredocs,
    // `${var:+...}` expansion, etc.) fails syntax-check on line one.
    // Starting `bash -s` and piping the script as stdin bypasses the
    // login shell entirely.
    channel
        .exec(true, "bash -s")
        .await
        .map_err(|e| ProvisionError::Ssh(anyhow::anyhow!("exec: {e}")))?;

    channel
        .data(script.as_bytes())
        .await
        .map_err(|e| ProvisionError::Ssh(anyhow::anyhow!("send script: {e}")))?;

    channel
        .eof()
        .await
        .map_err(|e| ProvisionError::Ssh(anyhow::anyhow!("close stdin: {e}")))?;

    let mut output = FilteredOutput::new(server_name, spinner, debug);

    // Drain until the channel closes. Servers commonly send EOF *before*
    // the exit status, so stopping at EOF would report failed scripts as
    // successful.
    let mut exit_code = None;
    let mut exit_signal = None;
    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { data }) => output.write_stdout(&data),
            Some(ChannelMsg::ExtendedData { data, ext: 1 }) => output.write_stderr(&data),
            Some(ChannelMsg::ExitStatus { exit_status }) => exit_code = Some(exit_status),
            Some(ChannelMsg::ExitSignal { signal_name, .. }) => exit_signal = Some(signal_name),
            Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }

    match (exit_code, exit_signal) {
        (Some(0), _) => Ok(()),
        (Some(code), _) => Err(ProvisionError::ScriptFailed {
            server_name: server_name.to_owned(),
            code,
        }),
        (None, Some(sig)) => Err(ProvisionError::Ssh(anyhow::anyhow!(
            "script on {server_name} killed by signal {sig:?}"
        ))),
        (None, None) => Err(ProvisionError::Ssh(anyhow::anyhow!(
            "connection to {server_name} closed without an exit status"
        ))),
    }
}
