#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use std::path::Path;

use super::*;
use crate::config::{ContainerRuntime, DirectorySpec, PathMode, ServiceSpec, SetupConfig, UfwRule};

#[test]
fn test_empty_builder_produces_valid_script() {
    let script = ScriptBuilder::new().build();
    assert!(script.starts_with("#!/bin/bash\nset -e"));
    assert!(script.contains("apt-get update"));
    assert!(script.contains("Setup completed successfully"));
    assert!(script.trim_end().ends_with('\''));
}

#[test]
fn test_packages_component() {
    let c = PackagesComponent {
        packages: vec!["git".to_owned(), "curl".to_owned(), "jq".to_owned()],
    };
    let lines = c.render();
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("apt-get install -y git curl jq"));
}

#[test]
fn test_packages_component_empty() {
    let c = PackagesComponent {
        packages: Vec::new(),
    };
    assert!(c.render().is_empty());
}

#[test]
fn test_go_component() {
    let c = GoComponent {
        version: "1.24.5".to_owned(),
    };
    let lines = c.render();
    assert!(lines[0].contains("Installing Go 1.24.5"));
    assert!(lines.iter().any(|l| l.contains("go1.24.5.linux")));
    assert!(lines.iter().any(|l| l.contains("rm go.tar.gz")));
}

#[test]
fn test_docker_component() {
    let lines = DockerComponent.render();
    assert!(lines.iter().any(|l| l.contains("docker-ce")));
    assert!(lines.iter().any(|l| l.contains("systemctl enable docker")));
}

#[test]
fn test_path_component_prepend() {
    let c = PathComponent {
        mode: PathMode::Prepend,
        paths: vec!["/usr/local/go/bin".to_owned()],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("/usr/local/go/bin:$PATH")));
}

#[test]
fn test_path_component_append() {
    let c = PathComponent {
        mode: PathMode::Append,
        paths: vec!["/opt/bin".to_owned()],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("$PATH:/opt/bin")));
}

#[test]
fn test_path_component_overwrite() {
    let c = PathComponent {
        mode: PathMode::Overwrite,
        paths: vec!["/custom/bin".to_owned()],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("PATH=\"/custom/bin\"") && !l.contains("$PATH"))
    );
}

#[test]
fn test_path_component_empty() {
    let c = PathComponent {
        mode: PathMode::Prepend,
        paths: Vec::new(),
    };
    assert!(c.render().is_empty());
}

#[test]
fn test_env_component_masks_sensitive_keys() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("GITHUB_TOKEN".to_owned(), "secret123".to_owned());
    vars.insert("APP_NAME".to_owned(), "myapp".to_owned());
    let c = EnvComponent { vars };
    let lines = c.render();

    assert!(lines.iter().any(|l| l.contains("Setting up GITHUB_TOKEN")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("export GITHUB_TOKEN=\"secret123\""))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("export APP_NAME=\"myapp\""))
    );
}

#[test]
fn test_system_user_component() {
    let c = SystemUserComponent {
        name: "appuser".to_owned(),
        home: "/var/lib/appuser".to_owned(),
        shell: "/bin/bash".to_owned(),
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("useradd") && l.contains("appuser"))
    );
}

#[test]
fn test_directories_component() {
    let c = DirectoriesComponent {
        dirs: vec![DirectorySpec {
            path: "/var/data".to_owned(),
            owner: "app".to_owned(),
            group: "app".to_owned(),
            mode: "755".to_owned(),
        }],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("mkdir -p /var/data")));
    assert!(lines.iter().any(|l| l.contains("chown app:app /var/data")));
    assert!(lines.iter().any(|l| l.contains("chmod 755 /var/data")));
}

#[test]
fn test_services_component_with_exec_start() {
    let c = ServicesComponent {
        services: vec![ServiceSpec {
            name: "myapp".to_owned(),
            enabled: true,
            start: true,
            user: "appuser".to_owned(),
            working_directory: "/var/lib/app".to_owned(),
            exec_start: "/usr/local/bin/myapp".to_owned(),
            restart: "always".to_owned(),
            restart_sec: 10,
            image: None,
            runtime: crate::config::ContainerRuntime::Docker,
            ports: Vec::new(),
            volumes: Vec::new(),
            env: std::collections::BTreeMap::new(),
        }],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("[Service]")));
    assert!(lines.iter().any(|l| l.contains("User=appuser")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("ExecStart=/usr/local/bin/myapp"))
    );
    assert!(!lines.iter().any(|l| l.contains("usercanal")));
    assert!(lines.iter().any(|l| l.contains("systemctl enable myapp")));
    assert!(lines.iter().any(|l| l.contains("systemctl restart myapp")));
}

#[test]
fn test_services_enable_only_no_start() {
    let c = ServicesComponent {
        services: vec![ServiceSpec {
            name: "blissd".to_owned(),
            enabled: true,
            start: false,
            user: String::new(),
            working_directory: String::new(),
            exec_start: String::new(),
            restart: String::new(),
            restart_sec: 0,
            image: None,
            runtime: crate::config::ContainerRuntime::Docker,
            ports: Vec::new(),
            volumes: Vec::new(),
            env: std::collections::BTreeMap::new(),
        }],
    };
    let lines = c.render();
    assert!(!lines.iter().any(|l| l.contains("[Service]")));
    assert!(lines.iter().any(|l| l.contains("systemctl daemon-reload")));
    assert!(lines.iter().any(|l| l.contains("systemctl enable blissd")));
    // Must NOT start — binary not installed yet
    assert!(!lines.iter().any(|l| l.contains("systemctl start blissd")));
}

