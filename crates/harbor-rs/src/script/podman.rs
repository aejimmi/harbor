//! Podman installation component — installs the `podman` package from
//! Ubuntu's main repo and enables `podman.socket` so that Quadlet
//! `.container` files under `/etc/containers/systemd/` are picked up by
//! `podman-system-generator`.

use super::{ScriptComponent, status_echo};

/// Install Podman and enable the `podman.socket` systemd unit.
///
/// Added automatically by `ScriptBuilder::from_setup_config` when any
/// service declares `runtime: podman`. Podman Quadlet files written to
/// `/etc/containers/systemd/` are picked up by `podman-system-generator`
/// at `systemctl daemon-reload` time.
pub struct PodmanComponent;

impl ScriptComponent for PodmanComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Setting up Podman"),
            "apt-get install -y podman".to_owned(),
            "systemctl enable --now podman.socket".to_owned(),
        ]
    }
}
