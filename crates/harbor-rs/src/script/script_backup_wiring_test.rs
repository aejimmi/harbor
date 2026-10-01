#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::full_backup_component;
use super::*;
use crate::config::{BackupTransport, SetupConfig};
use std::path::Path;

// --- from_setup_config wiring (spec 014) ---

const BACKUP_YAML_RC: &str = r"
name: blissd
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    transport: rc
    destination: s3://bucket
    endpoint: https://x.r2.cloudflarestorage.com
    schedule: daily
    stop_services: [blissd]
    paths: [/opt/blissd]
";

const BACKUP_YAML_RCLONE: &str = r"
name: blissd
setup:
  services:
    - name: blissd
      enabled: true
  backup:
    transport: rclone
    destination: s3://bucket
    endpoint: https://x.r2.cloudflarestorage.com
    schedule: daily
    stop_services: [blissd]
    paths: [/opt/blissd]
";

fn test_creds() -> crate::config::BackupCredentials {
    crate::config::BackupCredentials {
        access_key_id: "AKID".to_owned(),
        secret_access_key: "SECRET".to_owned(),
    }
}

#[test]
fn test_from_setup_config_no_backup_adds_no_backup_components() {
    let yaml = "setup:\n  packages: [git]\n";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    // Even when creds are somehow provided, an absent backup block
    // must not trigger any backup component.
    let creds = test_creds();
    let script = ScriptBuilder::from_setup_config(&config, "", Path::new("."), Some(&creds))
        .expect("build")
        .build();
    assert!(!script.contains("/etc/harbor/backup.env"));
    assert!(!script.contains("harbor-backup-"));
    assert!(!script.contains("/usr/local/bin/rc"));
}

#[test]
fn test_from_setup_config_backup_present_missing_creds_errors() {
    let config: SetupConfig = serde_yaml::from_str(BACKUP_YAML_RC).expect("parse");
    let result = ScriptBuilder::from_setup_config(&config, "", Path::new("."), None);
    let Err(err) = result else {
        panic!("expected Err when backup is declared without creds");
    };
    let msg = format!("{err}");
    assert!(msg.contains("blissd"), "error must name the project: {msg}");
    assert!(
        msg.contains("~/.harbor/config.yaml"),
        "error must point to the user config: {msg}"
    );
}

#[test]
fn test_from_setup_config_rc_transport_adds_rc_install() {
    let config: SetupConfig = serde_yaml::from_str(BACKUP_YAML_RC).expect("parse");
    let creds = test_creds();
    let script = ScriptBuilder::from_setup_config(&config, "", Path::new("."), Some(&creds))
        .expect("build")
        .build();
    assert!(
        script.contains(super::rc_install::RC_VERSION),
        "rc install component must be wired when transport: rc"
    );
    assert!(
        !script.contains("rclone.org/install.sh"),
        "rclone install must not appear when transport: rc"
    );
}

#[test]
fn test_from_setup_config_rclone_transport_adds_rclone_install() {
    let config: SetupConfig = serde_yaml::from_str(BACKUP_YAML_RCLONE).expect("parse");
    let creds = test_creds();
    let script = ScriptBuilder::from_setup_config(&config, "", Path::new("."), Some(&creds))
        .expect("build")
        .build();
    assert!(
        script.contains("rclone.org/install.sh"),
        "rclone install component must be wired when transport: rclone"
    );
    assert!(
        !script.contains(super::rc_install::RC_VERSION),
        "rc install must not appear when transport: rclone"
    );
}

#[test]
fn test_from_setup_config_backup_ordering_transport_before_backup_component() {
    let config: SetupConfig = serde_yaml::from_str(BACKUP_YAML_RC).expect("parse");
    let creds = test_creds();
    let script = ScriptBuilder::from_setup_config(&config, "", Path::new("."), Some(&creds))
        .expect("build")
        .build();
    let rc_install_idx = script
        .find("Installing rc ")
        .expect("rc install status line missing");
    let backup_env_idx = script
        .find("/etc/harbor/backup.env")
        .expect("backup env line missing");
    assert!(
        rc_install_idx < backup_env_idx,
        "transport install must render before BackupComponent — \
         backup scripts would otherwise invoke a non-existent binary"
    );
}

#[test]
fn test_backup_enable_never_enables_the_service_unit() {
    let r = full_backup_component().render().join("\n");
    // No `systemctl enable ... .service` line should reference
    // the backup service — the timer owns the enable state.
    assert!(
        !r.contains("systemctl enable harbor-backup-blissd.service"),
        "service unit must not be enabled — timer owns enable state: {r}"
    );
    assert!(
        !r.contains("systemctl enable --now harbor-backup-blissd.service"),
        "service unit must not be enabled: {r}"
    );
}

#[test]
fn test_backup_scripts_keep_secrets_off_command_lines() {
    for transport in [BackupTransport::Rc, BackupTransport::Rclone] {
        let mut c = full_backup_component();
        c.transport = transport;
        let r = c.render().join("\n");
        let backup = r.split("BACKUP_SCRIPT_EOF").nth(1).expect("backup script");
        let restore = r
            .split("RESTORE_SCRIPT_EOF")
            .nth(1)
            .expect("restore script");
        for script in [backup, restore] {
            assert!(!script.contains("$AWS_SECRET_ACCESS_KEY"), "{script}");
            assert!(!script.contains("rc alias set"), "{script}");
        }
    }
}