#[test]
fn test_services_enable_and_start() {
    let c = ServicesComponent {
        services: vec![ServiceSpec {
            name: "caddy".to_owned(),
            enabled: true,
            start: true,
            user: String::new(),
            working_directory: String::new(),
            exec_start: String::new(),
            restart: String::new(),
            restart_sec: 0,
            image: None,
            runtime: crate::config::ContainerRuntime::Docker,
            ports: Vec::new(),
            volumes: Vec::new(),
            env: std::collections::BTreeMap::new(),
        }],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("systemctl enable caddy")));
    assert!(lines.iter().any(|l| l.contains("systemctl restart caddy")));
}

#[test]
fn test_ufw_component_with_rules() {
    let c = UfwComponent::from_config(
        &[],
        &[
            UfwRule {
                port: 22,
                proto: "tcp".to_owned(),
                limit: true,
            },
            UfwRule {
                port: 443,
                proto: "tcp".to_owned(),
                limit: false,
            },
        ],
    );
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("ufw --force reset")));
    assert!(lines.iter().any(|l| l.contains("ufw allow 22/tcp")));
    assert!(lines.iter().any(|l| l.contains("ufw limit 22/tcp")));
    assert!(lines.iter().any(|l| l.contains("ufw allow 443/tcp")));
    assert!(!lines.iter().any(|l| l.contains("ufw limit 443")));
    assert!(lines.iter().any(|l| l.contains("ufw --force enable")));
}

#[test]
fn test_ufw_component_backward_compat() {
    let c = UfwComponent::from_config(&[22, 8080], &[]);
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("ufw allow 22/tcp")));
    assert!(lines.iter().any(|l| l.contains("ufw allow 8080/tcp")));
}

#[test]
fn test_updates_component_full() {
    let c = UpdatesComponent {
        auto_upgrade: true,
        upgrade_kernel: true,
        reboot_after_kernel: true,
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l.contains("apt-get")
            && l.contains("upgrade -y")
            && !l.contains("dist-upgrade"))
    );
    assert!(lines.iter().any(|l| l.contains("dist-upgrade -y")));
    assert!(lines.iter().any(|l| l.contains("shutdown -r +1")));
}

#[test]
fn test_updates_component_no_kernel() {
    let c = UpdatesComponent {
        auto_upgrade: true,
        upgrade_kernel: false,
        reboot_after_kernel: false,
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l.contains("apt-get")
            && l.contains("upgrade -y")
            && !l.contains("dist-upgrade"))
    );
    assert!(!lines.iter().any(|l| l.contains("dist-upgrade")));
    assert!(!lines.iter().any(|l| l.contains("shutdown")));
}

#[test]
fn test_hostname_component() {
    let c = HostnameComponent {
        hostname: "myhost".to_owned(),
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("hostnamectl set-hostname myhost"))
    );
    assert!(lines.iter().any(|l| l.contains("127.0.1.1 myhost")));
}

#[test]
fn test_timezone_component() {
    let c = hostname::TimezoneComponent {
        timezone: "UTC".to_owned(),
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("timedatectl set-timezone UTC"))
    );
}

// --- New component tests ---

#[test]
fn test_rust_component() {
    let lines = RustComponent.render();
    assert!(lines.iter().any(|l| l.contains("rustup.rs")));
    assert!(lines.iter().any(|l| l.contains("cargo/env")));
}

#[test]
fn test_caddy_component() {
    let lines = CaddyComponent.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cloudsmith.io/public/caddy"))
    );
    assert!(lines.iter().any(|l| l.contains("apt-get install -y caddy")));
}

#[test]
fn test_fish_component() {
    let lines = FishComponent.render();
    assert!(lines.iter().any(|l| l.contains("ppa:fish-shell")));
    assert!(lines.iter().any(|l| l.contains("apt-get install -y fish")));
    assert!(lines.iter().any(|l| l.contains("chsh")));
}

#[test]
fn test_swap_component() {
    let c = SwapComponent {
        size: "2G".to_owned(),
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("fallocate -l 2G")));
    assert!(lines.iter().any(|l| l.contains("mkswap")));
    assert!(lines.iter().any(|l| l.contains("swapon")));
    assert!(lines.iter().any(|l| l.contains("/etc/fstab")));
}

#[test]
fn test_chrony_nts_component() {
    let lines = ChronyNtsComponent.render();
    assert!(lines.iter().any(|l| l.contains("chrony")));
    assert!(lines.iter().any(|l| l.contains("time.cloudflare.com")));
    assert!(lines.iter().any(|l| l.contains("nts")));
}

#[test]
fn test_fail2ban_rs_component() {
    let lines = Fail2banRsComponent.render();
    assert!(lines.iter().any(|l| l.contains("fail2ban-rs")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("systemctl enable fail2ban-rs"))
    );
}

