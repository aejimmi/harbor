#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::*;

/// Build a `DeployComponent` suitable for unit-test assertions — all
/// the spec R2 plumbing filled in with sensible defaults so each test
/// body only overrides the field it cares about.
fn deploy(name: &str) -> DeployComponent {
    DeployComponent {
        name: name.to_owned(),
        repo: "github.com/user/myapp".to_owned(),
        steps: vec!["cargo build --release".to_owned()],
        binary: "target/release/myapp".to_owned(),
        install: "/usr/local/bin/myapp".to_owned(),
        services: Vec::new(),
        health_check: Vec::new(),
    }
}

#[test]
fn test_deploy_component_emits_clone_pull_block() {
    let lines = deploy("web").render();
    assert!(lines.iter().any(|l| l.contains("if [ -d")));
    assert!(lines.iter().any(|l| l.contains("git pull")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("git clone https://github.com/user/myapp"))
    );
    assert!(lines.iter().any(|l| l.contains("cd $HOME/myapp")));
}

#[test]
fn test_deploy_component_runs_user_steps() {
    let lines = deploy("web").render();
    assert!(lines.iter().any(|l| l == "cargo build --release"));
}

#[test]
fn test_deploy_component_emits_harbor_install_root_preamble() {
    let lines = deploy("web").render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("HARBOR_INSTALL_ROOT") && l.contains("/opt/harbor")),
        "expected the HARBOR_INSTALL_ROOT default preamble: {lines:#?}"
    );
}

#[test]
fn test_deploy_component_preserves_binary_under_versioned_dir() {
    let lines = deploy("web").render();
    assert!(
        lines.iter().any(|l| l == "SHA=$(git rev-parse HEAD)"),
        "expected SHA capture: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("VERSION_DIR=\"$HARBOR_INSTALL_ROOT/web/$SHA\"")),
        "expected versioned dir assignment keyed on name + SHA: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("mkdir -p \"$VERSION_DIR\"")),
        "expected version dir mkdir: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("install -m 755 \"target/release/myapp\" \"$VERSION_DIR/myapp\"")),
        "expected `install -m 755` of binary to basename in version dir: {lines:#?}"
    );
}

#[test]
fn test_deploy_component_atomic_symlink_swap() {
    let lines = deploy("web").render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ln -sfn \"$VERSION_DIR/myapp\" \"/usr/local/bin/myapp.new\"")),
        "expected staging symlink with .new suffix: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("mv -T \"/usr/local/bin/myapp.new\" \"/usr/local/bin/myapp\"")),
        "expected atomic mv -T swap: {lines:#?}"
    );
}

#[test]
fn test_deploy_component_restart_lines_in_declared_order() {
    let mut c = deploy("web");
    c.services = vec!["alpha".to_owned(), "beta".to_owned()];
    let lines = c.render();
    let alpha_idx = lines
        .iter()
        .position(|l| l == "systemctl restart alpha")
        .expect("alpha restart");
    let beta_idx = lines
        .iter()
        .position(|l| l == "systemctl restart beta")
        .expect("beta restart");
    assert!(
        alpha_idx < beta_idx,
        "declared service order must be preserved"
    );
}

#[test]
fn test_deploy_component_health_check_between_restart_and_log() {
    let mut c = deploy("web");
    c.services = vec!["web".to_owned()];
    c.health_check = vec!["echo 'Checking service health...'".to_owned()];
    let lines = c.render();
    let restart_idx = lines
        .iter()
        .position(|l| l == "systemctl restart web")
        .expect("restart");
    let health_idx = lines
        .iter()
        .position(|l| l == "echo 'Checking service health...'")
        .expect("health check");
    let log_idx = lines
        .iter()
        .position(|l| l.contains("deploys.log"))
        .expect("log write");
    assert!(
        restart_idx < health_idx && health_idx < log_idx,
        "restart -> health check -> log order must hold: {lines:#?}"
    );
}

#[test]
fn test_deploy_component_records_sha_in_log() {
    let lines = deploy("api").render();
    assert!(lines.iter().any(|l| l == "mkdir -p ~/.harbor"));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("deploys.log") && l.contains("$SHA") && l.contains("deploy api")),
        "log line should reference captured $SHA and be tagged with the deploy name: {lines:#?}"
    );
}

#[test]
fn test_deploy_component_emits_retention_gc() {
    let lines = deploy("web").render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ls -1dt") && l.contains("$HARBOR_INSTALL_ROOT/web")),
        "expected GC listing under the deploy's versioned root: {lines:#?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("tail -n +6")),
        "expected keep-last-5 tail offset: {lines:#?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("readlink -f")),
        "expected readlink safety-belt for active symlink target: {lines:#?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("rm -rf --")),
        "expected GC removal line: {lines:#?}"
    );
}

#[test]
fn test_deploy_repo_name() {
    assert_eq!(DeployComponent::repo_name("github.com/user/myapp"), "myapp");
    assert_eq!(
        DeployComponent::repo_name("github.com/user/myapp.git"),
        "myapp"
    );
    assert_eq!(
        DeployComponent::repo_name("https://github.com/user/myapp"),
        "myapp"
    );
    assert_eq!(DeployComponent::repo_name("myapp"), "myapp");
}

