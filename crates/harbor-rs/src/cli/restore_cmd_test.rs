#![allow(clippy::unwrap_used)]

//! Unit tests for the `harbor restore` helpers (spec 016).
//!
//! The full end-to-end flow (SSH + provisioner) lives in
//! acceptance-test territory. These cover the pure functions —
//! script rendering and pre-flight summary.

use super::restore_cmd::{build_restore_script, print_preflight_summary};

#[test]
fn test_restore_script_invokes_harbor_restore_with_explicit_timestamp() {
    let s = build_restore_script("blissd", "20260420T030000Z");
    assert!(s.contains("/usr/local/bin/harbor-restore-blissd"));
    assert!(s.contains("20260420T030000Z"));
    assert!(s.contains("set -e"));
}

#[test]
fn test_restore_preflight_summary_runs_without_panic() {
    // The function writes to stderr via `output::info`. This test
    // just exercises the code path — it doesn't capture output.
    print_preflight_summary(
        "blissd",
        "20260420T030000Z",
        &["blissd".to_owned()],
        &["/opt/blissd/db".to_owned(), "/opt/blissd/audio".to_owned()],
    );
}
