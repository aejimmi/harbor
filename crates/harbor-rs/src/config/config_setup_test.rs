#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]

use super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_user_config_load_valid() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("config.yaml");
    fs::write(
        &path,
        r#"
cloudflare:
  api_token: "cf-token-123"
  zone_id: "zone-abc"
hetzner:
  token: "hz-token-456"
dns:
  base_domain: ".example.com"
  provider: "cloudflare"
github:
  tokens:
    myproject: "gh-token-789"
"#,
    )
    .expect("write");

    let config = UserConfig::load(Some(&path)).expect("load");
    assert_eq!(config.cloudflare.api_token, "cf-token-123");
    assert_eq!(config.cloudflare.zone_id, "zone-abc");
    assert_eq!(config.hetzner.token, "hz-token-456");
    assert_eq!(config.dns.base_domain, ".example.com");
    assert_eq!(config.dns.provider, "cloudflare");
    assert_eq!(config.github.token_for("myproject"), "gh-token-789");
    assert_eq!(config.github.token_for("unknown"), "");
}

#[test]
fn test_user_config_defaults() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("config.yaml");
    fs::write(&path, "hetzner:\n  token: abc\n").expect("write");

    let config = UserConfig::load(Some(&path)).expect("load");
    assert!(config.dns.base_domain.is_empty());
    assert_eq!(config.dns.provider, "cloudflare");
    assert!(config.cloudflare.api_token.is_empty());
    assert!(config.github.tokens.is_empty());
}

#[test]
fn test_user_config_missing_file() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("nonexistent.yaml");

    let err = UserConfig::load(Some(&path)).unwrap_err();
    assert!(matches!(err, ConfigError::NotFound { .. }));
}

#[test]
fn test_setup_config_load_full() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("setup.yaml");
    fs::write(
        &path,
        r#"
setup:
  packages:
    - git
    - curl
  components:
    docker:
      enabled: true
    go:
      enabled: true
      version: "1.24.5"
  path:
    mode: "prepend"
    paths:
      - "/usr/local/go/bin"
  system_user:
    name: "appuser"
    home: "/var/lib/appuser"
    shell: "/bin/bash"
    system: true
  directories:
    - path: "/var/lib/appuser/data"
      owner: "appuser"
      group: "appuser"
      mode: "755"
  services:
    - name: "myservice"
      enabled: true
      user: "appuser"
      working_directory: "/var/lib/appuser"
      exec_start: "/usr/local/bin/myservice"
      restart: "always"
      restart_sec: 10
  system:
    timezone: "UTC"
  updates:
    auto_upgrade: true
    upgrade_kernel: false
  security:
    ufw:
      enabled: true
      allow_ports: [22, 8080]
"#,
    )
    .expect("write");

    let config = SetupConfig::load(&path).expect("load");
    assert_eq!(config.setup.packages, vec!["git", "curl"]);
    assert!(config.setup.components.docker.enabled);
    assert!(config.setup.components.go.enabled);
    assert_eq!(config.setup.components.go.version, "1.24.5");
    assert_eq!(config.setup.path.mode, PathMode::Prepend);
    assert_eq!(config.setup.path.paths, vec!["/usr/local/go/bin"]);
    assert_eq!(config.setup.system_user.name, "appuser");
    assert_eq!(config.setup.directories.len(), 1);
    assert_eq!(config.setup.services.len(), 1);
    assert_eq!(config.setup.services[0].name, "myservice");
    assert!(config.setup.services[0].enabled);
    assert_eq!(config.setup.system.timezone, "UTC");
    assert!(config.setup.updates.auto_upgrade);
    assert!(!config.setup.updates.upgrade_kernel);
    assert!(config.setup.security.ufw.enabled);
    assert_eq!(config.setup.security.ufw.allow_ports, vec![22, 8080]);
}

#[test]
fn test_setup_config_defaults_for_missing_fields() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("minimal.yaml");
    fs::write(&path, "setup:\n  packages: []\n").expect("write");

    let config = SetupConfig::load(&path).expect("load");
    assert!(config.setup.packages.is_empty());
    assert!(!config.setup.components.docker.enabled);
    assert!(!config.setup.components.go.enabled);
    assert!(config.setup.system_user.name.is_empty());
    assert!(config.setup.directories.is_empty());
    assert!(config.setup.services.is_empty());
}

#[test]
fn test_path_mode_deserialization() {
    let prepend: PathMode = serde_yaml::from_str("\"prepend\"").expect("prepend");
    let append: PathMode = serde_yaml::from_str("\"append\"").expect("append");
    let overwrite: PathMode = serde_yaml::from_str("\"overwrite\"").expect("overwrite");

    assert_eq!(prepend, PathMode::Prepend);
    assert_eq!(append, PathMode::Append);
    assert_eq!(overwrite, PathMode::Overwrite);
}

#[test]
fn test_server_spec_type_rename() {
    let yaml = r#"
name: "test-server"
type: "cpx31"
location: "nbg1"
image: "ubuntu-24.04"
"#;
    let spec: ServerSpec = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(spec.server_type, "cpx31");
}

#[test]
fn test_setup_config_invalid_yaml_returns_parse_error() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("bad.yaml");
    fs::write(&path, "setup: [not: valid: yaml").expect("write");

    let err = SetupConfig::load(&path).unwrap_err();
    assert!(matches!(err, ConfigError::ParseFailed { .. }));
}