#[test]
fn test_deploy_clone_url() {
    assert_eq!(
        DeployComponent::clone_url("github.com/user/myapp"),
        "https://github.com/user/myapp"
    );
    assert_eq!(
        DeployComponent::clone_url("https://github.com/user/myapp"),
        "https://github.com/user/myapp"
    );
    assert_eq!(
        DeployComponent::clone_url("http://gitlab.com/user/myapp"),
        "http://gitlab.com/user/myapp"
    );
    assert_eq!(
        DeployComponent::clone_url("file:///tmp/bare.git"),
        "file:///tmp/bare.git"
    );
    assert_eq!(
        DeployComponent::clone_url("git@github.com:user/repo.git"),
        "git@github.com:user/repo.git"
    );
    assert_eq!(
        DeployComponent::clone_url("ssh://git@host/repo.git"),
        "ssh://git@host/repo.git"
    );
    assert_eq!(
        DeployComponent::clone_url("git://example.com/repo.git"),
        "git://example.com/repo.git"
    );
}

/// Build a `RollbackComponent` with sensible defaults for the spec
/// R3 flow. Tests override `version`, `services`, and `health_check`.
fn rollback(name: &str, version: Option<&str>) -> RollbackComponent {
    RollbackComponent {
        name: name.to_owned(),
        version: version.map(str::to_owned),
        binary: "target/release/myapp".to_owned(),
        install: "/usr/local/bin/myapp".to_owned(),
        services: Vec::new(),
        health_check: Vec::new(),
    }
}

#[test]
fn test_rollback_component_with_explicit_sha_sets_target() {
    let lines = rollback("web", Some("abc123f")).render();
    assert!(
        lines.iter().any(|l| l == "TARGET_SHA=\"abc123f\""),
        "explicit version should set TARGET_SHA directly: {lines:#?}"
    );
}

#[test]
fn test_rollback_component_without_sha_reads_previous_from_log() {
    let lines = rollback("web", None).render();
    assert!(
        lines.iter().any(|l| l.contains("~/.harbor/deploys.log")),
        "expected deploys.log read when version is None: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("grep -E") && l.contains("web")),
        "expected grep filtered by deploy name: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("TARGET_SHA=") && l.contains("awk")),
        "expected TARGET_SHA extracted via awk: {lines:#?}"
    );
}

#[test]
fn test_rollback_component_emits_harbor_install_root_preamble() {
    let lines = rollback("web", Some("abc123f")).render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("HARBOR_INSTALL_ROOT") && l.contains("/opt/harbor")),
        "expected the HARBOR_INSTALL_ROOT preamble: {lines:#?}"
    );
}

#[test]
fn test_rollback_component_checks_version_dir_exists_before_swap() {
    let lines = rollback("web", Some("abc123f")).render();
    let check_idx = lines
        .iter()
        .position(|l| l == "if [ ! -f \"$TARGET\" ]; then")
        .expect("existence check");
    let swap_idx = lines
        .iter()
        .position(|l| l.contains("ln -sfn \"$TARGET\""))
        .expect("symlink swap");
    assert!(
        check_idx < swap_idx,
        "existence check must precede symlink swap: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("not preserved") && l.contains("re-deploy")),
        "expected friendly 'not preserved' error with re-deploy hint: {lines:#?}"
    );
}

#[test]
fn test_rollback_component_atomic_symlink_swap() {
    let lines = rollback("web", Some("abc123f")).render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ln -sfn \"$TARGET\" \"/usr/local/bin/myapp.new\"")),
        "expected stage symlink: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("mv -T \"/usr/local/bin/myapp.new\" \"/usr/local/bin/myapp\"")),
        "expected mv -T swap: {lines:#?}"
    );
}

#[test]
fn test_rollback_component_restarts_services_in_declared_order() {
    let mut c = rollback("web", Some("abc123f"));
    c.services = vec!["alpha".to_owned(), "beta".to_owned()];
    let lines = c.render();
    let alpha_idx = lines
        .iter()
        .position(|l| l == "systemctl restart alpha")
        .expect("alpha restart");
    let beta_idx = lines
        .iter()
        .position(|l| l == "systemctl restart beta")
        .expect("beta restart");
    assert!(
        alpha_idx < beta_idx,
        "declared service order must be preserved on rollback"
    );
}

#[test]
fn test_rollback_component_records_target_sha_in_log() {
    let lines = rollback("web", Some("def456")).render();
    assert!(lines.iter().any(|l| l == "mkdir -p ~/.harbor"));
    assert!(
        lines.iter().any(|l| l.contains("deploys.log")
            && l.contains("$TARGET_SHA")
            && l.contains("rollback web")),
        "log line should reference $TARGET_SHA and be tagged with deploy name: {lines:#?}"
    );
}

#[test]
fn test_rollback_component_emits_no_gc_block() {
    let lines = rollback("web", Some("abc123f")).render();
    assert!(
        !lines.iter().any(|l| l.contains("tail -n +6")),
        "rollback must not run retention GC (spec R5): {lines:#?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("git fetch") || l.contains("git checkout")),
        "rollback must not run any git operations (spec R3): {lines:#?}"
    );
}
