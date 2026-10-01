#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::*;
use crate::config::SetupConfig;
use std::path::Path;

#[test]
fn test_files_component() {
    let c = FilesComponent {
        files: vec![ResolvedFile {
            target: "/etc/myapp/config.toml".to_owned(),
            content: "[server]\nport = 8080\n".to_owned(),
            owner: "myapp".to_owned(),
            group: "myapp".to_owned(),
            mode: "640".to_owned(),
        }],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cat > /etc/myapp/config.toml"))
    );
    assert!(lines.iter().any(|l| l.contains("port = 8080")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("chown myapp:myapp /etc/myapp/config.toml"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("chmod 640 /etc/myapp/config.toml"))
    );
}

#[test]
fn test_files_component_empty() {
    let c = FilesComponent { files: Vec::new() };
    assert!(c.render().is_empty());
}

// --- Integration tests ---

#[test]
fn test_from_setup_config() {
    let yaml = r#"
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
  system:
    timezone: "UTC"
  security:
    ufw:
      enabled: true
      allow_ports: [22, 80]
  updates:
    auto_upgrade: true
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
    let script = builder.build();

    assert!(script.contains("apt-get install -y git curl"));
    assert!(script.contains("Installing Go 1.24.5"));
    assert!(script.contains("/usr/local/go/bin:$PATH"));
    assert!(script.contains("Setting up Docker"));
    assert!(script.contains("timedatectl set-timezone UTC"));
    assert!(script.contains("ufw allow 22/tcp"));
    assert!(script.contains("apt-get") && script.contains("upgrade -y"));
    assert!(script.contains("Setup completed successfully"));
}

#[test]
fn test_builder_skips_disabled_components() {
    let yaml = r#"
setup:
  packages: []
  components:
    docker:
      enabled: false
    go:
      enabled: false
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
    let script = builder.build();

    assert!(!script.contains("Docker"));
    assert!(!script.contains("Installing Go"));
    assert!(script.contains("apt-get update"));
    assert!(script.contains("Setup completed successfully"));
}

#[test]
fn test_from_setup_config_new_components() {
    let yaml = r#"
setup:
  components:
    fish: { enabled: true }
    rust: { enabled: true }
    caddy: { enabled: true }
    chrony_nts: { enabled: true }
    fail2ban_rs: { enabled: true }
    swap: { size: "2G" }
  security:
    ssh_hardening: true
    kernel_hardening: true
    ufw:
      enabled: true
      rules:
        - { port: 22, proto: tcp, limit: true }
        - { port: 443, proto: tcp }
  services:
    - { name: myapp, enabled: true }
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
    let script = builder.build();

    assert!(script.contains("Fish shell"));
    assert!(script.contains("rustup.rs"));
    assert!(script.contains("caddy"));
    assert!(script.contains("chrony"));
    assert!(script.contains("fail2ban-rs"));
    assert!(script.contains("fallocate -l 2G"));
    assert!(script.contains("SSH hardening"));
    assert!(script.contains("kernel hardening"));
    assert!(script.contains("ufw limit 22/tcp"));
    assert!(script.contains("ufw allow 443/tcp"));
    assert!(script.contains("systemctl enable myapp"));
    // Enable-only mode — no unit file generated
    assert!(!script.contains("[Service]"));
}

// --- Mount hardening tests ---

// 25. Mount hardening renders fstab entries
#[test]
fn test_mount_hardening_component() {
    let c = MountHardeningComponent;
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("/tmp") && l.contains("noexec")),
        "must harden /tmp with noexec"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("/dev/shm") && l.contains("noexec")),
        "must harden /dev/shm with noexec"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("/var/tmp") && l.contains("bind")),
        "must bind /var/tmp to /tmp"
    );
}

// 26. Mount hardening wired into from_setup_config
#[test]
fn test_from_setup_config_mount_hardening() {
    let yaml = r#"
setup:
  security:
    mount_hardening: true
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
    let script = builder.build();
    assert!(
        script.contains("mount hardening"),
        "mount_hardening: true must emit mount hardening script"
    );
}

// 27. Container security flags deserialized from YAML
#[test]
fn test_service_spec_security_fields_yaml() {
    let yaml = r#"
setup:
  services:
    - name: web
      image: "nginx:latest"
      cap_drop: ["ALL"]
      cap_add: ["NET_BIND_SERVICE"]
      read_only: true
      pids_limit: 1024
      enabled: true
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let svc = &config.setup.services[0];
    assert_eq!(svc.cap_drop, vec!["ALL"]);
    assert_eq!(svc.cap_add, vec!["NET_BIND_SERVICE"]);
    assert!(svc.read_only);
    assert_eq!(svc.pids_limit, 1024);
}

// 28. pids_limit defaults to 256 when omitted from YAML
#[test]
fn test_service_spec_pids_limit_default() {
    let yaml = r#"
setup:
  services:
    - name: web
      image: "nginx:latest"
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    assert_eq!(config.setup.services[0].pids_limit, 256);
}
