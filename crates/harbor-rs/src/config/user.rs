use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

/// Main harbor credentials config (`~/.harbor/config.yaml`).
#[derive(Debug, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub cloudflare: CloudflareCredentials,
    #[serde(default)]
    pub hetzner: HetznerCredentials,
    #[serde(default)]
    pub dns: DnsSettings,
    #[serde(default)]
    pub github: GitHubCredentials,
    /// Per-project S3-compatible credentials for `backup:`. Keyed by
    /// the `name:` field in `harbor.yaml` — mirrors the
    /// `github.tokens.<name>` pattern.
    #[serde(default)]
    pub backup: BackupCredentialsMap,
}

/// Cloudflare API credentials.
#[derive(Debug, Default, Deserialize)]
pub struct CloudflareCredentials {
    #[serde(default)]
    pub api_token: String,
    #[serde(default)]
    pub zone_id: String,
}

/// Hetzner Cloud credentials.
#[derive(Debug, Default, Deserialize)]
pub struct HetznerCredentials {
    #[serde(default)]
    pub token: String,
}

/// DNS provider settings.
#[derive(Debug, Deserialize)]
pub struct DnsSettings {
    #[serde(default = "default_base_domain")]
    pub base_domain: String,
    #[allow(dead_code)]
    #[serde(default = "default_provider")]
    pub provider: String,
}

impl Default for DnsSettings {
    fn default() -> Self {
        Self {
            base_domain: default_base_domain(),
            provider: default_provider(),
        }
    }
}

/// No default domain — DNS is only managed once one is configured.
fn default_base_domain() -> String {
    String::new()
}

fn default_provider() -> String {
    "cloudflare".to_owned()
}

/// GitHub credentials for private repository access.
///
/// Tokens are stored per project name (matching `name:` in harbor.yaml):
/// ```yaml
/// github:
///   tokens:
///     blissd: "github_pat_..."
///     tell-platform: "github_pat_..."
/// ```
#[derive(Debug, Default, Deserialize)]
pub struct GitHubCredentials {
    /// Per-project fine-grained tokens.
    #[serde(default)]
    pub tokens: HashMap<String, String>,
}

impl GitHubCredentials {
    /// Look up the token for a project. Returns empty string if not found.
    pub fn token_for(&self, project: &str) -> &str {
        self.tokens.get(project).map_or("", |t| t.as_str())
    }
}

/// Per-project S3 credentials for `backup:`. Keyed by `SetupConfig.name`.
#[derive(Debug, Default, Deserialize)]
pub struct BackupCredentialsMap {
    #[serde(default)]
    pub projects: HashMap<String, BackupCredentials>,
}

impl BackupCredentialsMap {
    /// Look up credentials for a project. Returns `None` if the
    /// project has no entry at all.
    #[must_use]
    pub fn for_project(&self, project: &str) -> Option<&BackupCredentials> {
        self.projects.get(project)
    }
}

/// S3-compatible credentials for one project's backup destination.
///
/// `Debug` is implemented manually to redact `secret_access_key` —
/// this mirrors the `ServiceSpec.env` precedent so a stray
/// `tracing::debug!(?user_config)` or panic message can't leak the
/// secret. `access_key_id` is printed in full because it is an
/// identifier, not a credential material.
#[derive(Clone, Default, Deserialize)]
pub struct BackupCredentials {
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: String,
}

impl std::fmt::Debug for BackupCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let secret_repr: &str = if self.secret_access_key.is_empty() {
            ""
        } else {
            "<redacted>"
        };
        f.debug_struct("BackupCredentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &secret_repr)
            .finish()
    }
}

impl UserConfig {
    /// Load user config from a path, or the default `~/.harbor/config.yaml`.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => super::default_config_path()?,
        };
        super::paths::load_yaml(&config_path)
    }
}
