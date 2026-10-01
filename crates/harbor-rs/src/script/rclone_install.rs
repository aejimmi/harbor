//! Install `rclone` via the upstream `install.sh`.
//!
//! No version pin in v1 — matches the `Fail2banRsComponent`
//! curl-pipe-bash precedent. Users on the default `transport: rc`
//! never execute this path; users on `transport: rclone` have
//! opted into the upstream install script.

use super::{ScriptComponent, status_echo};

/// Install `rclone` via the upstream install script.
///
/// Idempotent: if `rclone` is already on `$PATH`, the rendered
/// bash logs the installed version and skips the download.
pub struct RcloneInstallComponent;

impl ScriptComponent for RcloneInstallComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Installing rclone"),
            // Short-circuit when rclone is already installed.
            "if command -v rclone >/dev/null 2>&1; then \
             rclone --version; \
             else \
             curl -sSfL https://rclone.org/install.sh | bash; \
             fi"
            .to_owned(),
            status_echo("rclone installed"),
        ]
    }
}
