mod deploy;
mod paths;
mod setup;
mod templates;
mod user;

#[cfg(test)]
mod config_test;

pub use deploy::{DeployConfig, ServerSpec};
pub use paths::{default_config_path, default_server_config_path, harbor_dir};
pub use setup::{
    DirectorySpec, GithubRepo, PathMode, ServerSection, ServiceSpec, SetupConfig, UfwRule,
};
pub use templates::init_harbor_config;
// Re-exported for programmatic UserConfig construction (used in tests and future API consumers).
#[allow(unused_imports)]
pub use user::{
    CloudflareCredentials, DnsSettings, GitHubCredentials, HetznerCredentials, UserConfig,
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
}
