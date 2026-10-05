//! `server.volumes:` — Hetzner block volumes created, attached, and
//! mounted by `harbor up`.
//!
//! A volume outlives its server: `harbor down` detaches it and keeps
//! the data, and the next `harbor up` reattaches the same volume by
//! name. Harbor formats a volume only when it carries no filesystem,
//! so a reattached volume is never wiped.

use serde::Deserialize;

use super::ConfigError;

/// Hetzner's minimum and maximum volume size in GB.
const MIN_SIZE_GB: u32 = 10;
const MAX_SIZE_GB: u32 = 10_240;

/// Mount points that would shadow the operating system.
const RESERVED_MOUNTS: &[&str] = &[
    "/", "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/root", "/run", "/sbin",
    "/sys", "/tmp", "/usr", "/var",
];

/// One block volume declared under `server.volumes`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeSpec {
    /// Volume name on Hetzner — unique per project, reused across
    /// `harbor up` runs to find the existing volume.
    pub name: String,
    /// Size in GB (Hetzner: 10–10240). Only used when creating; an
    /// existing volume is never resized by harbor.
    pub size: u32,
    /// Absolute mount point on the server, e.g. `/opt/tell`.
    pub mount: String,
    /// Filesystem written when the volume is blank.
    #[serde(default)]
    pub format: VolumeFormat,
}

/// Filesystem for a blank volume. Serializes as lowercase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeFormat {
    #[default]
    Ext4,
    Xfs,
}

impl VolumeFormat {
    /// Name as Hetzner and `mkfs.<fs>` spell it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ext4 => "ext4",
            Self::Xfs => "xfs",
        }
    }
}

/// Hetzner volume names: alphanumerics, `-`, `_`, `.`; must start and
/// end alphanumeric; at most 64 characters.
fn is_volume_name(name: &str) -> bool {
    let edge_ok = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    name.len() <= 64
        && edge_ok(name.chars().next())
        && edge_ok(name.chars().last())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A mount point: absolute, plain characters, no `..`, no trailing
/// slash, and not a system directory.
fn validate_mount(name: &str, mount: &str) -> Result<(), ConfigError> {
    let plain = mount.starts_with('/')
        && !mount.ends_with('/')
        && !mount.split('/').any(|seg| seg == ".." || seg == ".")
        && mount
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'));
    if !plain {
        return Err(invalid(format!(
            "volume '{name}': mount '{mount}' must be an absolute path without '..', \
             a trailing '/', spaces, or shell characters"
        )));
    }
    if RESERVED_MOUNTS.contains(&mount) {
        return Err(invalid(format!(
            "volume '{name}': mount '{mount}' would shadow a system directory"
        )));
    }
    Ok(())
}

/// Validate every volume, then reject duplicate names and mounts.
pub(super) fn validate_volumes(volumes: &[VolumeSpec]) -> Result<(), ConfigError> {
    for vol in volumes {
        if !is_volume_name(&vol.name) {
            return Err(invalid(format!(
                "volume name '{}' may only contain [A-Za-z0-9_.-], must start and end \
                 alphanumeric, and be at most 64 characters",
                vol.name
            )));
        }
        if !(MIN_SIZE_GB..=MAX_SIZE_GB).contains(&vol.size) {
            return Err(invalid(format!(
                "volume '{}': size {} GB is outside Hetzner's {MIN_SIZE_GB}–{MAX_SIZE_GB} GB",
                vol.name, vol.size
            )));
        }
        validate_mount(&vol.name, &vol.mount)?;
    }
    for (i, vol) in volumes.iter().enumerate() {
        let later = volumes.iter().skip(i + 1);
        if let Some(dup) = later.clone().find(|v| v.name == vol.name) {
            return Err(invalid(format!("volume '{}' is declared twice", dup.name)));
        }
        if let Some(dup) = later.clone().find(|v| v.mount == vol.mount) {
            return Err(invalid(format!(
                "volumes '{}' and '{}' share mount '{}'",
                vol.name, dup.name, vol.mount
            )));
        }
    }
    Ok(())
}

/// `system.journald_max_use`: a size like `500M` or `1G`, or empty.
pub(super) fn validate_journald_max_use(value: &str) -> Result<(), ConfigError> {
    if value.is_empty() {
        return Ok(());
    }
    let digits = value.trim_end_matches(['K', 'M', 'G', 'T']);
    let suffix_len = value.len() - digits.len();
    let ok = suffix_len <= 1
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && digits.parse::<u64>().is_ok_and(|n| n > 0);
    if !ok {
        return Err(invalid(format!(
            "system.journald_max_use '{value}' must be a size like 500M or 1G"
        )));
    }
    Ok(())
}

fn invalid(message: String) -> ConfigError {
    ConfigError::Invalid { message }
}