#[test]
fn test_ssh_hardening_component() {
    let lines = SshHardeningComponent.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("PermitRootLogin prohibit-password"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("PasswordAuthentication no"))
    );
    assert!(lines.iter().any(|l| l.contains("MaxAuthTries")));
    assert!(lines.iter().any(|l| l.contains("systemctl restart ssh")));
}

#[test]
fn test_kernel_hardening_component() {
    let lines = KernelHardeningComponent.render();
    assert!(lines.iter().any(|l| l.contains("tcp_syncookies")));
    assert!(lines.iter().any(|l| l.contains("rp_filter")));
    assert!(lines.iter().any(|l| l.contains("sysctl --system")));
    assert!(lines.iter().any(|l| l.contains("disable-unused.conf")));
    assert!(lines.iter().any(|l| l.contains("hard core 0")));
}

#[test]
fn test_deploy_component() {
    let c = DeployComponent {
        repo: "github.com/user/myapp".to_owned(),
        steps: vec![
            "cargo build --release".to_owned(),
            "cp target/release/myapp /usr/local/bin/".to_owned(),
        ],
    };
    let lines = c.render();
    // Clone-or-pull logic
    assert!(lines.iter().any(|l| l.contains("if [ -d")));
    assert!(lines.iter().any(|l| l.contains("git pull")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("git clone https://github.com/user/myapp"))
    );
    assert!(lines.iter().any(|l| l.contains("cd $HOME/myapp")));
    // Build steps
    assert!(lines.iter().any(|l| l.contains("cargo build --release")));
    assert!(lines.iter().any(|l| l.contains("cp target/release/myapp")));
}

#[test]
fn test_deploy_component_records_version() {
    let c = DeployComponent {
        repo: "github.com/user/myapp".to_owned(),
        steps: vec!["make build".to_owned()],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("mkdir -p ~/.harbor")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("deploys.log") && l.contains("git rev-parse HEAD"))
    );
}

#[test]
fn test_deploy_component_empty_steps() {
    let c = DeployComponent {
        repo: "github.com/user/myapp".to_owned(),
        steps: Vec::new(),
    };
    assert!(c.render().is_empty());
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
}

#[test]
fn test_rollback_component() {
    let c = RollbackComponent {
        repo: "github.com/user/myapp".to_owned(),
        version: "abc123f".to_owned(),
        steps: vec!["cargo build --release".to_owned()],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("Rolling back to abc123f")));
    assert!(lines.iter().any(|l| l.contains("cd $HOME/myapp")));
    assert!(lines.iter().any(|l| l.contains("git fetch --all")));
    assert!(lines.iter().any(|l| l.contains("git checkout abc123f")));
    assert!(lines.iter().any(|l| l.contains("cargo build --release")));
    // Records rollback in deploys.log
    assert!(
        lines
            .iter()
            .any(|l| l.contains("deploys.log") && l.contains("rollback"))
    );
}

#[test]
fn test_rollback_component_records_version() {
    let c = RollbackComponent {
        repo: "github.com/user/myapp".to_owned(),
        version: "def456".to_owned(),
        steps: vec![],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("mkdir -p ~/.harbor")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("git rev-parse HEAD") && l.contains("rollback"))
    );
}

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
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
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
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
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
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
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

#[test]
fn test_github_repos_with_token() {
    let c = GithubReposComponent {
        repos: vec![crate::config::GithubRepo {
            repo: "github.com/user/project".to_owned(),
            binary: "mybin".to_owned(),
            install_path: "/usr/local/bin".to_owned(),
            config_source: String::new(),
            config_target: String::new(),
        }],
        github_token: "ghp_test123".to_owned(),
        system_user: None,
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("GITHUB_TOKEN=\"ghp_test123\""))
    );
    assert!(lines.iter().any(|l| l.contains("git clone")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("go build -o $GOPATH/bin/mybin"))
    );
}

#[test]
fn test_github_repos_without_token() {
    let c = GithubReposComponent {
        repos: vec![crate::config::GithubRepo {
            repo: "github.com/user/project".to_owned(),
            binary: "mybin".to_owned(),
            install_path: String::new(),
            config_source: String::new(),
            config_target: String::new(),
        }],
        github_token: String::new(),
        system_user: None,
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("GITHUB_TOKEN not available"))
    );
}

