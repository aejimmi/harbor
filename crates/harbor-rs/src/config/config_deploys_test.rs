#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]

use super::*;
use std::fs;
use tempfile::TempDir;

// --- Named deploys schema (spec 009) ---

#[test]
fn test_setup_deploys_map_round_trips_by_name() {
    let yaml = r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: target/release/web
      install: /usr/local/bin/web
      steps:
        - cargo build --release
      services: [web]
    api:
      repo: github.com/you/api
      binary: target/release/api
      install: /usr/local/bin/api
      steps:
        - cargo build --release
      services: [api]
";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(config.setup.deploys.len(), 2);

    let web = config.setup.deploys.get("web").expect("web entry");
    assert_eq!(web.repo, "github.com/you/web");
    assert_eq!(web.binary, "target/release/web");
    assert_eq!(web.install, "/usr/local/bin/web");
    assert_eq!(web.steps.len(), 1);
    assert_eq!(web.services, vec!["web".to_owned()]);

    let api = config.setup.deploys.get("api").expect("api entry");
    assert_eq!(api.repo, "github.com/you/api");
    assert_eq!(api.binary, "target/release/api");
    assert_eq!(api.install, "/usr/local/bin/api");
    assert_eq!(api.services, vec!["api".to_owned()]);
}

#[test]
fn test_setup_deploy_entry_defaults_services_to_empty_vec() {
    let yaml = r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: target/release/web
      install: /usr/local/bin/web
      steps: [make]
";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let web = config.setup.deploys.get("web").expect("web entry");
    assert!(
        web.services.is_empty(),
        "missing services: should deserialize to empty vec"
    );
}

/// Every invariant validated by `SetupConfig::validate` is exercised
/// below. These all go through `SetupConfig::load` so the hook from
/// `load()` → `validate()` is covered too.
fn write_yaml(dir: &TempDir, yaml: &str) -> std::path::PathBuf {
    let path = dir.path().join("setup.yaml");
    fs::write(&path, yaml).expect("write");
    path
}

#[test]
fn test_setup_deploy_valid_binary_and_install_loads() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: target/release/web
      install: /usr/local/bin/web
      steps: [make]
",
    );
    let _ = SetupConfig::load(&path).expect("load");
}

#[test]
fn test_setup_deploy_missing_binary_is_serde_error() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      install: /usr/local/bin/web
      steps: [make]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::ParseFailed { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("binary"), "error should name field: {msg}");
}

#[test]
fn test_setup_deploy_missing_install_is_serde_error() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: target/release/web
      steps: [make]
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::ParseFailed { .. }));
    let msg = format!("{err}");
    assert!(msg.contains("install"), "error should name field: {msg}");
}

#[test]
fn test_setup_deploy_binary_traversal_is_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: ../../etc/passwd
      install: /usr/local/bin/web
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains(".."),
        "error should cite '..' traversal: {msg}"
    );
}

#[test]
fn test_setup_deploy_absolute_binary_is_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: /absolute/path
      install: /usr/local/bin/web
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("repo-relative"),
        "error should explain repo-relative rule: {msg}"
    );
}

#[test]
fn test_setup_deploy_relative_install_is_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r"
setup:
  deploys:
    web:
      repo: github.com/you/web
      binary: target/release/web
      install: usr/local/bin/web
",
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("absolute"),
        "error should explain absolute-path rule: {msg}"
    );
}

#[test]
fn test_setup_deploy_invalid_name_chars_are_rejected() {
    let dir = TempDir::new().expect("tempdir");
    let path = write_yaml(
        &dir,
        r#"
setup:
  deploys:
    "web$prod":
      repo: github.com/you/web
      binary: target/release/web
      install: /usr/local/bin/web
"#,
    );
    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::Invalid { .. }));
    let msg = format!("{err}");
    assert!(
        msg.contains("[A-Za-z0-9_-]"),
        "error should explain allowed charset: {msg}"
    );
}

#[test]
fn test_setup_without_deploys_key_parses_to_empty_map() {
    let yaml = r"
setup:
  packages: []
";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    assert!(
        config.setup.deploys.is_empty(),
        "missing deploys: should yield empty map"
    );
}

#[test]
fn test_setup_legacy_top_level_deploy_key_is_rejected() {
    // The old schema had a single `deploy:` block under `setup:`. With
    // `deny_unknown_fields` on SetupSection, any stale YAML using that
    // key must surface a parse error so users see the migration path.
    let yaml = r"
setup:
  deploy:
    repo: github.com/you/legacy
    steps: [make]
";
    let result: Result<SetupConfig, _> = serde_yaml::from_str(yaml);
    assert!(
        result.is_err(),
        "legacy top-level deploy: key should fail to parse"
    );
}

// ---

#[test]
fn test_service_spec_debug_redacts_env() {
    let mut env = std::collections::BTreeMap::new();
    env.insert("DB_PASSWORD".to_owned(), "hunter2".to_owned());
    env.insert("API_KEY".to_owned(), "s3cr3t".to_owned());
    let svc = ServiceSpec {
        name: "web".to_owned(),
        enabled: true,
        start: true,
        user: String::new(),
        working_directory: String::new(),
        exec_start: String::new(),
        restart: String::new(),
        restart_sec: 0,
        image: Some("nginx:latest".to_owned()),
        runtime: ContainerRuntime::Docker,
        ports: Vec::new(),
        volumes: Vec::new(),
        env,
        cap_drop: Vec::new(),
        cap_add: Vec::new(),
        read_only: false,
        pids_limit: 256,
    };
    let rendered = format!("{svc:?}");
    // The secret values must not surface.
    assert!(
        !rendered.contains("hunter2"),
        "Debug output leaked env value: {rendered}"
    );
    assert!(
        !rendered.contains("s3cr3t"),
        "Debug output leaked env value: {rendered}"
    );
    // The key count is surfaced along with a redaction marker so
    // reviewers know the field is deliberately hidden.
    assert!(
        rendered.contains("redacted"),
        "Debug output must show a redaction marker: {rendered}"
    );
    assert!(
        rendered.contains("2 keys"),
        "Debug output must show env key count: {rendered}"
    );
    // The non-secret fields are still readable.
    assert!(rendered.contains("web"), "name must still render");
    assert!(rendered.contains("nginx:latest"), "image must still render");
}
