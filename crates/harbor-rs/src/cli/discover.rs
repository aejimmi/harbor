use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::config::SetupConfig;

/// Find `harbor.yaml` by walking up from the current directory.
pub fn find_config() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("getting current directory")?;
    loop {
        let candidate = dir.join("harbor.yaml");
        if candidate.exists() {
            return Ok(candidate);
        }
        let candidate = dir.join("harbor.yml");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            bail!("no harbor.yaml found in current or parent directories");
        }
    }
}

/// Load the project config from auto-discovered harbor.yaml.
pub fn load_project_config() -> Result<(SetupConfig, PathBuf)> {
    let path = find_config()?;
    let config = SetupConfig::load(&path).context("loading harbor.yaml")?;
    Ok((config, path))
}