#[test]
fn test_github_repos_cmd_subpath() {
    let c = GithubReposComponent {
        repos: vec![crate::config::GithubRepo {
            repo: "github.com/org/project/cmd/server".to_owned(),
            binary: "server".to_owned(),
            install_path: "/usr/local/bin".to_owned(),
            config_source: String::new(),
            config_target: String::new(),
        }],
        github_token: "token".to_owned(),
        system_user: None,
    };
    let lines = c.render();

    assert!(
        lines
            .iter()
            .any(|l| l.contains("git clone https://github.com/org/project.git"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("go build -o $GOPATH/bin/server ./cmd/server"))
    );
}

#[test]
fn test_github_repos_usercanal_dependent_repos() {
    let c = GithubReposComponent {
        repos: vec![crate::config::GithubRepo {
            repo: "github.com/usercanal/usercanal".to_owned(),
            binary: "usercanal".to_owned(),
            install_path: "/usr/local/bin".to_owned(),
            config_source: "configs/collector-server.yaml".to_owned(),
            config_target: "/etc/usercanal/usercanal.yaml".to_owned(),
        }],
        github_token: "token".to_owned(),
        system_user: Some("appuser".to_owned()),
    };
    let lines = c.render();

    assert!(
        lines
            .iter()
            .any(|l| l.contains("git clone https://github.com/usercanal/cdp-collector.git"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("git clone https://github.com/usercanal/cdp-api.git"))
    );
    assert!(lines.iter().any(|l| l.contains("mkdir -p /etc/usercanal")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("chown -R appuser:appuser /etc/usercanal"))
    );
}

#[test]
fn test_from_setup_config_with_github_repos() {
    let yaml = r#"
setup:
  components:
    go:
      enabled: true
      version: "1.24.5"
  github_repos:
    - repo: "github.com/user/project"
      binary: "mybin"
      install_path: "/usr/local/bin"
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder =
        ScriptBuilder::from_setup_config(&config, "gh_token_123", Path::new(".")).expect("build");
    let script = builder.build();

    assert!(script.contains("GITHUB_TOKEN=\"gh_token_123\""));
    assert!(script.contains("go build -o $GOPATH/bin/mybin"));
}

// --- Container service render tests (spec 007) ---

/// Build a container `ServiceSpec` with only the fields a test cares
/// about. All other fields take their empty / default values.
fn container_svc(name: &str, image: &str, runtime: ContainerRuntime) -> ServiceSpec {
    ServiceSpec {
        name: name.to_owned(),
        enabled: true,
        start: true,
        user: String::new(),
        working_directory: String::new(),
        exec_start: String::new(),
        restart: String::new(),
        restart_sec: 0,
        image: Some(image.to_owned()),
        runtime,
        ports: Vec::new(),
        volumes: Vec::new(),
        env: std::collections::BTreeMap::new(),
    }
}

// --- Docker rendering ---

#[test]
fn test_services_docker_unit_minimal() {
    let c = ServicesComponent {
        services: vec![container_svc(
            "web",
            "nginx:latest",
            ContainerRuntime::Docker,
        )],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cat > /etc/systemd/system/web.service"))
    );
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line present");
    assert!(
        exec_start.starts_with("ExecStart=/usr/bin/docker run"),
        "got: {exec_start}"
    );
}

#[test]
fn test_services_docker_unit_has_log_driver_journald() {
    let c = ServicesComponent {
        services: vec![container_svc(
            "api",
            "ghcr.io/foo/api:v1",
            ContainerRuntime::Docker,
        )],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart");
    assert!(
        exec_start.contains("--log-driver=journald"),
        "got: {exec_start}"
    );
}

#[test]
fn test_services_docker_unit_ports_volumes_env() {
    let mut svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    svc.ports = vec!["80:80".to_owned(), "443:443/tcp".to_owned()];
    svc.volumes = vec!["/data:/data:ro".to_owned()];
    svc.env.insert("LOG_LEVEL".to_owned(), "info".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart");
    assert!(exec_start.contains("-p 80:80"), "got: {exec_start}");
    assert!(exec_start.contains("-p 443:443/tcp"), "got: {exec_start}");
    assert!(
        exec_start.contains("-v /data:/data:ro"),
        "got: {exec_start}"
    );
    // Env is referenced via --env-file, not inline `-e` flags.
    assert!(
        exec_start.contains("--env-file /etc/harbor/env/web.env"),
        "got: {exec_start}"
    );
    assert!(
        !exec_start.contains("-e LOG_LEVEL=info"),
        "env must not be inlined: {exec_start}"
    );
    // --env-file precedes the image argument.
    let image_pos = exec_start.find("nginx:1").expect("image present");
    let env_pos = exec_start
        .find("--env-file /etc/harbor/env/web.env")
        .expect("env-file present");
    assert!(
        env_pos < image_pos,
        "env-file must precede image: {exec_start}"
    );
}

#[test]
fn test_services_docker_env_sorted() {
    let mut svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    // Insert out of alphabetical order — BTreeMap will sort.
    svc.env.insert("ZETA".to_owned(), "z".to_owned());
    svc.env.insert("ALPHA".to_owned(), "a".to_owned());
    svc.env.insert("MIKE".to_owned(), "m".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    // Env file contents (between the heredoc markers) must be sorted.
    let alpha = lines
        .iter()
        .position(|l| l == "ALPHA=a")
        .expect("ALPHA present");
    let mike = lines
        .iter()
        .position(|l| l == "MIKE=m")
        .expect("MIKE present");
    let zeta = lines
        .iter()
        .position(|l| l == "ZETA=z")
        .expect("ZETA present");
    assert!(alpha < mike && mike < zeta, "env file not sorted");
}

#[test]
fn test_services_docker_unit_orphan_cleanup() {
    let c = ServicesComponent {
        services: vec![container_svc("web", "nginx:1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l == "ExecStartPre=-/usr/bin/docker rm -f web"),
        "orphan cleanup line with leading dash missing"
    );
}

#[test]
fn test_services_docker_unit_pull_is_best_effort() {
    let c = ServicesComponent {
        services: vec![container_svc("web", "nginx:1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l == "ExecStartPre=-/usr/bin/docker pull nginx:1"),
        "best-effort pull line with leading dash missing"
    );
}

#[test]
fn test_services_docker_unit_uses_rm_flag() {
    let c = ServicesComponent {
        services: vec![container_svc("web", "nginx:1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart");
    assert!(exec_start.contains("--rm"), "got: {exec_start}");
}

#[test]
fn test_services_docker_unit_no_docker_restart_flag() {
    let c = ServicesComponent {
        services: vec![container_svc("web", "nginx:1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart");
    assert!(
        !exec_start.contains("--restart"),
        "docker run must not carry --restart (systemd owns restart): {exec_start}"
    );
}

#[test]
fn test_services_docker_unit_execstop_present() {
    let c = ServicesComponent {
        services: vec![container_svc("web", "nginx:1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l == "ExecStop=/usr/bin/docker stop -t 10 web"),
        "ExecStop line missing"
    );
}

#[test]
fn test_services_docker_unit_dependencies() {
    let c = ServicesComponent {
        services: vec![container_svc("web", "nginx:1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l == "After=network-online.target docker.service")
    );
    assert!(lines.iter().any(|l| l == "Requires=docker.service"));
}

#[test]
fn test_services_docker_env_written_to_env_file_with_mode_600() {
    let mut svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    svc.env
        .insert("DB_PASSWORD".to_owned(), "hunter2".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "mkdir -p /etc/harbor/env"),
        "env parent dir must be created"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "cat > /etc/harbor/env/web.env << 'EOF'"),
        "env file heredoc missing"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "chmod 600 /etc/harbor/env/web.env"),
        "chmod 600 missing"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "chown root:root /etc/harbor/env/web.env"),
        "chown root:root missing"
    );
    // The env file contains the KEY=VALUE line.
    assert!(
        lines.iter().any(|l| l == "DB_PASSWORD=hunter2"),
        "env file must contain the KEY=VALUE line"
    );
}

#[test]
fn test_services_docker_unit_uses_env_file_flag() {
    let mut svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    svc.env
        .insert("DB_PASSWORD".to_owned(), "hunter2".to_owned());
    svc.env.insert("API_KEY".to_owned(), "s3cret".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart");
    assert!(
        exec_start.contains("--env-file /etc/harbor/env/web.env"),
        "docker run must reference --env-file: {exec_start}"
    );
    assert!(
        !exec_start.contains("-e DB_PASSWORD=hunter2"),
        "docker run must not inline env via -e: {exec_start}"
    );
    assert!(
        !exec_start.contains("-e API_KEY=s3cret"),
        "docker run must not inline env via -e: {exec_start}"
    );
}

#[test]
fn test_services_docker_unit_no_env_file_when_empty() {
    let svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    // svc.env is empty by default from container_svc().
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    // No env file is written.
    assert!(
        !lines.iter().any(|l| l.contains("/etc/harbor/env/web.env")),
        "no env file lines must be emitted when env is empty"
    );
    // No --env-file flag in the docker run line.
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart");
    assert!(
        !exec_start.contains("--env-file"),
        "--env-file must not be added when env is empty: {exec_start}"
    );
}

// --- Podman Quadlet rendering ---

#[test]
fn test_services_podman_unit_minimal() {
    let c = ServicesComponent {
        services: vec![container_svc(
            "api",
            "quay.io/org/api:v1",
            ContainerRuntime::Podman,
        )],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l == "[Container]"));
    assert!(lines.iter().any(|l| l == "Image=quay.io/org/api:v1"));
    assert!(lines.iter().any(|l| l == "ContainerName=api"));
}

#[test]
fn test_services_podman_unit_ports_volumes_env() {
    let mut svc = container_svc("api", "quay.io/org/api:v1", ContainerRuntime::Podman);
    svc.ports = vec!["8080:8080".to_owned()];
    svc.volumes = vec!["/etc/api:/etc/api:ro".to_owned()];
    svc.env.insert("RUST_LOG".to_owned(), "info".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l == "PublishPort=8080:8080"));
    assert!(lines.iter().any(|l| l == "Volume=/etc/api:/etc/api:ro"));
    // Env is referenced via EnvironmentFile=, not inline Environment=.
    assert!(
        lines
            .iter()
            .any(|l| l == "EnvironmentFile=/etc/harbor/env/api.env"),
        "EnvironmentFile line missing"
    );
    assert!(
        !lines.iter().any(|l| l == "Environment=RUST_LOG=info"),
        "env must not be inlined as Environment="
    );
}

#[test]
fn test_services_podman_env_sorted() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.env.insert("ZETA".to_owned(), "z".to_owned());
    svc.env.insert("ALPHA".to_owned(), "a".to_owned());
    svc.env.insert("MIKE".to_owned(), "m".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    // Env file contents (between the heredoc markers) must be sorted.
    let alpha = lines
        .iter()
        .position(|l| l == "ALPHA=a")
        .expect("ALPHA present");
    let mike = lines
        .iter()
        .position(|l| l == "MIKE=m")
        .expect("MIKE present");
    let zeta = lines
        .iter()
        .position(|l| l == "ZETA=z")
        .expect("ZETA present");
    assert!(alpha < mike && mike < zeta, "env file not sorted");
}

#[test]
fn test_services_podman_preserves_list_order() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.ports = vec![
        "9000:9000".to_owned(),
        "8080:8080".to_owned(),
        "7000:7000".to_owned(),
    ];
    svc.volumes = vec!["/z:/z".to_owned(), "/a:/a".to_owned(), "/m:/m".to_owned()];
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let ports: Vec<&String> = lines
        .iter()
        .filter(|l| l.starts_with("PublishPort="))
        .collect();
    assert_eq!(ports.len(), 3);
    assert_eq!(ports[0], "PublishPort=9000:9000");
    assert_eq!(ports[1], "PublishPort=8080:8080");
    assert_eq!(ports[2], "PublishPort=7000:7000");
    let vols: Vec<&String> = lines.iter().filter(|l| l.starts_with("Volume=")).collect();
    assert_eq!(vols.len(), 3);
    assert_eq!(vols[0], "Volume=/z:/z");
    assert_eq!(vols[1], "Volume=/a:/a");
    assert_eq!(vols[2], "Volume=/m:/m");
}

#[test]
fn test_services_podman_no_exec_start_emitted() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    // Quadlet generates the ExecStart — the rendered file must not
    // carry one itself. The shared enable/start tail uses `systemctl`,
    // not `ExecStart=`.
    assert!(
        !lines.iter().any(|l| l.starts_with("ExecStart=")),
        "Podman Quadlet must not emit ExecStart"
    );
}

#[test]
fn test_services_podman_unit_written_to_quadlet_path() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cat > /etc/containers/systemd/api.container")),
        "Podman Quadlet must be written to /etc/containers/systemd/"
    );
    // Must NOT be written to the Docker/native path.
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("/etc/systemd/system/api.service")),
        "Podman services must not write to /etc/systemd/system/"
    );
}

#[test]
fn test_services_podman_env_written_to_env_file_with_mode_600() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.env
        .insert("DB_PASSWORD".to_owned(), "hunter2".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "mkdir -p /etc/harbor/env"),
        "env parent dir must be created"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "cat > /etc/harbor/env/api.env << 'EOF'"),
        "env file heredoc missing"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "chmod 600 /etc/harbor/env/api.env"),
        "chmod 600 missing"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "chown root:root /etc/harbor/env/api.env"),
        "chown root:root missing"
    );
    assert!(
        lines.iter().any(|l| l == "DB_PASSWORD=hunter2"),
        "env file must contain the KEY=VALUE line"
    );
}

#[test]
fn test_services_podman_quadlet_uses_environment_file() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.env
        .insert("DB_PASSWORD".to_owned(), "hunter2".to_owned());
    svc.env.insert("API_KEY".to_owned(), "s3cret".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines
            .iter()
            .any(|l| l == "EnvironmentFile=/etc/harbor/env/api.env"),
        "Quadlet must reference EnvironmentFile="
    );
    assert!(
        !lines.iter().any(|l| l == "Environment=DB_PASSWORD=hunter2"),
        "Quadlet must not inline env via Environment="
    );
    assert!(
        !lines.iter().any(|l| l == "Environment=API_KEY=s3cret"),
        "Quadlet must not inline env via Environment="
    );
}

