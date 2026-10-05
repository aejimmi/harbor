//! Post-parse invariants for `SetupConfig`.
//!
//! Runs after serde deserialization. Rejects:
//! - deploy names outside `[A-Za-z0-9_-]` (they would need shell
//!   quoting when interpolated into generated bash paths and could
//!   otherwise enable shell injection)
//! - `binary:` paths with a `..` segment or starting with `/`
//!   (prevents escaping the cloned repo tree and reading e.g.
//!   `/etc/shadow` into the versioned install dir)
//! - `install:` paths that are not absolute (the symlink swap uses
//!   the value verbatim in bash — relative paths would resolve
//!   against whatever cwd the SSH session happens to have)
//! - `backup:` block invariants — see `validate_backup`
//! - project, service, and system user names outside a safe charset —
//!   they become file paths, unit names, and bash arguments
//! - env keys that aren't identifiers, env values with newlines (they
//!   would inject extra lines into env files and heredocs)
//! - `files:`/`directories:` paths that aren't plain absolute paths, and
//!   modes/owners that aren't octal/safe names
//! - `server.volumes` and `system.journald_max_use` — see `setup_volume`

use super::setup::BackupConfig;
use super::{ConfigError, SetupConfig};

/// Run every invariant over every deploy entry. First failure wins so
/// users fix their YAML one error at a time.
pub(super) fn validate(config: &SetupConfig) -> Result<(), ConfigError> {
    validate_names(config)?;
    validate_services(config)?;
    validate_env(config)?;
    validate_fs_entries(config)?;
    if let Some(server) = &config.server {
        super::setup_volume::validate_volumes(&server.volumes)?;
    }
    super::setup_volume::validate_journald_max_use(&config.setup.system.journald_max_use)?;
    for (name, deploy) in &config.setup.deploys {
        validate_deploy_name(name)?;
        validate_binary(name, &deploy.binary)?;
        validate_install(name, &deploy.install)?;
    }

    if let Some(backup) = &config.setup.backup {
        validate_backup(backup, config)?;
    }

    Ok(())
}

/// Characters allowed in names that reach generated bash and paths.
/// `.` and `@` cover systemd unit names like `foo@1` or `foo.bar`.
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'))
}

/// Reject project, service, and system user names that would need shell
/// quoting. An empty project or user name means "not set" and is allowed.
fn validate_names(config: &SetupConfig) -> Result<(), ConfigError> {
    let setup = &config.setup;
    let optional = [
        ("project", &config.name),
        ("system_user", &setup.system_user.name),
    ];
    let named = optional.into_iter().filter(|(_, n)| !n.is_empty());
    let services = setup.services.iter().map(|s| ("service", &s.name));
    for (kind, name) in named.chain(services) {
        if !is_safe_name(name) {
            return Err(ConfigError::Invalid {
                message: format!(
                    "{kind} name '{name}' may only contain [A-Za-z0-9_.@-] \
                     and must not start with '-' or '.'"
                ),
            });
        }
    }
    Ok(())
}

/// A service runs either a native binary (`exec_start`) or a container
/// (`image`), never both; an image must not be blank.
fn validate_services(config: &SetupConfig) -> Result<(), ConfigError> {
    for svc in &config.setup.services {
        let Some(image) = &svc.image else { continue };
        if !svc.exec_start.is_empty() {
            return Err(invalid(format!(
                "service `{}`: `image` and `exec_start` are mutually exclusive. Remove one.",
                svc.name
            )));
        }
        if image.trim().is_empty() {
            return Err(invalid(format!(
                "service `{}`: `image` must not be empty.",
                svc.name
            )));
        }
    }
    Ok(())
}

fn invalid(message: String) -> ConfigError {
    ConfigError::Invalid { message }
}

