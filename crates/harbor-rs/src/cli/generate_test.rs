#![allow(clippy::panic, clippy::unwrap_used)]

//! Tests for the `harbor generate` pre-flight (spec 014 R5).
//!
//! These exercise the credential-check wiring without shelling out
//! — they write fixture YAMLs into a `TempDir` and invoke
//! `generate::run` directly.

use std::fs;

use tempfile::TempDir;

use super::generate;

#[test]
fn test_generate_without_backup_block_is_unchanged() {
    let dir = TempDir::new().expect("tempdir");
    let setup = dir.path().join("harbor.yaml");
    fs::write(&setup, "name: any\nsetup:\n  packages: [git]\n").expect("write");
    // No user config needed when backup is absent.
    let result = generate::run(&setup, None, None);
    assert!(
        result.is_ok(),
        "generate must succeed when backup is absent: {result:?}"
    );
}

#[test]
fn test_generate_with_backup_missing_user_config_errors() {
    let dir = TempDir::new().expect("tempdir");
    let setup = dir.path().join("harbor.yaml");
    fs::write(
        &setup,
        r"
name: blissd
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    destination: s3://bucket
    endpoint: https://x
    schedule: daily
    stop_services: [blissd]
    paths: [/opt/blissd]
",
    )
    .expect("write");
    let user = dir.path().join("user.yaml");
    // Create an empty-but-present user config so UserConfig::load
    // succeeds and the error surfaces from require_backup_creds,
    // not from `loading user config`.
    fs::write(&user, "hetzner:\n  token: x\n").expect("write");

    let err = generate::run(&setup, None, Some(&user)).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("blissd"), "error must name the project: {msg}");
    assert!(
        msg.contains("~/.harbor/config.yaml"),
        "error must point to the user config path: {msg}"
    );
}

#[test]
fn test_generate_with_backup_and_valid_creds_succeeds() {
    let dir = TempDir::new().expect("tempdir");
    let setup = dir.path().join("harbor.yaml");
    fs::write(
        &setup,
        r"
name: blissd
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    destination: s3://bucket
    endpoint: https://x
    schedule: daily
    stop_services: [blissd]
    paths: [/opt/blissd]
",
    )
    .expect("write");
    let user = dir.path().join("user.yaml");
    fs::write(
        &user,
        r#"
hetzner:
  token: x
backup:
  projects:
    blissd:
      access_key_id: "AKID"
      secret_access_key: "SECRET"
"#,
    )
    .expect("write");

    generate::run(&setup, None, Some(&user)).expect("generate should succeed with creds present");
}