#[test]
fn test_services_podman_no_env_file_when_empty() {
    let svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    // svc.env is empty by default from container_svc().
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        !lines.iter().any(|l| l.contains("/etc/harbor/env/api.env")),
        "no env file lines must be emitted when env is empty"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("EnvironmentFile=")),
        "no EnvironmentFile= line when env is empty"
    );
}

#[test]
fn test_services_env_file_contents_sorted() {
    // Insert out of order — BTreeMap should sort deterministically
    // in the emitted env file body.
    let mut svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    svc.env.insert("ZETA".to_owned(), "z".to_owned());
    svc.env.insert("ALPHA".to_owned(), "a".to_owned());
    svc.env.insert("MIKE".to_owned(), "m".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    // Find the heredoc start and EOF bounding the env file body.
    let start = lines
        .iter()
        .position(|l| l == "cat > /etc/harbor/env/web.env << 'EOF'")
        .expect("env heredoc start");
    let end = lines[start + 1..]
        .iter()
        .position(|l| l == "EOF")
        .expect("env heredoc EOF")
        + start
        + 1;
    let body: Vec<&String> = lines[start + 1..end].iter().collect();
    assert_eq!(body.len(), 3, "env file body must have 3 lines");
    assert_eq!(body[0], "ALPHA=a");
    assert_eq!(body[1], "MIKE=m");
    assert_eq!(body[2], "ZETA=z");
}

// --- Mixed and auto-enable ---

#[test]
fn test_services_mixed_native_docker_podman() {
    let native = ServiceSpec {
        name: "native-svc".to_owned(),
        enabled: true,
        start: true,
        user: "app".to_owned(),
        working_directory: "/var/lib/app".to_owned(),
        exec_start: "/usr/local/bin/native-svc".to_owned(),
        restart: "always".to_owned(),
        restart_sec: 5,
        image: None,
        runtime: ContainerRuntime::Docker,
        ports: Vec::new(),
        volumes: Vec::new(),
        env: std::collections::BTreeMap::new(),
    };
    let docker = container_svc("docker-svc", "nginx:1", ContainerRuntime::Docker);
    let podman = container_svc("podman-svc", "img:v1", ContainerRuntime::Podman);
    let c = ServicesComponent {
        services: vec![native, docker, podman],
    };
    let lines = c.render();
    // Native unit lives at /etc/systemd/system and uses ExecStart to the binary.
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cat > /etc/systemd/system/native-svc.service"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "ExecStart=/usr/local/bin/native-svc")
    );
    // Docker unit lives at /etc/systemd/system with docker run.
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cat > /etc/systemd/system/docker-svc.service"))
    );
    let docker_exec = lines
        .iter()
        .find(|l| l.starts_with("ExecStart=/usr/bin/docker run"))
        .expect("docker ExecStart");
    assert!(docker_exec.contains("--name docker-svc"));
    // Podman Quadlet lives at /etc/containers/systemd.
    assert!(
        lines
            .iter()
            .any(|l| l.contains("cat > /etc/containers/systemd/podman-svc.container"))
    );
    assert!(lines.iter().any(|l| l == "ContainerName=podman-svc"));
    // All three got enabled + started via the shared tail.
    assert!(lines.iter().any(|l| l == "systemctl enable native-svc"));
    assert!(lines.iter().any(|l| l == "systemctl enable docker-svc"));
    assert!(lines.iter().any(|l| l == "systemctl enable podman-svc"));
}