#[test]
fn test_user_config_invalid_yaml_returns_parse_error() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("bad.yaml");
    fs::write(&path, ": invalid\n  yaml: [broken").expect("write");

    let err = UserConfig::load(Some(&path)).unwrap_err();
    assert!(matches!(err, ConfigError::ParseFailed { .. }));
}

// --- Container service parse tests (spec 007) ---

#[test]
fn test_service_spec_parses_native_unchanged() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("native.yaml");
    fs::write(
        &path,
        r#"
setup:
  services:
    - name: "myapp"
      enabled: true
      start: true
      user: "appuser"
      working_directory: "/var/lib/appuser"
      exec_start: "/usr/local/bin/myapp"
      restart: "always"
      restart_sec: 10
"#,
    )
    .expect("write");

    let config = SetupConfig::load(&path).expect("load");
    let svc = &config.setup.services[0];
    assert_eq!(svc.name, "myapp");
    assert!(svc.enabled);
    assert!(svc.start);
    assert_eq!(svc.exec_start, "/usr/local/bin/myapp");
    assert_eq!(svc.restart, "always");
    assert_eq!(svc.restart_sec, 10);
    assert!(svc.image.is_none());
    assert_eq!(svc.runtime, ContainerRuntime::Docker);
    assert!(svc.ports.is_empty());
    assert!(svc.volumes.is_empty());
    assert!(svc.env.is_empty());
}

#[test]
fn test_service_spec_parses_container_docker_default() {
    let yaml = r#"
setup:
  services:
    - name: "web"
      enabled: true
      image: "nginx:latest"
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let svc = &config.setup.services[0];
    assert_eq!(svc.image.as_deref(), Some("nginx:latest"));
    assert_eq!(svc.runtime, ContainerRuntime::Docker);
}

#[test]
fn test_service_spec_parses_container_podman_explicit() {
    let yaml = r#"
setup:
  services:
    - name: "api"
      enabled: true
      image: "quay.io/myorg/api:v1"
      runtime: podman
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let svc = &config.setup.services[0];
    assert_eq!(svc.image.as_deref(), Some("quay.io/myorg/api:v1"));
    assert_eq!(svc.runtime, ContainerRuntime::Podman);
}

#[test]
fn test_service_spec_parses_container_all_fields() {
    let yaml = r#"
setup:
  services:
    - name: "web"
      enabled: true
      image: "nginx:latest"
      runtime: docker
      ports:
        - "80:80"
        - "443:443/tcp"
      volumes:
        - "/etc/nginx:/etc/nginx:ro"
        - "/var/log/nginx:/var/log/nginx"
      env:
        LOG_LEVEL: info
        APP_ENV: prod
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let svc = &config.setup.services[0];
    assert_eq!(svc.image.as_deref(), Some("nginx:latest"));
    assert_eq!(svc.runtime, ContainerRuntime::Docker);
    assert_eq!(svc.ports, vec!["80:80", "443:443/tcp"]);
    assert_eq!(
        svc.volumes,
        vec!["/etc/nginx:/etc/nginx:ro", "/var/log/nginx:/var/log/nginx"]
    );
    assert_eq!(svc.env.len(), 2);
    assert_eq!(svc.env.get("LOG_LEVEL").map(String::as_str), Some("info"));
    assert_eq!(svc.env.get("APP_ENV").map(String::as_str), Some("prod"));
}

#[test]
fn test_from_setup_config_rejects_image_and_exec_start() {
    let yaml = r#"
setup:
  services:
    - name: "bad"
      enabled: true
      image: "nginx:latest"
      exec_start: "/usr/local/bin/nginx"
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let result = crate::script::ScriptBuilder::from_setup_config(
        &config,
        "",
        std::path::Path::new("."),
        None,
    );
    let Err(err) = result else {
        panic!("expected Err for image + exec_start conflict, got Ok");
    };
    let msg = format!("{err}");
    assert!(msg.contains("bad"), "error should name the service: {msg}");
    assert!(
        msg.contains("image") && msg.contains("exec_start"),
        "error should mention both fields: {msg}"
    );
}

#[test]
fn test_from_setup_config_rejects_empty_string_image() {
    let yaml = r#"
setup:
  services:
    - name: "web"
      enabled: true
      image: ""
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let result = crate::script::ScriptBuilder::from_setup_config(
        &config,
        "",
        std::path::Path::new("."),
        None,
    );
    let Err(err) = result else {
        panic!("expected Err for empty-string image, got Ok");
    };
    let msg = format!("{err}");
    assert!(msg.contains("web"), "error should name the service: {msg}");
    assert!(
        msg.contains("image") && msg.contains("empty"),
        "error should mention empty image: {msg}"
    );
}

#[test]
fn test_from_setup_config_rejects_whitespace_image() {
    let yaml = r#"
setup:
  services:
    - name: "web"
      enabled: true
      image: "   "
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let result = crate::script::ScriptBuilder::from_setup_config(
        &config,
        "",
        std::path::Path::new("."),
        None,
    );
    assert!(
        result.is_err(),
        "whitespace-only image should be rejected like empty string"
    );
}

#[test]
fn test_default_config_template_parses_with_no_dns() {
    let config: UserConfig =
        serde_yaml::from_str(super::templates::DEFAULT_CONFIG).expect("template parses");
    assert!(config.hetzner.token.is_empty());
    assert!(!crate::dns::is_configured(&config));
}
