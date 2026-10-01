#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::*;

// --- rc_install + rclone_install (spec 012) ---

#[test]
fn test_rc_install_contains_pinned_version_string() {
    let rendered = RcInstallComponent.render().join("\n");
    assert!(
        rendered.contains(super::rc_install::RC_VERSION),
        "rendered bash must mention the pinned RC_VERSION: {rendered}"
    );
}

#[test]
fn test_rc_install_single_install_line_with_0755_root() {
    let lines = RcInstallComponent.render();
    let install_lines: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("install -m 0755 -o root -g root /tmp/rc /usr/local/bin/rc"))
        .collect();
    assert_eq!(
        install_lines.len(),
        1,
        "expected exactly one install line, got {}: {lines:?}",
        install_lines.len()
    );
}

#[test]
fn test_rc_install_has_version_gate() {
    let rendered = RcInstallComponent.render().join("\n");
    assert!(
        rendered.contains("/usr/local/bin/rc") && rendered.contains("--version"),
        "rendered bash must check installed version before downloading: {rendered}"
    );
    assert!(
        rendered.contains("already installed"),
        "version gate should log the already-installed case: {rendered}"
    );
}

#[test]
fn test_rc_install_uses_https() {
    let rendered = RcInstallComponent.render().join("\n");
    assert!(
        rendered.contains("https://"),
        "download URL must be https: {rendered}"
    );
    assert!(
        !rendered.contains("http://"),
        "rendered bash must not contain plaintext http: {rendered}"
    );
}

#[test]
fn test_rc_install_no_sudo_and_no_curl_pipe_bash() {
    let rendered = RcInstallComponent.render().join("\n");
    assert!(
        !rendered.contains("sudo "),
        "harbor provisions as root — sudo must not appear: {rendered}"
    );
    assert!(
        !rendered.contains("curl -sSfL") || !rendered.contains("| bash"),
        "rc path must never trust an upstream install script: {rendered}"
    );
}

#[test]
fn test_rc_install_download_url_template_uses_only_two_placeholder_names() {
    let template = super::rc_install::RC_DOWNLOAD_URL_TEMPLATE;
    assert!(
        template.contains("{version}"),
        "template must use the {{version}} placeholder: {template}"
    );
    assert!(
        template.contains("{arch}"),
        "template must use the {{arch}} placeholder: {template}"
    );
    // No placeholder names beyond the two the spec allows.
    // Strip every occurrence of the allowed placeholders and assert
    // no unresolved `{...}` braces remain — a defence against a
    // future accidental `{version}/{arch}/{os}` three-way template.
    let stripped = template.replace("{version}", "").replace("{arch}", "");
    assert!(
        !stripped.contains('{'),
        "template must not contain any placeholder beyond {{version}} and {{arch}}: {stripped}"
    );
}

#[test]
fn test_rclone_install_uses_https_install_url() {
    let rendered = RcloneInstallComponent.render().join("\n");
    assert!(
        rendered.contains("https://rclone.org/install.sh"),
        "rendered bash must use the upstream https install URL: {rendered}"
    );
}

#[test]
fn test_rclone_install_has_idempotency_gate() {
    let rendered = RcloneInstallComponent.render().join("\n");
    assert!(
        rendered.contains("command -v rclone"),
        "rendered bash must check existing rclone before install: {rendered}"
    );
}

#[test]
fn test_rc_and_rclone_reexports_compile() {
    // Smoke-test: construct each component through the public
    // re-export path and call render() so clippy can't drop the
    // bindings as dead. Catches any future rename that would break
    // the ScriptBuilder wiring in spec 014.
    let rc_lines = crate::script::RcInstallComponent.render();
    let rclone_lines = crate::script::RcloneInstallComponent.render();
    assert!(!rc_lines.is_empty());
    assert!(!rclone_lines.is_empty());
}
