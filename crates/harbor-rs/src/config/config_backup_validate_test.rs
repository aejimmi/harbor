#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]

use super::config_test_helpers::FULL_BACKUP_YAML;
use super::*;
use std::fs;
use tempfile::TempDir;

// --- setup_validate::validate_backup (spec 011 R3) ---

fn write_setup(dir: &TempDir, yaml: &str) -> std::path::PathBuf {
    let path = dir.path().join("harbor.yaml");
    fs::write(&path, yaml).expect("write");
    path
}

#[test]
fn test_backup_valid_config_passes_validation() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(&dir, FULL_BACKUP_YAML);
    let _ = SetupConfig::load(&path).expect("load");
}

#[test]
fn test_backup_destination_not_s3_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: https://bucket
    endpoint: https://x
    schedule: daily
    paths: [/a]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("s3://"),
        "error should cite s3:// requirement: {msg}"
    );
}

#[test]
fn test_backup_destination_empty_bucket_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: 's3://'
    endpoint: https://x
    schedule: daily
    paths: [/a]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("bucket"),
        "error should mention empty bucket: {msg}"
    );
}

#[test]
fn test_backup_destination_prefix_traversal_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: s3://b/../etc
    endpoint: https://x
    schedule: daily
    paths: [/a]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(msg.contains(".."), "error should cite '..': {msg}");
}

#[test]
fn test_backup_endpoint_http_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: http://insecure
    schedule: daily
    paths: [/a]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("https"),
        "error should cite https requirement: {msg}"
    );
}

#[test]
fn test_backup_empty_paths_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    paths: []
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("paths"), "error should cite paths: {msg}");
}

#[test]
fn test_backup_relative_path_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    paths: [relative/path]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("absolute"),
        "error should require absolute path: {msg}"
    );
}

#[test]
fn test_backup_path_with_traversal_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    paths: [/a/../b]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(msg.contains(".."), "error should cite '..': {msg}");
}

#[test]
fn test_backup_retention_zero_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    retention_days: 0
    paths: [/a]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("retention_days"),
        "error should cite retention_days: {msg}"
    );
}

#[test]
fn test_backup_stop_services_unknown_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    stop_services: [ghost]
    paths: [/a]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("ghost"),
        "error should name the unknown service: {msg}"
    );
}

#[test]
fn test_backup_stop_services_cross_reference_passes() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_setup(
        &dir,
        r"
name: p
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    stop_services: [blissd]
    paths: [/a]
",
    );
    let _ = SetupConfig::load(&path).expect("load");
}

// --- require_backup_creds helper (spec 011 R4) ---

fn setup_config_with_backup() -> SetupConfig {
    serde_yaml::from_str::<SetupConfig>(FULL_BACKUP_YAML).expect("parse")
}

fn setup_config_no_backup() -> SetupConfig {
    serde_yaml::from_str::<SetupConfig>("name: other\nsetup:\n  packages: []\n").expect("parse")
}

#[test]
fn test_require_backup_creds_no_backup_returns_ok() {
    let setup = setup_config_no_backup();
    let yaml = "hetzner:\n  token: x\n";
    let user: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    super::require_backup_creds(&setup, &user)
        .expect("should not require creds when backup absent");
}

#[test]
fn test_require_backup_creds_with_creds_returns_ok() {
    let setup = setup_config_with_backup();
    let yaml = r#"
hetzner:
  token: x
backup:
  projects:
    blissd:
      access_key_id: "A"
      secret_access_key: "S"
"#;
    let user: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    super::require_backup_creds(&setup, &user).expect("creds present must pass");
}

#[test]
fn test_require_backup_creds_missing_creds_errors() {
    let setup = setup_config_with_backup();
    let yaml = "hetzner:\n  token: x\n";
    let user: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    let err = super::require_backup_creds(&setup, &user).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("blissd"),
        "error should name the project: {msg}"
    );
    assert!(
        msg.contains("~/.harbor/config.yaml"),
        "error should name the user config path: {msg}"
    );
}

#[test]
fn test_require_backup_creds_empty_secret_errors() {
    let setup = setup_config_with_backup();
    let yaml = r#"
backup:
  projects:
    blissd:
      access_key_id: "A"
      secret_access_key: ""
"#;
    let user: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    let err = super::require_backup_creds(&setup, &user).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("blissd"),
        "error should name the project: {msg}"
    );
}