#[test]
fn test_from_setup_config_auto_adds_docker_component_when_docker_service_present() {
    let yaml = r#"
setup:
  services:
    - name: web
      enabled: true
      image: "nginx:latest"
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
    let script = builder.build();
    assert!(
        script.contains("Setting up Docker"),
        "DockerComponent must be auto-added when a docker-runtime service exists"
    );
    assert!(!script.contains("Setting up Podman"));
}

#[test]
fn test_from_setup_config_auto_adds_podman_component_when_podman_service_present() {
    let yaml = r#"
setup:
  services:
    - name: api
      enabled: true
      image: "quay.io/org/api:v1"
      runtime: podman
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
    let script = builder.build();
    assert!(
        script.contains("Setting up Podman"),
        "PodmanComponent must be auto-added when a podman-runtime service exists"
    );
    assert!(script.contains("apt-get install -y podman"));
    assert!(script.contains("systemctl enable --now podman.socket"));
    assert!(
        !script.contains("Setting up Docker"),
        "DockerComponent must not be added for a pure-podman config"
    );
}

#[test]
fn test_from_setup_config_adds_both_when_mixed() {
    let yaml = r#"
setup:
  services:
    - name: web
      enabled: true
      image: "nginx:latest"
    - name: api
      enabled: true
      image: "quay.io/org/api:v1"
      runtime: podman
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
    let script = builder.build();
    assert!(script.contains("Setting up Docker"));
    assert!(script.contains("Setting up Podman"));
}

