//! Shared fixtures for the split test modules.

/// Minimal `backup:` block used by several happy-path parse tests.
pub(super) const FULL_BACKUP_YAML: &str = r"
name: blissd
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    transport: rc
    destination: s3://bucket/prefix
    endpoint: https://account.r2.cloudflarestorage.com
    schedule: daily
    retention_days: 7
    stop_services: [blissd]
    paths:
      - /opt/blissd/db
      - /opt/blissd/audio
";
