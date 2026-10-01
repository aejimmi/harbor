#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::full_backup_component;
use super::*;
use crate::config::BackupSchedule;

// R4: restore script

#[test]
fn test_restore_script_exists_with_755_mode() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("chmod 755 /usr/local/bin/harbor-restore-blissd"),
        "restore script must be 0755: {r}"
    );
}

#[test]
fn test_restore_tar_tzf_runs_before_any_systemctl_stop() {
    let r = full_backup_component().render().join("\n");
    let section = restore_section(&r);
    let tar_check = section.find("tar tzf").expect("tar tzf pre-flight missing");
    let first_stop = section
        .find("systemctl stop")
        .expect("stop line missing in restore");
    assert!(
        tar_check < first_stop,
        "tar tzf must run before any systemctl stop — corrupt archive must not touch live services"
    );
}

/// Return the body text of the restore script heredoc. Finding the
/// close marker via a second `find` after the opener's `<<` skips
/// past the heredoc token on the opener line.
fn restore_section(rendered: &str) -> &str {
    let opener = rendered
        .find("harbor-restore-blissd <<")
        .expect("restore heredoc opener");
    // Advance past the opener line so the subsequent
    // `RESTORE_SCRIPT_EOF` find lands on the close marker, not on
    // the same line.
    let after_opener = rendered[opener..]
        .find('\n')
        .map_or(opener, |nl| opener + nl + 1);
    let close_rel = rendered[after_opener..]
        .find("RESTORE_SCRIPT_EOF")
        .expect("restore close marker");
    &rendered[after_opener..after_opener + close_rel]
}

#[test]
fn test_restore_uses_mv_minus_t_for_every_swap() {
    let r = full_backup_component().render().join("\n");
    // Count `mv -T` occurrences — every swap uses it.
    let count = r.matches("mv -T").count();
    // Two paths, two swaps per path (old + new), plus restore's
    // rollback path = at least 4 occurrences. No bare `mv` in swap logic.
    assert!(
        count >= 4,
        "expected at least 4 'mv -T' invocations for two paths, found {count}: {r}"
    );
}

#[test]
fn test_restore_has_both_timestamp_present_and_absent_branches() {
    let r = full_backup_component().render().join("\n");
    let section = restore_section(&r);
    assert!(
        section.contains("if [ -n \"${1:-}\" ]"),
        "restore must branch on $1: {section}"
    );
    assert!(
        section.contains("$1.tar.gz"),
        "restore must use $1 in explicit branch: {section}"
    );
    assert!(
        section.contains("sort -r") && section.contains("head -n 1"),
        "restore must resolve newest when $1 absent: {section}"
    );
}

#[test]
fn test_restore_failure_after_stop_triggers_rollback_and_restart() {
    let r = full_backup_component().render().join("\n");
    let section = restore_section(&r);
    // Rollback: move `.old-*` back + start services best-effort.
    assert!(
        section.contains("RESTORED[@]") || section.contains("RESTORED[*]"),
        "restore must track restored paths for rollback: {section}"
    );
    assert!(
        section.contains("systemctl start"),
        "rollback path must restart services: {section}"
    );
}

// R5: service unit

#[test]
fn test_backup_service_unit_type_oneshot() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("Type=oneshot"),
        "service unit must be oneshot: {r}"
    );
}

#[test]
fn test_backup_service_unit_network_online_references() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("After=network-online.target"),
        "service unit After=network-online.target: {r}"
    );
    assert!(
        r.contains("Wants=network-online.target"),
        "service unit Wants=network-online.target: {r}"
    );
}

#[test]
fn test_backup_service_unit_has_no_install_section() {
    let r = full_backup_component().render().join("\n");
    let service_start = r
        .find("harbor-backup-blissd.service << 'EOF'")
        .expect("service heredoc");
    // Advance past the opener line so the next \nEOF lands on the
    // matching close marker, not on the opener's `<< 'EOF'` text.
    let nl = rendered_nl(&r, service_start);
    let after_opener = service_start + nl + 1;
    let rest = &r[after_opener..];
    let eof_idx = rest.find("\nEOF").expect("service close marker");
    let service_block = &rest[..eof_idx];
    assert!(
        !service_block.contains("[Install]"),
        "service unit must not have an [Install] section: {service_block}"
    );
}

/// Tiny helper — newline offset from `start`. Returns 0 on
/// pathological inputs; every caller here has a newline after the
/// opener by construction.
fn rendered_nl(rendered: &str, start: usize) -> usize {
    rendered[start..].find('\n').unwrap_or(0)
}

#[test]
fn test_backup_service_unit_execstart_points_at_script() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("ExecStart=/usr/local/bin/harbor-backup-blissd"),
        "service unit ExecStart must match rendered script path: {r}"
    );
}

// R6: timer unit

#[test]
fn test_backup_timer_oncalendar_matches_hourly_variant() {
    let mut c = full_backup_component();
    c.schedule = BackupSchedule::Hourly;
    let r = c.render().join("\n");
    assert!(r.contains("OnCalendar=hourly"), "hourly → 'hourly': {r}");
}

#[test]
fn test_backup_timer_oncalendar_matches_daily_variant() {
    let mut c = full_backup_component();
    c.schedule = BackupSchedule::Daily;
    let r = c.render().join("\n");
    assert!(r.contains("OnCalendar=daily"), "daily → 'daily': {r}");
}

#[test]
fn test_backup_timer_oncalendar_matches_weekly_variant() {
    let mut c = full_backup_component();
    c.schedule = BackupSchedule::Weekly;
    let r = c.render().join("\n");
    assert!(r.contains("OnCalendar=weekly"), "weekly → 'weekly': {r}");
}

#[test]
fn test_backup_timer_has_randomized_delay_and_persistent() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("RandomizedDelaySec=1h"),
        "timer must randomise delay: {r}"
    );
    assert!(
        r.contains("Persistent=yes"),
        "timer must persist across missed fires: {r}"
    );
}

#[test]
fn test_backup_timer_installs_under_timers_target_and_points_at_service() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("WantedBy=timers.target"),
        "timer must install under timers.target: {r}"
    );
    assert!(
        r.contains("Unit=harbor-backup-blissd.service"),
        "timer must reference the matching service unit: {r}"
    );
}

// R7: enable lines

#[test]
fn test_backup_enable_includes_daemon_reload() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("systemctl daemon-reload"),
        "enable block must daemon-reload: {r}"
    );
}

#[test]
fn test_backup_enable_only_enables_the_timer() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("systemctl enable --now harbor-backup-blissd.timer"),
        "enable block must target the timer: {r}"
    );
}
