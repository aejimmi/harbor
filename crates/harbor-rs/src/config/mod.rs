mod deploy;
mod fleet;
mod paths;
pub mod setup;
mod setup_backup;
mod setup_service;
mod setup_validate;
mod setup_volume;
mod templates;
mod user;

#[cfg(test)]
mod config_backup_test;
#[cfg(test)]
mod config_backup_validate_test;
#[cfg(test)]
mod config_deploys_test;
#[cfg(test)]
mod config_setup_test;
#[cfg(test)]
mod config_test_helpers;
#[cfg(test)]
mod examples_test;
#[cfg(test)]
mod fleet_test;
#[cfg(test)]
mod setup_validate_test;
#[cfg(test)]
mod setup_volume_test;

pub use deploy::ServerSpec;
#[allow(unused_imports)]
pub use fleet::{FleetConfig, FleetServer, RoleSpec, expand_servers};
pub use paths::{default_config_path, harbor_dir};
#[allow(unused_imports)]
pub use setup::{
    BackupConfig, BackupSchedule, BackupTransport, ContainerRuntime, DirectorySpec, PathMode,
    ServiceSpec, SetupConfig, UfwRule, VolumeFormat, VolumeSpec,
};
pub use templates::init_harbor_config;
// Re-exported for programmatic UserConfig construction (used in tests and future API consumers).
#[allow(unused_imports)]
pub use user::{
    BackupCredentials, BackupCredentialsMap, CloudflareCredentials, DnsSettings, GitHubCredentials,
    HetznerCredentials, UserConfig,
};

/// Errors from configuration loading and validation.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    ReadFailed {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    ParseFailed {
        path: String,
        source: serde_yaml::Error,
    },

    #[error("config file not found: {path}. Run 'harbor init' to create it")]
    NotFound { path: String },

    #[error("failed to get home directory")]
    NoHomeDir,

    #[error("failed to create directory {path}: {source}")]
    CreateDirFailed {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to write file {path}: {source}")]
    WriteFailed {
        path: String,
        source: std::io::Error,
    },

    #[error("invalid config: {message}")]
    Invalid { message: String },
}

/// Fail fast when `backup:` is declared in `harbor.yaml` but the
/// user config has no matching `backup.projects.<name>` entry (or
/// has one with empty credentials).
///
/// Called from `harbor up` and `harbor generate` before any script
/// rendering so operators see the missing-creds error immediately,
/// not at the first `rc object copy` that fails 12 hours later when
/// the timer fires.
///
/// When `setup.backup` is `None`, always returns `Ok` — the helper
/// is a no-op for projects that don't declare a backup block.
pub fn require_backup_creds(setup: &SetupConfig, user: &UserConfig) -> Result<(), anyhow::Error> {
    if setup.setup.backup.is_none() {
        return Ok(());
    }

    let project = &setup.name;
    let Some(creds) = user.backup.for_project(project) else {
        anyhow::bail!(
            "backup: is declared but user config has no backup.projects.{project} — \
             add credentials to ~/.harbor/config.yaml"
        );
    };

    if creds.access_key_id.is_empty() || creds.secret_access_key.is_empty() {
        anyhow::bail!(
            "backup: credentials for '{project}' are incomplete — \
             set access_key_id and secret_access_key in ~/.harbor/config.yaml"
        );
    }

    Ok(())
}
