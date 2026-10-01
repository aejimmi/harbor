use std::path::Path;

use anyhow::{Context, Result};

use crate::config::{self, SetupConfig, UserConfig};
use crate::script::ScriptBuilder;

/// Generate and print setup commands for an existing server.
pub fn run(
    setup_config_path: &Path,
    hostname: Option<&str>,
    user_config_path: Option<&Path>,
) -> Result<()> {
    let setup_config = SetupConfig::load(setup_config_path).context("loading setup config")?;

    // `generate` is normally credential-agnostic, but once `backup:`
    // is declared the operator needs concrete AWS_* values in the
    // rendered script — fail identically to `harbor up` so the
    // error is caught locally, not at the first timer fire.
    // Missing user config is only an error when `backup:` is
    // declared; otherwise keep the zero-config behaviour.
    let backup_creds = if setup_config.setup.backup.is_some() {
        let user_config = UserConfig::load(user_config_path).context("loading user config")?;
        config::require_backup_creds(&setup_config, &user_config)?;
        user_config.backup.for_project(&setup_config.name).cloned()
    } else {
        None
    };

    let config_dir = setup_config_path.parent().unwrap_or(Path::new("."));
    let mut builder =
        ScriptBuilder::from_setup_config(&setup_config, "", config_dir, backup_creds.as_ref())
            .context("building setup script")?;

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