/// Env keys must be shell identifiers; values must be single-line.
/// `environment:` values also land in `/etc/environment` as `KEY="v"`,
/// so they can't contain `"`.
fn validate_env(config: &SetupConfig) -> Result<(), ConfigError> {
    let setup = &config.setup;
    let global = setup.environment.iter().map(|(k, v)| ("environment", k, v));
    let services = setup
        .services
        .iter()
        .flat_map(|s| s.env.iter().map(|(k, v)| (s.name.as_str(), k, v)));
    for (scope, key, value) in global.chain(services) {
        let mut chars = key.chars();
        let ident = chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !ident {
            return Err(invalid(format!(
                "{scope}: env key '{key}' is not a valid identifier"
            )));
        }
        if value.contains(['\n', '\r']) {
            return Err(invalid(format!(
                "{scope}: env value for '{key}' must be a single line"
            )));
        }
        if scope == "environment" && value.contains('"') {
            return Err(invalid(format!(
                "environment: value for '{key}' must not contain '\"'"
            )));
        }
    }
    Ok(())
}

/// A plain absolute path: no `..`, no whitespace or shell metacharacters.
fn is_plain_abs_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.split('/').any(|seg| seg == "..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.' | '@' | '+'))
}

fn is_octal_mode(mode: &str) -> bool {
    mode.is_empty()
        || ((3..=4).contains(&mode.len()) && mode.chars().all(|c| ('0'..='7').contains(&c)))
}

/// `files:` targets and `directories:` paths, plus their modes and owners.
fn validate_fs_entries(config: &SetupConfig) -> Result<(), ConfigError> {
    let setup = &config.setup;
    let files = setup
        .files
        .iter()
        .map(|f| (&f.target, &f.mode, &f.owner, &f.group));
    let dirs = setup
        .directories
        .iter()
        .map(|d| (&d.path, &d.mode, &d.owner, &d.group));
    for (path, mode, owner, group) in files.chain(dirs) {
        if !is_plain_abs_path(path) {
            return Err(invalid(format!(
                "path '{path}' must be absolute without '..', spaces, or shell characters"
            )));
        }
        if !is_octal_mode(mode) {
            return Err(invalid(format!(
                "{path}: mode '{mode}' must be octal like 644 or 0755"
            )));
        }
        for who in [owner, group] {
            if !who.is_empty() && !is_safe_name(who) {
                return Err(invalid(format!(
                    "{path}: owner/group '{who}' is not a valid name"
                )));
            }
        }
    }
    Ok(())
}

/// Reject deploy names outside `[A-Za-z0-9_-]+` — any other character
/// would need shell quoting when interpolated into generated bash.
fn validate_deploy_name(name: &str) -> Result<(), ConfigError> {
    if name.is_empty() {
        return Err(ConfigError::Invalid {
            message: "deploy name must not be empty".to_owned(),
        });
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(ConfigError::Invalid {
                message: format!("deploy '{name}': name may only contain [A-Za-z0-9_-] characters"),
            });
        }
    }
    Ok(())
}

/// Reject `binary:` values that can escape the cloned repo tree.
fn validate_binary(name: &str, binary: &str) -> Result<(), ConfigError> {
    if binary.is_empty() {
        return Err(ConfigError::Invalid {
            message: format!("deploy '{name}': binary must not be empty"),
        });
    }
    if binary.starts_with('/') {
        return Err(ConfigError::Invalid {
            message: format!(
                "deploy '{name}': binary '{binary}' must be repo-relative, not absolute"
            ),
        });
    }
    for segment in binary.split('/') {
        if segment == ".." {
            return Err(ConfigError::Invalid {
                message: format!(
                    "deploy '{name}': binary '{binary}' contains '..' — path traversal is rejected"
                ),
            });
        }
    }
    Ok(())
}

/// Reject `install:` values that are not absolute paths.
fn validate_install(name: &str, install: &str) -> Result<(), ConfigError> {
    if install.is_empty() {
        return Err(ConfigError::Invalid {
            message: format!("deploy '{name}': install must not be empty"),
        });
    }
    if !install.starts_with('/') {
        return Err(ConfigError::Invalid {
            message: format!(
                "deploy '{name}': install '{install}' must be an absolute path (start with '/')"
            ),
        });
    }
    Ok(())
}

