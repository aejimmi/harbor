use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::ConfigError;

/// Default config template (`~/.harbor/config.yaml`), written by
/// `harbor init`. Only `hetzner.token` is required; every other section
/// is optional and commented out.
pub(super) const DEFAULT_CONFIG: &str = r#"# Harbor user config — credentials only, never commit this file.

hetzner:
  token: ""                  # or set HCLOUD_TOKEN

# DNS records for server hostnames (optional). Both Cloudflare values and
# base_domain must be set for harbor to manage DNS.
# cloudflare:
#   api_token: ""
#   zone_id: ""
# dns:
#   base_domain: ".i.example.com"   # myapp -> myapp.i.example.com

# GitHub tokens for private repos (optional), keyed by project name.
# github:
#   tokens:
#     myapp: "github_pat_..."

# S3-compatible backup credentials (optional), keyed by project name.
# backup:
#   projects:
#     myapp:
#       access_key_id: ""
#       secret_access_key: ""
"#;

/// Create `~/.harbor/config.yaml` from the template (mode 0600).
/// An existing file is left untouched.
pub fn init_harbor_config() -> Result<(), ConfigError> {
    let path = super::harbor_dir()?.join("config.yaml");
    write_template_if_missing(&path, DEFAULT_CONFIG, 0o600)
}

fn write_template_if_missing(path: &Path, content: &str, mode: u32) -> Result<(), ConfigError> {
    if path.exists() {
        tracing::info!(path = %path.display(), "file already exists, skipping");
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ConfigError::CreateDirFailed {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    fs::write(path, content).map_err(|e| ConfigError::WriteFailed {
        path: path.display().to_string(),
        source: e,
    })?;

    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| {
        ConfigError::WriteFailed {
            path: path.display().to_string(),
            source: e,
        }
    })?;

    tracing::info!(path = %path.display(), "created");
    Ok(())
}
