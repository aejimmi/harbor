#![allow(clippy::unwrap_used)]

//! Tests for the `harbor status` bash renderer (spec 017).
//!
//! Covers the backup-block addition — shape when absent, shape
//! when present with the timer enabled, shape when present but
//! the timer has never been enabled.

use super::status_cmd::build_status_script;

#[test]
fn test_status_without_backup_block_is_unchanged() {
    let script = build_status_script(&["blissd"], None);
    // The backup section must not appear when no project is
    // declared.
    assert!(!script.contains("Backup:"));
    assert!(!script.contains("harbor-backup-"));
    // But existing blocks still render.
    assert!(script.contains("Deploy:"));
    assert!(script.contains("Uptime:"));
    assert!(script.contains("Disk:"));
    assert!(script.contains("Services:"));
}

#[test]
fn test_status_with_backup_block_appends_backup_section() {
    let script = build_status_script(&["blissd"], Some("blissd"));
    // Three lines land in the `is-enabled` branch.
    assert!(script.contains("echo 'Backup:'"));
    assert!(script.contains("TIMER=harbor-backup-blissd.timer"));
    assert!(script.contains("echo \"  state: $STATE\""));
    assert!(script.contains("echo \"  last:  ${LAST:-never}\""));
    assert!(script.contains("echo \"  next:  ${NEXT:-unknown}\""));
}

#[test]
fn test_status_with_backup_block_and_disabled_timer_prints_fallback() {
    let script = build_status_script(&[], Some("blissd"));
    // The `else` branch must cover timers that were never enabled.
    assert!(script.contains("backup timer not enabled on server"));
}

#[test]
fn test_status_section_order_deploy_then_uptime_then_disk_then_services_then_backup() {
    let script = build_status_script(&["blissd"], Some("blissd"));
    let deploy_idx = script.find("Deploy:").expect("deploy");
    let uptime_idx = script.find("Uptime:").expect("uptime");
    let disk_idx = script.find("Disk:").expect("disk");
    let services_idx = script.find("echo 'Services:'").expect("services");
    let backup_idx = script.find("echo 'Backup:'").expect("backup");
    assert!(deploy_idx < uptime_idx);
    assert!(uptime_idx < disk_idx);
    assert!(disk_idx < services_idx);
    assert!(services_idx < backup_idx);
}

#[test]
fn test_status_single_script_string_for_one_ssh_call() {
    // R2: the backup block lives in the same script string. This
    // test documents the contract — if someone refactors into
    // two calls, the assert below changes and forces a re-read
    // of the spec.
    let script = build_status_script(&["blissd"], Some("blissd"));
    // One rendered string, one SSH invocation downstream. Assert
    // that the returned `String` contains both the services block
    // and the backup block — confirming they share a render.
    assert!(script.contains("Services:"));
    assert!(script.contains("Backup:"));
}
