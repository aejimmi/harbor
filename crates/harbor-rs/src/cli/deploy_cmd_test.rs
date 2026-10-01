#![allow(clippy::unwrap_used, clippy::panic)]

use super::deploy_cmd::*;
use crate::config::SetupConfig;
use crate::config::setup::DeployConfig;

/// A single entry with services yields a health-check block that names
/// only those services.
#[test]
fn test_build_deploy_script_emits_health_check_for_listed_services() {
    let deploy = DeployConfig {
        repo: "github.com/you/web".to_owned(),
        binary: "target/release/web".to_owned(),
        install: "/usr/local/bin/web".to_owned(),
        steps: vec!["cargo build --release".to_owned()],
        services: vec!["web".to_owned()],
    };
    let script = build_deploy_script("web", &deploy);

    assert!(
        script.contains("Checking service health"),
        "expected a health-check block: {script}"
    );
    assert!(
        script.contains("systemctl is-active --quiet web"),
        "expected web to be checked: {script}"
    );
    assert!(
        !script.contains("systemctl is-active --quiet api"),
        "unrelated service 'api' should not appear: {script}"
    );
}

/// An entry with an empty `services:` list skips the health-check block
/// entirely — the deploy succeeds on step exit codes alone.
#[test]
fn test_build_deploy_script_skips_health_check_when_services_empty() {
    let deploy = DeployConfig {
        repo: "github.com/you/web".to_owned(),
        binary: "target/release/web".to_owned(),
        install: "/usr/local/bin/web".to_owned(),
        steps: vec!["make".to_owned()],
        services: Vec::new(),
    };
    let script = build_deploy_script("web", &deploy);

    assert!(
        !script.contains("Checking service health"),
        "empty services list should produce no health-check block: {script}"
    );
    assert!(
        !script.contains("systemctl is-active"),
        "empty services list should emit no systemctl check: {script}"
    );
}

/// Two different entries in the same config produce scripts that `cd`
/// into their own repo directory — entries are independent.
#[test]
fn test_build_deploy_script_scopes_cd_to_entry_repo() {
    let web = DeployConfig {
        repo: "github.com/you/web".to_owned(),
        binary: "target/release/web".to_owned(),
        install: "/usr/local/bin/web".to_owned(),
        steps: vec!["make".to_owned()],
        services: vec!["web".to_owned()],
    };
    let api = DeployConfig {
        repo: "github.com/you/api".to_owned(),
        binary: "target/release/api".to_owned(),
        install: "/usr/local/bin/api".to_owned(),
        steps: vec!["make".to_owned()],
        services: vec!["api".to_owned()],
    };

    let web_script = build_deploy_script("web", &web);
    let api_script = build_deploy_script("api", &api);

    assert!(
        web_script.contains("cd $HOME/web"),
        "web script should cd into web repo: {web_script}"
    );
    assert!(
        api_script.contains("cd $HOME/api"),
        "api script should cd into api repo: {api_script}"
    );
    assert!(
        !web_script.contains("cd $HOME/api"),
        "web script must not touch api repo"
    );
    assert!(
        !api_script.contains("cd $HOME/web"),
        "api script must not touch web repo"
    );
}

/// The deploy name is embedded in the `deploys.log` line so rollback can
/// filter history per-entry later.
#[test]
fn test_build_deploy_script_tags_log_with_deploy_name() {
    let deploy = DeployConfig {
        repo: "github.com/you/web".to_owned(),
        binary: "target/release/web".to_owned(),
        install: "/usr/local/bin/web".to_owned(),
        steps: vec!["make".to_owned()],
        services: Vec::new(),
    };
    let script = build_deploy_script("web", &deploy);

    assert!(
        script.contains("deploy web") && script.contains("deploys.log"),
        "log line should include 'deploy web' tag: {script}"
    );
}

/// Helper to build a SetupConfig with two named deploys for name-sorting
/// assertions.
fn two_deploy_config() -> SetupConfig {
    let yaml = r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: target/release/web
      install: /usr/local/bin/web
      steps: [make]
      services: [web]
    api:
      repo: github.com/you/api
      binary: target/release/api
      install: /usr/local/bin/api
      steps: [make]
      services: [api]
";
    serde_yaml::from_str(yaml).expect("parse")
}

#[test]
fn test_sorted_names_is_alphabetical() {
    let config = two_deploy_config();
    let names = sorted_names(&config);
    assert_eq!(names, vec!["api".to_owned(), "web".to_owned()]);
}

#[test]
fn test_resolve_entry_returns_entry_for_known_name() {
    let config = two_deploy_config();
    let names = sorted_names(&config);
    let entry = resolve_entry(&config, "web", &names).expect("web resolves");
    assert_eq!(entry.repo, "github.com/you/web");
}

#[test]
fn test_resolve_entry_errors_with_available_names_hint() {
    let config = two_deploy_config();
    let names = sorted_names(&config);
    let err = resolve_entry(&config, "missing", &names).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("'missing'") && msg.contains("not found"),
        "error should name the missing entry: {msg}"
    );
    assert!(
        msg.contains("api") && msg.contains("web"),
        "error should list available names: {msg}"
    );
}

#[test]
fn test_available_hint_is_friendly_when_empty() {
    let hint = available_hint(&[]);
    assert_eq!(hint, "no deploys configured");
}

#[test]
fn test_available_hint_lists_names() {
    let hint = available_hint(&["api".to_owned(), "web".to_owned()]);
    assert_eq!(hint, "available deploys: api, web");
}

#[test]
fn test_is_full_sha_accepts_40_hex() {
    assert!(super::rollback_cmd::is_full_sha(&"a1".repeat(20)));
}

#[test]
fn test_is_full_sha_rejects_short_upper_and_shell() {
    use super::rollback_cmd::is_full_sha;
    assert!(!is_full_sha("abc1234"));
    assert!(!is_full_sha(&"A".repeat(40)));
    assert!(!is_full_sha(&format!("{};id", "a".repeat(37))));
}
