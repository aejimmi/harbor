use std::path::Path;

use anyhow::{Context, Result, bail};

use super::output;
use crate::config::{self, SetupConfig};

/// Install a harbor config into `~/.harbor/configs-deploy/{name}.yaml`.
pub fn install(config_path: &Path) -> Result<()> {
    let setup = SetupConfig::load(config_path).context("loading config")?;

    if setup.name.is_empty() {
        bail!(
            "config is missing required 'name' field at top level.\n\
             Add e.g. `name: myproject` to {}",
            config_path.display()
        );
    }

    let deploy_dir = config::harbor_dir()
        .context("finding harbor directory")?
        .join("configs-deploy");

    std::fs::create_dir_all(&deploy_dir).context("creating configs-deploy directory")?;

    let dest = deploy_dir.join(format!("{}.yaml", setup.name));
    std::fs::copy(config_path, &dest)
        .with_context(|| format!("copying {} to {}", config_path.display(), dest.display()))?;

    output::success(&format!("installed '{}' -> {}", setup.name, dest.display()));
    Ok(())
}

/// List installed deployment configurations.
pub fn list() {
    let Ok(harbor) = config::harbor_dir() else {
        output::error("failed to find harbor directory");
        return;
    };
    let deploy_dir = harbor.join("configs-deploy");

    let Ok(entries) = std::fs::read_dir(&deploy_dir) else {
        output::subtle("no deployment configurations installed");
        return;
    };

    output::header("Installed Configurations");
    eprintln!();

    let mut count = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(".yaml") || name_str.ends_with(".yml") {
            let stem = name_str.trim_end_matches(".yaml").trim_end_matches(".yml");
            output::info(stem);
            count += 1;
        }
    }

    if count == 0 {
        output::subtle("no configs installed — run: harbor config install <path>");
    } else {
        eprintln!();
        output::subtle("usage: harbor config show <name>");
    }
}

/// Show the contents of an installed configuration.
pub fn show(name: &str) -> Result<()> {
    let deploy_dir = config::harbor_dir()
        .context("finding harbor directory")?
        .join("configs-deploy");

    let path = deploy_dir.join(format!("{name}.yaml"));
    if !path.exists() {
        let yml_path = deploy_dir.join(format!("{name}.yml"));
        if yml_path.exists() {
            let contents = std::fs::read_to_string(&yml_path)
                .with_context(|| format!("reading {}", yml_path.display()))?;
            print!("{contents}");
            return Ok(());
        }
        bail!("config '{name}' not found. Run 'harbor config list' to see installed configs");
    }

    let contents =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    print!("{contents}");
    Ok(())
}