/// Validate the `backup:` block: destination shape, https endpoint,
/// non-empty paths, absolute + no-`..` paths, retention ≥ 1, and
/// every `stop_services` entry cross-references a declared service.
fn validate_backup(backup: &BackupConfig, config: &SetupConfig) -> Result<(), ConfigError> {
    validate_backup_destination(&backup.destination)?;
    validate_backup_endpoint(&backup.endpoint)?;
    validate_backup_paths(&backup.paths)?;
    validate_backup_retention(backup.retention_days)?;
    validate_stop_services(&backup.stop_services, config)?;
    Ok(())
}

/// `destination` must match `s3://<bucket>[/<prefix>]` — bucket
/// non-empty, only `[A-Za-z0-9._-]`, prefix must not contain `..`.
fn validate_backup_destination(destination: &str) -> Result<(), ConfigError> {
    let Some(rest) = destination.strip_prefix("s3://") else {
        return Err(ConfigError::Invalid {
            message: format!("destination must start with s3:// (got '{destination}')"),
        });
    };
    let (bucket, prefix) = match rest.split_once('/') {
        Some((b, p)) => (b, Some(p)),
        None => (rest, None),
    };
    if bucket.is_empty() {
        return Err(ConfigError::Invalid {
            message: "destination bucket must not be empty".to_owned(),
        });
    }
    for c in bucket.chars() {
        if !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
            return Err(ConfigError::Invalid {
                message: format!(
                    "destination bucket '{bucket}' contains invalid character '{c}' \
                     — allowed: [A-Za-z0-9._-]"
                ),
            });
        }
    }
    if let Some(prefix) = prefix {
        for segment in prefix.split('/') {
            if segment == ".." {
                return Err(ConfigError::Invalid {
                    message: format!(
                        "destination prefix contains '..' — path traversal is rejected (got '{destination}')"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// `endpoint` must start with `https://` — reject plaintext http and
/// schemeless strings.
fn validate_backup_endpoint(endpoint: &str) -> Result<(), ConfigError> {
    if !endpoint.starts_with("https://") {
        return Err(ConfigError::Invalid {
            message: format!("endpoint must be https:// (got '{endpoint}')"),
        });
    }
    Ok(())
}

/// `paths` must be non-empty, each entry absolute with no `..` segment.
fn validate_backup_paths(paths: &[String]) -> Result<(), ConfigError> {
    if paths.is_empty() {
        return Err(ConfigError::Invalid {
            message: "paths must not be empty".to_owned(),
        });
    }
    for path in paths {
        if !path.starts_with('/') {
            return Err(ConfigError::Invalid {
                message: format!("path must be absolute (got '{path}')"),
            });
        }
        for segment in path.split('/') {
            if segment == ".." {
                return Err(ConfigError::Invalid {
                    message: format!("path contains '..' — traversal is rejected (got '{path}')"),
                });
            }
        }
    }
    Ok(())
}

/// `retention_days` must be at least 1 — zero would prune every
/// archive immediately after upload.
fn validate_backup_retention(retention: u32) -> Result<(), ConfigError> {
    if retention < 1 {
        return Err(ConfigError::Invalid {
            message: format!("retention_days must be at least 1 (got {retention})"),
        });
    }
    Ok(())
}

/// Every entry in `stop_services` must appear in `setup.services` as
/// a `ServiceSpec.name`. Catches typos at load time rather than at
/// 3am when the timer fires.
fn validate_stop_services(
    stop_services: &[String],
    config: &SetupConfig,
) -> Result<(), ConfigError> {
    for svc in stop_services {
        if !config.setup.services.iter().any(|s| &s.name == svc) {
            return Err(ConfigError::Invalid {
                message: format!("stop_services: '{svc}' not declared in services"),
            });
        }
    }
    Ok(())
}
