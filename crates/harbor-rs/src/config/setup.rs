use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

/// Server setup/provisioning configuration (`harbor.yaml`).
#[derive(Debug, Deserialize)]
pub struct SetupConfig {
    /// Project/package name used to identify this config (e.g. `blissd`).
    #[serde(default)]
    pub name: String,
    /// Server infrastructure (optional — only needed for `harbor up/down`).
    #[serde(default)]
    pub server: Option<ServerSection>,
    pub setup: SetupSection,
}

/// The inner `setup:` block of a setup config.
#[derive(Debug, Default, Deserialize)]
pub struct SetupSection {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub components: Components,
    #[serde(default)]
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub github_repos: Vec<GithubRepo>,
    #[allow(dead_code)]
    #[serde(default)]
    pub ssh_keys: SshKeys,
    #[serde(default)]
    pub path: PathConfig,
    #[serde(default)]
    pub system_user: SystemUser,
    #[serde(default)]
    pub directories: Vec<DirectorySpec>,
    #[serde(default)]
    pub files: Vec<FileSpec>,
    #[serde(default)]
    pub services: Vec<ServiceSpec>,
    #[allow(dead_code)]
    #[serde(default)]
    pub dns: DnsConfig,
    #[serde(default)]
    pub deploy: Option<DeployConfig>,
    #[serde(default)]
    pub system: SystemConfig,
    #[serde(default)]
    pub updates: UpdateConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

/// Installable software components.
#[derive(Debug, Default, Deserialize)]
pub struct Components {
    #[serde(default)]
    pub docker: DockerConfig,
    #[serde(default)]
    pub go: GoConfig,
    #[serde(default)]
    pub fish: FishConfig,
    #[serde(default)]
    pub rust: RustConfig,
    #[serde(default)]
    pub caddy: CaddyConfig,
    #[serde(default)]
    pub chrony_nts: ChronyNtsConfig,
    #[serde(default)]
    pub fail2ban_rs: Fail2banRsConfig,
    #[serde(default)]
    pub swap: SwapConfig,
}

/// Docker installation toggle.
#[derive(Debug, Default, Deserialize)]
pub struct DockerConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Go installation settings.
#[derive(Debug, Default, Deserialize)]
pub struct GoConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub version: String,
}

/// Fish shell installation toggle.
#[derive(Debug, Default, Deserialize)]
pub struct FishConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Rust toolchain installation toggle.
#[derive(Debug, Default, Deserialize)]
pub struct RustConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Caddy web server installation toggle.
#[derive(Debug, Default, Deserialize)]
pub struct CaddyConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Chrony NTP with NTS installation toggle.
#[derive(Debug, Default, Deserialize)]
pub struct ChronyNtsConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// fail2ban-rs installation toggle.
#[derive(Debug, Default, Deserialize)]
pub struct Fail2banRsConfig {
    #[serde(default)]
    pub enabled: bool,
}

/// Swap file creation settings.
#[derive(Debug, Default, Deserialize)]
pub struct SwapConfig {
    #[serde(default)]
    pub size: String,
}

/// A GitHub repository to clone, build, and install.
#[derive(Debug, Clone, Deserialize)]
pub struct GithubRepo {
    pub repo: String,
    #[serde(default)]
    pub binary: String,
    #[serde(default)]
    pub install_path: String,
    #[serde(default)]
    pub config_source: String,
    #[serde(default)]
    pub config_target: String,
}

/// SSH key paths for private repo access.
#[derive(Debug, Default, Deserialize)]
pub struct SshKeys {
    #[allow(dead_code)]
    #[serde(default)]
    pub github_deploy_key: String,
}

/// PATH environment variable configuration.
#[derive(Debug, Default, Deserialize)]
pub struct PathConfig {
    #[serde(default)]
    pub mode: PathMode,
    #[serde(default)]
    pub paths: Vec<String>,
}

/// How to modify the system PATH.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PathMode {
    #[default]
    Prepend,
    Append,
    Overwrite,
}

/// System user to create on the server.
#[derive(Debug, Default, Deserialize)]
pub struct SystemUser {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub home: String,
    #[serde(default)]
    pub shell: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub system: bool,
}

/// A directory to create with specific ownership and permissions.
#[derive(Debug, Clone, Deserialize)]
pub struct DirectorySpec {
    pub path: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub mode: String,
}

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
            .finish()
    }
}

/// Clone a repo and run build/install steps.
#[derive(Debug, Clone, Deserialize)]
pub struct DeployConfig {
    /// Repository URL (e.g. `github.com/aejimmi/bliss-core`).
    pub repo: String,
    /// Commands to run inside the cloned repo.
    #[serde(default)]
    pub steps: Vec<String>,
}

/// DNS integration settings within setup config.
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct DnsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub base_domain: String,
    #[serde(default)]
    pub provider: String,
}

/// System-level settings.
#[derive(Debug, Default, Deserialize)]
pub struct SystemConfig {
    #[serde(default)]
    pub timezone: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub hostname_prefix: String,
}

/// System update policies.
#[derive(Debug, Default, Deserialize)]
pub struct UpdateConfig {
    #[serde(default)]
    pub auto_upgrade: bool,
    #[serde(default)]
    pub upgrade_kernel: bool,
    #[serde(default)]
    pub reboot_after_kernel: bool,
}

/// Security configuration.
#[derive(Debug, Default, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub ufw: UfwConfig,
    #[serde(default)]
    pub ssh_hardening: bool,
    #[serde(default)]
    pub kernel_hardening: bool,
}

/// UFW firewall settings.
#[derive(Debug, Default, Deserialize)]
pub struct UfwConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Backward-compatible simple port list (TCP, no limit).
    #[serde(default)]
    pub allow_ports: Vec<u16>,
    /// Rich rules with protocol and rate limiting.
    #[serde(default)]
    pub rules: Vec<UfwRule>,
}

/// A single UFW firewall rule.
#[derive(Debug, Clone, Deserialize)]
pub struct UfwRule {
    pub port: u16,
    #[serde(default = "default_proto")]
    pub proto: String,
    #[serde(default)]
    pub limit: bool,
}

fn default_proto() -> String {
    "tcp".to_owned()
}

/// A file to deploy from local repo to server.
#[derive(Debug, Clone, Deserialize)]
pub struct FileSpec {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub mode: String,
}

/// Server infrastructure specification (read from `server:` block).
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    /// Server name on Hetzner.
    pub name: String,
    /// Hetzner server type.
    #[serde(default = "default_server_type")]
    pub r#type: String,
    /// Hetzner datacenter location.
    #[serde(default = "default_location")]
    pub location: String,
    /// OS image.
    #[serde(default = "default_image")]
    pub image: String,
    /// SSH key name on Hetzner.
    pub ssh_key: String,
    /// Hostname for DNS record.
    #[serde(default)]
    pub hostname: Option<String>,
}

fn default_server_type() -> String {
    "cax11".to_owned()
}

fn default_location() -> String {
    "nbg1".to_owned()
}

fn default_image() -> String {
    "ubuntu-24.04".to_owned()
}

impl SetupConfig {
    /// Load a setup config from a YAML file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound {
                path: path.display().to_string(),
            });
        }

        let data = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadFailed {
            path: path.display().to_string(),
            source: e,
        })?;

        serde_yaml::from_str(&data).map_err(|e| ConfigError::ParseFailed {
            path: path.display().to_string(),
            source: e,
        })
    }
}
