use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use super::ConfigError;

/// Returns the harbor configuration directory (`~/.harbor/`).
pub fn harbor_dir() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
    Ok(home.join(".harbor"))
}

/// Returns the default config file path (`~/.harbor/config.yaml`).
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    Ok(harbor_dir()?.join("config.yaml"))
}

/// Read and parse a YAML file. A missing file is `NotFound` (so callers
/// can suggest `harbor init` or a path fix); other IO errors are
/// `ReadFailed`.
pub(crate) fn load_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let shown = || path.display().to_string();
    let data = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ConfigError::NotFound { path: shown() }
        } else {
            ConfigError::ReadFailed {
                path: shown(),
                source: e,
            }
        }
    })?;
    serde_yaml::from_str(&data).map_err(|e| ConfigError::ParseFailed {
        path: shown(),
        source: e,
    })
}
