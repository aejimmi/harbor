//! `services:` entry types for `harbor.yaml`.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Container runtime used when a `ServiceSpec` declares an `image`.
///
/// Docker is the default because it is what most users reach for first.
/// Selecting `Podman` routes the service through the Quadlet path
/// (`/etc/containers/systemd/<name>.container`) instead of a raw Docker
/// systemd unit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerRuntime {
    /// Render a hand-written Docker systemd unit.
    #[default]
    Docker,
    /// Render a Podman Quadlet `.container` file.
    Podman,
}

/// A systemd service to configure.
///
/// When `image` is `None`, Harbor renders a native systemd unit driven by
/// `exec_start`. When `image` is `Some`, Harbor renders a container-aware
/// unit (Docker `.service` or Podman Quadlet `.container`) selected by
/// `runtime`. The two modes are mutually exclusive: setting both `image`
/// and a non-empty `exec_start` is rejected at config load.
///
/// `Debug` is implemented manually so that `env` values are redacted
/// when a `ServiceSpec` is formatted — e.g. in `tracing::debug!(?svc)`
/// or a panic message — preventing secrets from leaking via logs.
#[derive(Clone, Deserialize)]
pub struct ServiceSpec {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    /// Start the service immediately. Defaults to false.
    #[serde(default)]
    pub start: bool,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub working_directory: String,
    #[serde(default)]
    pub exec_start: String,
    #[serde(default)]
    pub restart: String,
    #[serde(default)]
    pub restart_sec: u32,
    /// Container image to run. Setting this switches rendering to the
    /// container path; `exec_start` must be empty when `image` is set.
    #[serde(default)]
    pub image: Option<String>,
    /// Which container runtime to use. Defaults to `Docker`.
    #[serde(default)]
    pub runtime: ContainerRuntime,
    /// Port publications in native runtime syntax
    /// (`host:container[/proto]`). Emitted in declaration order.
    #[serde(default)]
    pub ports: Vec<String>,
    /// Bind mounts in native runtime syntax (`src:dest[:opts]`).
    /// Emitted in declaration order.
    #[serde(default)]
    pub volumes: Vec<String>,
    /// Environment variables scoped to this service. Uses `BTreeMap`
    /// for deterministic sorted rendering.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Linux capabilities to drop (e.g. `["ALL"]`). Empty = Docker
    /// defaults.
    #[serde(default)]
    pub cap_drop: Vec<String>,
    /// Linux capabilities to add back after `cap_drop`.
    #[serde(default)]
    pub cap_add: Vec<String>,
    /// Run the container with a read-only root filesystem. A writable
    /// `/tmp` tmpfs (64 MB) is added automatically.
    #[serde(default)]
    pub read_only: bool,
    /// Maximum number of processes inside the container. Defaults to
    /// 256. Set to `0` to disable the limit entirely.
    #[serde(default = "default_pids_limit")]
    pub pids_limit: u32,
}

fn default_pids_limit() -> u32 {
    256
}

impl std::fmt::Debug for ServiceSpec {
    /// Redacts the `env` map — only the key count is printed — so that
    /// secret values never surface in trace logs or panic messages.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceSpec")
            .field("name", &self.name)
            .field("enabled", &self.enabled)
            .field("start", &self.start)
            .field("user", &self.user)
            .field("working_directory", &self.working_directory)
            .field("exec_start", &self.exec_start)
            .field("restart", &self.restart)
            .field("restart_sec", &self.restart_sec)
            .field("image", &self.image)
            .field("runtime", &self.runtime)
            .field("ports", &self.ports)
            .field("volumes", &self.volumes)
            .field("env", &format!("<{} keys redacted>", self.env.len()))
            .field("cap_drop", &self.cap_drop)
            .field("cap_add", &self.cap_add)
            .field("read_only", &self.read_only)
            .field("pids_limit", &self.pids_limit)
            .finish()
    }
}
