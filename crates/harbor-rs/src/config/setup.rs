use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;
pub use super::setup_backup::{BackupConfig, BackupSchedule, BackupTransport};
pub use super::setup_service::{ContainerRuntime, ServiceSpec};

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
///
/// `deny_unknown_fields` surfaces a parse error for legacy keys such as the
/// old top-level `deploy:` block that has been replaced by the `deploys:`
/// map. Users with stale YAML see the unknown field name instead of silent
/// no-ops.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupSection {
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub components: Components,
    #[serde(default)]
    pub environment: HashMap<String, String>,
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
    /// Named deployable entries. Each entry is independently runnable via
    /// `harbor deploy <name>`; its `services` field scopes health checks
    /// after the deploy to those systemd units only.
    #[serde(default)]
    pub deploys: HashMap<String, DeployConfig>,
    #[serde(default)]
    pub system: SystemConfig,
    #[serde(default)]
    pub updates: UpdateConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    /// Optional backup configuration. When absent, the server is not
    /// provisioned with backup artifacts. See 011-backup-config spec.
    #[serde(default)]
    pub backup: Option<BackupConfig>,
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

/// A single named deploy entry. Harbor preserves the built `binary`
/// under `/opt/harbor/<name>/<sha>/<basename>` and swaps a symlink at
/// `install` to point at it. Rollback swaps the symlink back — no
/// rebuild. `binary` and `install` are required.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployConfig {
    /// Repository URL (e.g. `github.com/aejimmi/bliss-core`).
    pub repo: String,
    /// Repo-relative path to the single binary produced by `steps`
    /// (e.g. `target/release/web`). Traversal and absolute paths are
    /// rejected at config load.
    pub binary: String,
    /// Absolute path where the active-version symlink is maintained
    /// (e.g. `/usr/local/bin/web`).
    pub install: String,
    /// Commands to run inside the cloned repo.
    #[serde(default)]
    pub steps: Vec<String>,
    /// Systemd service names whose health is checked after the deploy.
    /// Empty = no health checks — deploy succeeds on step exit codes alone.
    #[serde(default)]
    pub services: Vec<String>,
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
    /// Mount `/tmp`, `/var/tmp`, `/dev/shm` with `noexec,nosuid,nodev`.
    #[serde(default)]
    pub mount_hardening: bool,
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
    /// Server name on Hetzner. Fleet roles may omit it — fleet generates
    /// `{role}-{fleet}-{n}` names; single-server commands require it.
    #[serde(default)]
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
    /// Load a setup config from a YAML file. Runs `validate()` after
    /// deserialization so structural rules that serde can't express —
    /// deploy-name shape, no `..` in `binary:`, absolute `install:` —
    /// surface as `Invalid` errors before they reach bash rendering.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let config: Self = super::paths::load_yaml(path)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate post-parse invariants that serde can't express.
    ///
    /// Delegates to `setup_validate` — see that module for the rules.
    pub fn validate(&self) -> Result<(), ConfigError> {
        super::setup_validate::validate(self)
    }
}
