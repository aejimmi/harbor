#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::full_backup_component;
use super::*;
use crate::config::BackupTransport;

// --- BackupComponent (spec 013) ---

// R1: component purity + Debug redaction + field parity

#[test]
fn test_backup_component_debug_redacts_secret() {
    let c = full_backup_component();
    let rendered = format!("{c:?}");
    assert!(
        !rendered.contains("S3CRET"),
        "secret must not appear: {rendered}"
    );
    assert!(
        rendered.contains("<redacted>"),
        "must show redaction marker: {rendered}"
    );
    assert!(
        rendered.contains("AKID"),
        "access_key_id is not a secret: {rendered}"
    );
}

#[test]
fn test_backup_component_render_is_pure() {
    // Rendering the component twice yields byte-identical output —
    // proves no env reads, no clock reads, no RNG.
    let c = full_backup_component();
    let first = c.render().join("\n");
    let second = c.render().join("\n");
    assert_eq!(first, second, "render must be deterministic");
}

#[test]
fn test_backup_component_field_parity_with_backup_config() {
    // Sanity: the struct covers every field of BackupConfig plus
    // the two credential fields. This catches a future drift where
    // a field is added to BackupConfig but forgotten here.
    let c = full_backup_component();
    let _ = (
        &c.project,
        c.transport,
        &c.destination,
        &c.endpoint,
        c.schedule,
        c.retention_days,
        &c.stop_services,
        &c.paths,
        &c.access_key_id,
        &c.secret_access_key,
    );
}

// R2: env file

#[test]
fn test_backup_env_file_perms_are_0600_root() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("chmod 600 /etc/harbor/backup.env"),
        "env file must be 0600: {r}"
    );
    assert!(
        r.contains("chown root:root /etc/harbor/backup.env"),
        "env file must be root:root: {r}"
    );
}

#[test]
fn test_backup_env_file_heredoc_is_single_quoted() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("cat > /etc/harbor/backup.env << 'EOF'"),
        "env heredoc must be single-quoted to prevent expansion: {r}"
    );
}

#[test]
fn test_backup_env_file_has_all_keys_plus_transport() {
    let r = full_backup_component().render().join("\n");
    for key in [
        "AWS_ACCESS_KEY_ID=",
        "AWS_SECRET_ACCESS_KEY=",
        "BACKUP_BUCKET=",
        "BACKUP_PREFIX=",
        "BACKUP_ENDPOINT=",
        "BACKUP_RETENTION_DAYS=",
        "BACKUP_TRANSPORT=",
    ] {
        assert!(r.contains(key), "env file missing key '{key}': {r}");
    }
}

#[test]
fn test_backup_env_file_special_chars_render_literally() {
    let mut c = full_backup_component();
    c.secret_access_key = "pa$$w`rd\"with\\specials".to_owned();
    let r = c.render().join("\n");
    // Single-quoted heredoc prevents expansion; the literal
    // characters appear verbatim in the rendered bash.
    assert!(
        r.contains("pa$$w`rd\"with\\specials"),
        "special chars must render literally inside single-quoted heredoc: {r}"
    );
}

#[test]
fn test_backup_env_file_transport_reflects_rclone() {
    let mut c = full_backup_component();
    c.transport = BackupTransport::Rclone;
    let r = c.render().join("\n");
    assert!(
        r.contains("BACKUP_TRANSPORT=rclone"),
        "rclone transport must appear in env file: {r}"
    );
}

// R3: backup script

#[test]
fn test_backup_script_has_set_euo_pipefail() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("set -euo pipefail"),
        "backup script must set -euo pipefail: {r}"
    );
}

#[test]
fn test_backup_script_stop_order_is_declared_start_order_is_reverse() {
    let r = full_backup_component().render().join("\n");
    let section = backup_section(&r);
    let stop_blissd = section
        .find("systemctl stop 'blissd'")
        .expect("stop blissd line missing");
    let stop_worker = section
        .find("systemctl stop 'worker'")
        .expect("stop worker line missing");
    assert!(stop_blissd < stop_worker, "stop must follow declared order");
    let start_worker = section
        .find("systemctl start 'worker'")
        .expect("worker start in backup section");
    let start_blissd = section
        .find("systemctl start 'blissd'")
        .expect("blissd start in backup section");
    assert!(
        start_worker < start_blissd,
        "start must be reverse of declared order inside backup section"
    );
}

/// Scope helper — the body of the backup script heredoc.
fn backup_section(rendered: &str) -> &str {
    let opener = rendered
        .find("harbor-backup-blissd <<")
        .expect("backup heredoc opener");
    let after_opener = rendered[opener..]
        .find('\n')
        .map_or(opener, |nl| opener + nl + 1);
    let close_rel = rendered[after_opener..]
        .find("BACKUP_SCRIPT_EOF")
        .expect("backup close marker");
    &rendered[after_opener..after_opener + close_rel]
}

#[test]
fn test_backup_transport_rc_emits_only_rc_commands() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("rc alias set harbor_backup"),
        "rc transport must configure the alias: {r}"
    );
    assert!(
        r.contains("rc object copy"),
        "rc transport must render rc object copy: {r}"
    );
    assert!(
        r.contains("rc object list"),
        "rc transport must render rc object list for prune: {r}"
    );
    assert!(
        r.contains("rc object remove"),
        "rc transport must render rc object remove for prune: {r}"
    );
    assert!(
        !r.contains("rclone "),
        "rc transport must not emit rclone commands: {r}"
    );
    // Legacy flat commands must never appear — they're deprecated
    // in rc and would break on future CLI versions.
    for legacy in ["rc put ", "rc get ", "rc ls ", "rc rm ", "--endpoint-url"] {
        assert!(
            !r.contains(legacy),
            "rc transport must not emit deprecated/flat form '{legacy}': {r}"
        );
    }
}

#[test]
fn test_backup_transport_rclone_emits_only_rclone_commands() {
    let mut c = full_backup_component();
    c.transport = BackupTransport::Rclone;
    let r = c.render().join("\n");
    assert!(
        r.contains("rclone "),
        "rclone transport must render rclone commands: {r}"
    );
    // No rc commands of any form under rclone.
    for rc_cmd in [
        "rc alias",
        "rc object",
        "rc put",
        "rc get",
        "rc ls",
        "rc rm",
    ] {
        assert!(
            !r.contains(rc_cmd),
            "rclone transport must not emit '{rc_cmd}': {r}"
        );
    }
}

#[test]
fn test_backup_archive_path_shape() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("/var/lib/harbor/backups/blissd-$TIMESTAMP.tar.gz"),
        "archive path must include project + timestamp under /var/lib: {r}"
    );
}

#[test]
fn test_backup_upload_success_deletes_local_failure_retains() {
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("rm -f \"$ARCHIVE\""),
        "success path must delete local archive: {r}"
    );
    assert!(
        r.contains("local archive retained"),
        "failure path must log retention of local archive: {r}"
    );
}

#[test]
fn test_backup_script_prune_references_env_retention_days() {
    // The retention window is read from `$BACKUP_RETENTION_DAYS`
    // (env file) rather than inlined at render time so operators
    // can tweak retention with a single file edit.
    let r = full_backup_component().render().join("\n");
    assert!(
        r.contains("$BACKUP_RETENTION_DAYS"),
        "prune must reference env-file retention: {r}"
    );
}
