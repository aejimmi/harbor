use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::ConfigError;

/// Default config template (`~/.harbor/config.yaml`).
const DEFAULT_CONFIG: &str = r#"# Harbor Configuration
# Edit these values with your actual credentials

cloudflare:
  api_token: "your_cloudflare_api_token_here"
  zone_id: "your_cloudflare_zone_id_here"

hetzner:
  token: "your_hetzner_cloud_token_here"

dns:
  base_domain: ".i.usercanal.com"  # Default domain for DNS records
  provider: "cloudflare"           # DNS provider

github:
  token: "your_github_token_here"  # For private repository access
"#;

/// Production deploy template.
const PRODUCTION_DEPLOY: &str = r#"# Production deployment configuration for Hetzner Cloud
# This defines what servers to create for a production environment
hcloud:
  token: "" # Will use token from ~/.harbor/config.yaml
  ssh_key: "production-key"

servers:
  - name: "app-prod-01"
    type: "cpx31" # 4 cores, 8GB RAM, 160GB disk
    location: "nbg1"
    image: "ubuntu-24.04"
  - name: "app-prod-02"
    type: "cpx31"
    location: "fsn1" # Different location for redundancy
    image: "ubuntu-24.04"
  - name: "db-prod-01"
    type: "ccx33" # 8 dedicated cores, 32GB RAM, 240GB disk
    location: "nbg1"
    image: "ubuntu-24.04"
"#;

/// Staging deploy template.
const STAGING_DEPLOY: &str = r#"# Staging deployment configuration for Hetzner Cloud
# This defines what servers to create for a staging environment
hcloud:
  token: "" # Will use token from ~/.harbor/config.yaml
  ssh_key: "staging-key"

servers:
  - name: "app-staging-01"
    type: "cax11" # 2 cores, 4GB RAM, 40GB disk
    location: "nbg1"
    image: "ubuntu-24.04"
  - name: "db-staging-01"
    type: "cpx21" # 3 cores, 4GB RAM, 80GB disk
    location: "nbg1"
    image: "ubuntu-24.04"
"#;

/// Development deploy template.
const DEVELOPMENT_DEPLOY: &str = r#"# Development deployment configuration for Hetzner Cloud
# This defines what servers to create for a development environment
hcloud:
  token: "" # Will use token from ~/.harbor/config.yaml
  ssh_key: "dev-key"

servers:
  - name: "app-dev-01"
    type: "cax11" # 2 cores, 4GB RAM, 40GB disk
    location: "nbg1"
    image: "ubuntu-24.04"
"#;

/// Server profile template.
const SERVER_PROFILE: &str = r#"# configs-server/server-profile.yaml
# Server setup configuration - shared across all deployments
setup:
  components:
    docker:
      enabled: true
    go:
      enabled: true
      version: "1.24.5"
  dns:
    enabled: true
    base_domain: ".i.usercanal.com"
    provider: "cloudflare"
  environment: {}
  path:
    mode: "prepend"
    paths:
      - "/usr/local/go/bin"
  system_user:
    name: "usercanal"
    home: "/var/lib/usercanal"
    shell: "/bin/bash"
    system: true
  directories:
    - path: "/var/lib/usercanal/data"
      owner: "usercanal"
      group: "usercanal"
      mode: "755"
    - path: "/var/log/usercanal"
      owner: "usercanal"
      group: "usercanal"
      mode: "755"
  github_repos:
    - repo: "github.com/usercanal/usercanal"
      binary: "usercanal"
      install_path: "/usr/local/bin"
      config_source: "configs/collector-server.yaml"
      config_target: "/etc/usercanal/usercanal.yaml"
  ssh_keys:
    github_deploy_key: "/root/.ssh/github_deploy_key"
  packages:
    - ca-certificates
    - curl
    - gnupg
    - jq
    - git
    - build-essential
  system:
    timezone: "UTC"
    hostname_prefix: "hetzner"
  updates:
    auto_upgrade: true
    upgrade_kernel: true
    reboot_after_kernel: true
  security:
    ufw:
      enabled: true
      allow_ports: [22, 50000]
  services:
    - name: "usercanal"
      enabled: true
      user: "usercanal"
      working_directory: "/var/lib/usercanal"
      exec_start: "/usr/local/bin/usercanal"
      restart: "always"
      restart_sec: 10
"#;

/// Template file entry: relative path within `~/.harbor/` and content.
struct TemplateFile {
    relative_path: &'static str,
    content: &'static str,
    /// File permission mode (e.g., 0o600 for secrets).
    mode: u32,
}

const TEMPLATE_FILES: &[TemplateFile] = &[
    TemplateFile {
        relative_path: "config.yaml",
        content: DEFAULT_CONFIG,
        mode: 0o600,
    },
    TemplateFile {
        relative_path: "configs-deploy/production.yaml",
        content: PRODUCTION_DEPLOY,
        mode: 0o644,
    },
    TemplateFile {
        relative_path: "configs-deploy/staging.yaml",
        content: STAGING_DEPLOY,
        mode: 0o644,
    },
    TemplateFile {
        relative_path: "configs-deploy/development.yaml",
        content: DEVELOPMENT_DEPLOY,
        mode: 0o644,
    },
    TemplateFile {
        relative_path: "configs-server/server-profile.yaml",
        content: SERVER_PROFILE,
        mode: 0o644,
    },
];

/// Initialize the `~/.harbor/` directory structure with template files.
///
/// Creates directories and writes template YAML files. Existing files are
/// skipped without overwriting.
pub fn init_harbor_config() -> Result<(), ConfigError> {
    let base = super::harbor_dir()?;

    let dirs = ["configs-deploy", "configs-server"];
    for dir in dirs {
        let dir_path = base.join(dir);
        fs::create_dir_all(&dir_path).map_err(|e| ConfigError::CreateDirFailed {
            path: dir_path.display().to_string(),
            source: e,
        })?;
    }

    for template in TEMPLATE_FILES {
        let file_path = base.join(template.relative_path);
        write_template_if_missing(&file_path, template.content, template.mode)?;
    }

    Ok(())
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
