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

fn default_base_domain() -> String {
    ".i.usercanal.com".to_owned()
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

impl UserConfig {
    /// Load user config from a path, or the default `~/.harbor/config.yaml`.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => super::default_config_path()?,
        };

        if !config_path.exists() {
            return Err(ConfigError::NotFound {
                path: config_path.display().to_string(),
            });
        }

        let data = std::fs::read_to_string(&config_path).map_err(|e| ConfigError::ReadFailed {
            path: config_path.display().to_string(),
            source: e,
        })?;

        serde_yaml::from_str(&data).map_err(|e| ConfigError::ParseFailed {
            path: config_path.display().to_string(),
            source: e,
        })
    }
}
