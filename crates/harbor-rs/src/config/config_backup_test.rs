#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]

use super::config_test_helpers::FULL_BACKUP_YAML;
use super::*;

// --- Backup config schema (spec 011 R1) ---

#[test]
fn test_backup_absent_parses_to_none() {
    let yaml = "setup:\n  packages: []\n";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    assert!(config.setup.backup.is_none());
}

#[test]
fn test_backup_full_block_parses() {
    let config: SetupConfig = serde_yaml::from_str(FULL_BACKUP_YAML).expect("parse");
    let backup = config.setup.backup.as_ref().expect("backup present");
    assert_eq!(backup.transport, BackupTransport::Rc);
    assert_eq!(backup.destination, "s3://bucket/prefix");
    assert_eq!(backup.endpoint, "https://account.r2.cloudflarestorage.com");
    assert_eq!(backup.schedule, BackupSchedule::Daily);
    assert_eq!(backup.retention_days, 7);
    assert_eq!(backup.stop_services, vec!["blissd".to_owned()]);
    assert_eq!(backup.paths.len(), 2);
    assert_eq!(backup.paths[0], "/opt/blissd/db");
}

#[test]
fn test_backup_transport_defaults_to_rc() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: hourly
    paths: [/a]
";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let backup = config.setup.backup.as_ref().expect("backup present");
    assert_eq!(backup.transport, BackupTransport::Rc);
}

#[test]
fn test_backup_retention_defaults_to_14() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: weekly
    paths: [/a]
";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let backup = config.setup.backup.as_ref().expect("backup present");
    assert_eq!(backup.retention_days, 14);
}

#[test]
fn test_backup_missing_schedule_serde_error() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    paths: [/a]
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("schedule"),
        "missing schedule should be named: {msg}"
    );
}

#[test]
fn test_backup_missing_destination_serde_error() {
    let yaml = r"
name: p
setup:
  backup:
    endpoint: https://x
    schedule: daily
    paths: [/a]
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("destination"),
        "missing destination should be named: {msg}"
    );
}

#[test]
fn test_backup_missing_endpoint_serde_error() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    schedule: daily
    paths: [/a]
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("endpoint"),
        "missing endpoint should be named: {msg}"
    );
}

#[test]
fn test_backup_missing_paths_serde_error() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("paths"),
        "missing paths should be named: {msg}"
    );
}

#[test]
fn test_backup_unknown_field_rejected() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: daily
    paths: [/a]
    surprise: true
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    assert!(
        err.is_err(),
        "unknown field should fail via deny_unknown_fields"
    );
}

#[test]
fn test_backup_unknown_transport_rejected() {
    let yaml = r"
name: p
setup:
  backup:
    transport: ftp
    destination: s3://b
    endpoint: https://x
    schedule: daily
    paths: [/a]
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    assert!(err.is_err(), "unknown transport should fail");
}

#[test]
fn test_backup_unknown_schedule_rejected() {
    let yaml = r"
name: p
setup:
  backup:
    destination: s3://b
    endpoint: https://x
    schedule: monthly
    paths: [/a]
";
    let err: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    assert!(err.is_err(), "unknown schedule should fail");
}

// --- BackupCredentialsMap (spec 011 R2) ---

#[test]
fn test_user_config_without_backup_block_parses() {
    let yaml = "hetzner:\n  token: x\n";
    let config: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    assert!(config.backup.projects.is_empty());
}

#[test]
fn test_user_config_populated_backup_lookup() {
    let yaml = r#"
hetzner:
  token: x
backup:
  projects:
    blissd:
      access_key_id: "AKID"
      secret_access_key: "SECRET"
    api:
      access_key_id: "AKID2"
      secret_access_key: "SECRET2"
"#;
    let config: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(config.backup.projects.len(), 2);
    let blissd = config.backup.for_project("blissd").expect("blissd present");
    assert_eq!(blissd.access_key_id, "AKID");
    assert_eq!(blissd.secret_access_key, "SECRET");
    let api = config.backup.for_project("api").expect("api present");
    assert_eq!(api.access_key_id, "AKID2");
}

#[test]
fn test_user_config_backup_lookup_absent_project_returns_none() {
    let yaml = r#"
backup:
  projects:
    blissd:
      access_key_id: "A"
      secret_access_key: "S"
"#;
    let config: UserConfig = serde_yaml::from_str(yaml).expect("parse");
    assert!(config.backup.for_project("absent").is_none());
}

#[test]
fn test_backup_credentials_debug_redacts_secret() {
    let creds = BackupCredentials {
        access_key_id: "AKID".to_owned(),
        secret_access_key: "topsecret".to_owned(),
    };
    let rendered = format!("{creds:?}");
    assert!(
        !rendered.contains("topsecret"),
        "secret must not appear: {rendered}"
    );
    assert!(
        rendered.contains("<redacted>"),
        "must show redaction marker: {rendered}"
    );
    assert!(
        rendered.contains("AKID"),
        "access_key_id is an identifier, keep it: {rendered}"
    );
}
