#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

//! Unit tests for the `harbor backup` helpers (spec 015).
//!
//! Does not exercise the full SSH flow — that lives in
//! acceptance-test territory. Covers script shape and the
//! `rc object list` parser.

use super::backup_cmd::{
    build_backup_script, build_list_script, matches_project_key, parse_list_output,
};

// R2: backup script shape

#[test]
fn test_backup_script_uses_systemctl_start_wait() {
    let s = build_backup_script("blissd");
    assert!(s.contains("UNIT=harbor-backup-blissd.service"));
    assert!(s.contains("systemctl start --wait \"$UNIT\""));
    // Real exit-code check: read the unit's Result property so a
    // failed oneshot can't be masked by a trailing pipe.
    assert!(
        s.contains("systemctl show -p Result --value"),
        "must explicitly read Result to detect failures: {s}"
    );
    assert!(
        s.contains("journalctl -u \"$UNIT\""),
        "must dump the journal on failure: {s}"
    );
}

#[test]
fn test_backup_script_sets_set_e() {
    let s = build_backup_script("blissd");
    assert!(s.contains("set -e"));
}

// R3: list script branches on env-file transport

#[test]
fn test_list_script_sources_env_and_branches_on_transport() {
    let s = build_list_script();
    assert!(s.contains("source /etc/harbor/backup.env"));
    assert!(s.contains("BACKUP_TRANSPORT"));
    // Credentials stay in the env file — never on a command line.
    assert!(
        !s.contains("rc alias set"),
        "alias is set at provision: {s}"
    );
    assert!(!s.contains("secret_access_key="), "{s}");
    assert!(!s.contains("\"$AWS_SECRET_ACCESS_KEY\""), "{s}");
    assert!(
        s.contains("rc object list"),
        "rc path must use rc object list: {s}"
    );
    assert!(s.contains("rclone lsl"));
    // Guard against regression to the deprecated flat form.
    assert!(!s.contains("--endpoint-url"));
    assert!(!s.contains("rc ls "));
}

// R3: list parser

#[test]
fn test_parse_list_output_empty_bucket() {
    let rows = parse_list_output("", "blissd");
    assert!(rows.is_empty());
}

#[test]
fn test_parse_list_output_sorts_newest_first() {
    let raw = "\
100  2026-04-01  blissd-20260401T120000Z.tar.gz
200  2026-04-20  blissd-20260420T030000Z.tar.gz
150  2026-04-10  blissd-20260410T000000Z.tar.gz
";
    let rows = parse_list_output(raw, "blissd");
    assert_eq!(rows.len(), 3);
    assert!(
        rows[0].key.contains("20260420T030000Z"),
        "newest first: {:?}",
        rows[0].key
    );
    assert!(
        rows[2].key.contains("20260401T120000Z"),
        "oldest last: {:?}",
        rows[2].key
    );
}

#[test]
fn test_parse_list_output_filters_unrelated_keys() {
    let raw = "\
100  date  blissd-20260420T030000Z.tar.gz
50   date  other-project-20260420T030000Z.tar.gz
25   date  random-file.txt
10   date  blissd-backup-20260420T030000Z.tar.gz
";
    let rows = parse_list_output(raw, "blissd");
    // Only the single blissd-<timestamp>.tar.gz key matches.
    // `blissd-backup-...` is NOT filtered in because its timestamp
    // suffix after the `-backup-` prefix still produces a bogus
    // name when split on the last '-' (leaving an invalid ts);
    // matches_project_key tolerates only the exact shape.
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "blissd-20260420T030000Z.tar.gz");
}

#[test]
fn test_matches_project_key_strict() {
    assert!(matches_project_key(
        "blissd-20260420T030000Z.tar.gz",
        "blissd"
    ));
    // Wrong length
    assert!(!matches_project_key("blissd-20260420T.tar.gz", "blissd"));
    // Missing Z
    assert!(!matches_project_key(
        "blissd-20260420T030000.tar.gz",
        "blissd"
    ));
    // Wrong project
    assert!(!matches_project_key(
        "api-20260420T030000Z.tar.gz",
        "blissd"
    ));
    // Non-digit in ts
    assert!(!matches_project_key(
        "blissd-2026XY20T030000Z.tar.gz",
        "blissd"
    ));
}

#[test]
fn test_parse_list_output_handles_lines_with_trailing_whitespace() {
    let raw = "\
100  2026  blissd-20260420T030000Z.tar.gz  \n";
    let rows = parse_list_output(raw, "blissd");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].size, "100");
}
