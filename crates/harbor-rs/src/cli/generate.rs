use std::path::Path;

use anyhow::{Context, Result};

use crate::config::SetupConfig;
use crate::script::ScriptBuilder;

/// Generate and print setup commands for an existing server.
pub fn run(setup_config_path: &Path, hostname: Option<&str>) -> Result<()> {
    let setup_config = SetupConfig::load(setup_config_path).context("loading setup config")?;

    let config_dir = setup_config_path.parent().unwrap_or(Path::new("."));
    let mut builder = ScriptBuilder::from_setup_config(&setup_config, "", config_dir);

    if let Some(h) = hostname {
        builder.add(crate::script::HostnameComponent {
            hostname: h.to_owned(),
        });
    }

    let script = builder.build();

    // Print only actual commands (skip shebang, set -e, comments, blanks)
    for line in script.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("#!/")
            || trimmed.starts_with("set -e")
            || trimmed.starts_with('#')
        {
            continue;
        }
        println!("{line}");
    }

    Ok(())
}
