use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::ConfigError;

/// Fleet composition config (`fleet.yaml`).
///
/// Maps role names to counts. Each role resolves to a directory
/// containing a `harbor.yaml` (setup config) and optional `dist/`.
#[derive(Debug, Deserialize)]
pub struct FleetConfig {
    pub roles: HashMap<String, RoleSpec>,
}

/// How a role is specified in `fleet.yaml`.
///
/// Short form: `collectors: 3` (just a count, directory = `./{role}/`).
/// Long form: `api: { count: 2, path: ./platform }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RoleSpec {
    /// Short form — just a count.
    Short(u32),
    /// Long form — count with optional path override.
    Long { count: u32, path: Option<PathBuf> },
}

impl RoleSpec {
    /// Number of servers for this role.
    #[must_use]
    pub fn count(&self) -> u32 {
        match self {
            Self::Short(n) => *n,
            Self::Long { count, .. } => *count,
        }
    }

    /// Directory containing the role's `harbor.yaml`, relative to `base_dir`.
    #[must_use]
    pub fn role_dir(&self, role_name: &str, base_dir: &Path) -> PathBuf {
        match self {
            Self::Short(_) => base_dir.join(role_name),
            Self::Long { path: Some(p), .. } => base_dir.join(p),
            Self::Long { path: None, .. } => base_dir.join(role_name),
        }
    }
}

/// An expanded server derived from fleet config + fleet name.
#[derive(Debug, Clone)]
pub struct FleetServer {
    /// Generated server name: `{role}-{fleet_name}-{index}`.
    pub name: String,
    /// Role this server belongs to.
    pub role: String,
    /// Path to the role directory containing `harbor.yaml`.
    pub role_dir: PathBuf,
}

impl FleetConfig {
    /// Load a fleet config from a YAML file.
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

    /// Validate the fleet config against the filesystem.
    ///
    /// Checks that each role has count >= 1, the role directory exists,
    /// and it contains a `harbor.yaml` with a `server:` section.
    pub fn validate(&self, base_dir: &Path) -> Result<(), anyhow::Error> {
        for (role_name, spec) in &self.roles {
            anyhow::ensure!(spec.count() >= 1, "role '{role_name}' must have count >= 1");

            // Reject path traversal.
            if let RoleSpec::Long { path: Some(p), .. } = spec {
                let p_str = p.to_string_lossy();
                anyhow::ensure!(
                    !p_str.contains(".."),
                    "role '{role_name}' path must not contain '..'"
                );
            }

            let dir = spec.role_dir(role_name, base_dir);
            anyhow::ensure!(
                dir.is_dir(),
                "role '{role_name}' directory not found: {}",
                dir.display()
            );

            let harbor_yaml = dir.join("harbor.yaml");
            anyhow::ensure!(
                harbor_yaml.is_file(),
                "role '{role_name}' missing harbor.yaml: {}",
                harbor_yaml.display()
            );

            // Check that harbor.yaml has a server: section.
            let setup = super::SetupConfig::load(&harbor_yaml)
                .map_err(|e| anyhow::anyhow!("role '{role_name}': {e}"))?;
            anyhow::ensure!(
                setup.server.is_some(),
                "role '{role_name}' harbor.yaml missing 'server:' section"
            );
        }

        Ok(())
    }
}

/// Expand a fleet config into individual server entries.
///
/// Roles are sorted alphabetically for deterministic ordering.
/// Server names follow `{role}-{fleet_name}-{index}` (1-based).
#[must_use]
pub fn expand_servers(config: &FleetConfig, fleet_name: &str, base_dir: &Path) -> Vec<FleetServer> {
    let mut roles: Vec<_> = config.roles.iter().collect();
    roles.sort_by_key(|(name, _)| name.as_str().to_owned());

    let mut servers = Vec::new();
    for (role_name, spec) in &roles {
        let dir = spec.role_dir(role_name, base_dir);
        for i in 1..=spec.count() {
            servers.push(FleetServer {
                name: format!("{role_name}-{fleet_name}-{i}"),
                role: (*role_name).clone(),
                role_dir: dir.clone(),
            });
        }
    }
    servers
}
