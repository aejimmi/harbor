use anyhow::Result;

use super::output;
use crate::config;

pub fn run() -> Result<()> {
    output::header("Harbor Configuration Setup");
    config::init_harbor_config()?;

    output::success("Harbor configuration initialized!");
    eprintln!();
    eprintln!("Configuration created in:");
    output::info("~/.harbor/config.yaml — Main configuration");
    output::info("~/.harbor/configs-deploy/ — Deployment configurations");
    output::info("~/.harbor/configs-server/ — Server setup configuration");
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("1. Edit ~/.harbor/config.yaml with your credentials");
    eprintln!("2. Configure your deployment files in ~/.harbor/configs-deploy/");
    eprintln!("3. Run: harbor server create <name> --ssh-key <key>");

    Ok(())
}
