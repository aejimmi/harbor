// Legacy deploy config — replaced by FleetConfig. Kept for existing tests
// and backward compatibility. Will be removed in a future release.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

/// Deployment configuration (`configs-deploy/*.yaml`).
///
/// Defines which servers to create and optional environment variables.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DeployConfig {
    pub hcloud: HCloudSection,
    pub servers: Vec<ServerSpec>,
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

/// Hetzner Cloud connection settings within a deploy config.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HCloudSection {
    #[serde(default)]
    pub token: String,
    pub ssh_key: String,
}

/// Specification for a single server to create.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub server_type: String,
    pub location: String,
    pub image: String,
}

#[allow(dead_code)]
impl DeployConfig {
    /// Load a deployment config from a YAML file.
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

    /// Resolve the Hetzner token using fallback chain:
    /// deploy config -> user config -> HCLOUD_TOKEN env var.
    pub fn resolve_token(&mut self, user_config: Option<&super::UserConfig>) {
        if !self.hcloud.token.is_empty() {
            return;
        }

        if let Some(uc) = user_config
            && !uc.hetzner.token.is_empty()
        {
            self.hcloud.token.clone_from(&uc.hetzner.token);
            return;
        }

        if let Ok(token) = std::env::var("HCLOUD_TOKEN") {
            self.hcloud.token = token;
        }
    }
}
