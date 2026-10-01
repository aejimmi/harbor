use super::{STATUS_SENTINEL, ScriptComponent, status_echo};

/// Install Rust via rustup.
pub struct RustComponent;

impl ScriptComponent for RustComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Installing Rust toolchain"),
            // rustup-init downloads to $TMPDIR and execs from it. When
            // the system has `mount_hardening: true`, /tmp is mounted
            // noexec, so rustup-init can't run. Point TMPDIR at a
            // writable+executable path under /root for the duration
            // of the installer. The prepared dir is also idempotent —
            // re-running over an existing rustup install is a no-op
            // handled by rustup-init itself.
            "mkdir -p /root/rustup-tmp".to_owned(),
            "TMPDIR=/root/rustup-tmp curl --proto '=https' --tlsv1.2 -sSf \
             https://sh.rustup.rs | TMPDIR=/root/rustup-tmp sh -s -- -y"
                .to_owned(),
            "rm -rf /root/rustup-tmp".to_owned(),
            "source $HOME/.cargo/env".to_owned(),
            format!("echo \"{STATUS_SENTINEL} Rust $(rustc --version) installed\""),
        ]
    }
}
