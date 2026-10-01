use anyhow::Result;

use super::output;
use crate::config;

pub fn run() -> Result<()> {
    output::header("Harbor Configuration Setup");
    config::init_harbor_config()?;

    output::success("Created ~/.harbor/config.yaml");
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("1. Add your Hetzner token to ~/.harbor/config.yaml (or set HCLOUD_TOKEN)");
    eprintln!("2. Add a harbor.yaml to your project — see examples/ in the harbor repo");
    eprintln!("3. Run: harbor up");

    Ok(())
}