#[test]
fn test_from_setup_config_adds_neither_when_no_container_services() {
    let yaml = r#"
setup:
  services:
    - name: native-svc
      enabled: true
      exec_start: /usr/local/bin/native-svc
      user: app
      working_directory: /var/lib/app
      restart: always
      restart_sec: 5
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
    let script = builder.build();
    assert!(!script.contains("Setting up Docker"));
    assert!(!script.contains("Setting up Podman"));
}

// --- Container services: spec 007 gap tests ---

// 1. Docker: empty restart + restart_sec=0 → defaults to always/10
#[test]
fn test_services_docker_unit_restart_defaults() {
    let c = ServicesComponent {
        services: vec![container_svc(
            "web",
            "nginx:latest",
            ContainerRuntime::Docker,
        )],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "Restart=always"),
        "empty restart must default to Restart=always"
    );
    assert!(
        lines.iter().any(|l| l == "RestartSec=10"),
        "restart_sec=0 must default to RestartSec=10"
    );
}

// 2. Docker: explicit restart policy and restart_sec are rendered as-is
#[test]
fn test_services_docker_unit_explicit_restart_policy() {
    let mut svc = container_svc("web", "nginx:latest", ContainerRuntime::Docker);
    svc.restart = "on-failure".to_owned();
    svc.restart_sec = 5;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "Restart=on-failure"),
        "explicit restart must render as-is"
    );
    assert!(
        lines.iter().any(|l| l == "RestartSec=5"),
        "explicit restart_sec must render as-is"
    );
}

