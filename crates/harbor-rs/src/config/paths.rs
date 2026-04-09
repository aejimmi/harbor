use std::path::PathBuf;

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

/// Returns the default server profile config path
/// (`~/.harbor/configs-server/server-profile.yaml`).
pub fn default_server_config_path() -> Result<PathBuf, ConfigError> {
    Ok(harbor_dir()?
        .join("configs-server")
        .join("server-profile.yaml"))
}

/// Returns the deploy config path for a given environment name
/// (`~/.harbor/configs-deploy/{env}.yaml`).
#[allow(dead_code)]
pub fn deploy_config_path(env: &str) -> Result<PathBuf, ConfigError> {
    Ok(harbor_dir()?
        .join("configs-deploy")
        .join(format!("{env}.yaml")))
}
