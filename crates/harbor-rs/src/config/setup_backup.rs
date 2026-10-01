//! `backup:` block types for `harbor.yaml`.

use serde::Deserialize;

/// Declarative backup configuration — what to archive, where to send
/// it, how often, and under which credentials.
///
/// Every field is validated post-parse by `setup_validate` (see R3 of
/// the 011 spec): destination shape, endpoint https-only, absolute
/// `paths` without `..`, `stop_services` cross-referenced to declared
/// `services:`, `retention_days` ≥ 1.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupConfig {
    /// Transport binary that ships archives to the S3 bucket. `rc`
    /// (the default) is pinned in-tree; `rclone` is the fallback.
    #[serde(default = "default_transport")]
    pub transport: BackupTransport,
    /// S3-style URI: `s3://<bucket>[/<prefix>]`.
    pub destination: String,
    /// S3-compatible HTTPS endpoint (e.g. R2's
    /// `https://<account>.r2.cloudflarestorage.com`).
    pub endpoint: String,
    /// When the systemd timer fires. One of `hourly | daily | weekly`.
    pub schedule: BackupSchedule,
    /// Remote archives older than this many days are pruned after a
    /// successful backup. Must be ≥ 1.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// Systemd service names to stop before archiving and start again
    /// after. Must cross-reference entries in `setup.services`.
    #[serde(default)]
    pub stop_services: Vec<String>,
    /// Absolute paths to archive. Each path must start with `/` and
    /// contain no `..` segments — validated at load.
    pub paths: Vec<String>,
}

/// Transport binary used to move archives to S3. Serializes as
/// lowercase (`rc`, `rclone`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupTransport {
    /// rustfs/cli — pure-Rust, pinned version installed by
    /// `RcInstallComponent`.
    #[default]
    Rc,
    /// rclone via the upstream install script. Fallback when `rc`
    /// doesn't fit the target bucket.
    Rclone,
}

/// When the backup timer fires. Serializes as lowercase
/// (`hourly | daily | weekly`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupSchedule {
    Hourly,
    Daily,
    Weekly,
}

fn default_transport() -> BackupTransport {
    BackupTransport::Rc
}

fn default_retention_days() -> u32 {
    14
}