// 3. Docker: image is the last token in the docker run command
#[test]
fn test_services_docker_unit_image_is_last_in_run_command() {
    let c = ServicesComponent {
        services: vec![container_svc(
            "web",
            "nginx:latest",
            ContainerRuntime::Docker,
        )],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    let last_token = exec_start
        .split_whitespace()
        .next_back()
        .expect("at least one token");
    assert_eq!(
        last_token, "nginx:latest",
        "image must be the last token in docker run: got {exec_start}"
    );
}

// 4. Docker: flag order is ports → volumes → --env-file → image
// Env values are written to /etc/harbor/env/<name>.env; the run
// command references them via --env-file, not inline -e flags.
#[test]
fn test_services_docker_unit_volume_flag_order() {
    let mut svc = container_svc("web", "nginx:1", ContainerRuntime::Docker);
    svc.ports = vec!["80:80".to_owned()];
    svc.volumes = vec!["/data:/data".to_owned()];
    svc.env.insert("KEY".to_owned(), "val".to_owned());
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    let p_pos = exec_start.find("-p 80:80").expect("-p present");
    let v_pos = exec_start.find("-v /data:/data").expect("-v present");
    let env_file_pos = exec_start
        .find("--env-file /etc/harbor/env/web.env")
        .expect("--env-file present when env non-empty");
    let img_pos = exec_start.find("nginx:1").expect("image present");
    assert!(p_pos < v_pos, "ports must precede volumes: {exec_start}");
    assert!(
        v_pos < env_file_pos,
        "volumes must precede --env-file: {exec_start}"
    );
    assert!(
        env_file_pos < img_pos,
        "--env-file must precede image: {exec_start}"
    );
}

// 5. Docker: no -p/-v and no --env-file when all fields are empty
#[test]
fn test_services_docker_unit_empty_flags_absent() {
    // container_svc already leaves ports/volumes/env empty
    let c = ServicesComponent {
        services: vec![container_svc(
            "web",
            "nginx:latest",
            ContainerRuntime::Docker,
        )],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    assert!(
        !exec_start.contains(" -p "),
        "no -p flag when ports empty: {exec_start}"
    );
    assert!(
        !exec_start.contains(" -v "),
        "no -v flag when volumes empty: {exec_start}"
    );
    assert!(
        !exec_start.contains("--env-file"),
        "no --env-file when env empty: {exec_start}"
    );
}

// 6. Docker: Description= in [Unit] uses the service name (not "name service")
#[test]
fn test_services_docker_unit_description_matches_name() {
    let c = ServicesComponent {
        services: vec![container_svc("myapp", "img:v1", ContainerRuntime::Docker)],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "Description=myapp"),
        "Docker unit Description must equal service name, not 'myapp service'"
    );
}

// 7. Podman: [Unit] declares network-online.target in both After= and Wants=
#[test]
fn test_services_podman_unit_network_online_target() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "After=network-online.target"),
        "Podman unit must have After=network-online.target"
    );
    assert!(
        lines.iter().any(|l| l == "Wants=network-online.target"),
        "Podman unit must have Wants=network-online.target"
    );
}

// 8. Podman: empty restart + restart_sec=0 → defaults to always/10
#[test]
fn test_services_podman_unit_restart_defaults() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "Restart=always"),
        "empty restart must default to Restart=always for Podman"
    );
    assert!(
        lines.iter().any(|l| l == "RestartSec=10"),
        "restart_sec=0 must default to RestartSec=10 for Podman"
    );
}

// 9. Podman: explicit restart policy and restart_sec are rendered as-is
#[test]
fn test_services_podman_unit_explicit_restart_policy() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.restart = "on-failure".to_owned();
    svc.restart_sec = 5;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "Restart=on-failure"),
        "explicit restart must render as-is for Podman"
    );
    assert!(
        lines.iter().any(|l| l == "RestartSec=5"),
        "explicit restart_sec must render as-is for Podman"
    );
}

// 10. Podman: no PublishPort=/Volume=/EnvironmentFile= lines when all fields empty
#[test]
fn test_services_podman_unit_empty_fields_absent() {
    // container_svc leaves ports/volumes/env empty
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        !lines.iter().any(|l| l.starts_with("PublishPort=")),
        "no PublishPort= lines when ports empty"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("Volume=")),
        "no Volume= lines when volumes empty"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("EnvironmentFile=")),
        "no EnvironmentFile= lines when env empty"
    );
}

// 11. Docker: enabled:false → no `systemctl enable` emitted
#[test]
fn test_services_docker_unit_enabled_false_no_enable() {
    let mut svc = container_svc("web", "nginx:latest", ContainerRuntime::Docker);
    svc.enabled = false;
    svc.start = false;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        !lines.iter().any(|l| l.contains("systemctl enable web")),
        "enabled:false must not emit systemctl enable"
    );
}

// 12. Docker: start:false → no `systemctl restart` emitted (enable may still appear)
#[test]
fn test_services_docker_unit_start_false_no_restart() {
    let mut svc = container_svc("web", "nginx:latest", ContainerRuntime::Docker);
    svc.enabled = true;
    svc.start = false;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        !lines.iter().any(|l| l.contains("systemctl restart web")),
        "start:false must not emit systemctl restart"
    );
    assert!(
        lines.iter().any(|l| l.contains("systemctl enable web")),
        "enabled:true must still emit systemctl enable even when start:false"
    );
}

// 13. from_setup_config: Docker component script appears before service unit script
#[test]
fn test_from_setup_config_docker_installed_before_services() {
    let yaml = r#"
setup:
  services:
    - name: web
      image: "nginx:latest"
      enabled: true
"#;
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("parse");
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new(".")).expect("build");
    let script = builder.build();
    let docker_pos = script
        .find("Setting up Docker")
        .expect("Docker setup present");
    let services_pos = script
        .find("Setting up systemd services")
        .expect("services setup present");
    assert!(
        docker_pos < services_pos,
        "Docker install must appear before service unit generation"
    );
}

// 14. Empty-string image is rejected at config load time via
// `ScriptBuilder::from_setup_config`. See
// `config_test::test_from_setup_config_rejects_empty_string_image` —
// the check lives next to the `image` + `exec_start` conflict
// validation in the same pass.
