#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use std::path::Path;

use super::*;
use crate::config::{DirectorySpec, PathMode, ServiceSpec, SetupConfig, UfwRule};

#[test]
fn test_empty_builder_produces_valid_script() {
    let script = ScriptBuilder::new().build();
    assert!(script.starts_with("#!/bin/bash\nset -e"));
    assert!(script.contains("apt-get update"));
    assert!(script.ends_with("echo 'Setup completed successfully'"));
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
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new("."));
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
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new("."));
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
    let builder = ScriptBuilder::from_setup_config(&config, "", Path::new("."));
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
    let builder = ScriptBuilder::from_setup_config(&config, "gh_token_123", Path::new("."));
    let script = builder.build();

    assert!(script.contains("GITHUB_TOKEN=\"gh_token_123\""));
    assert!(script.contains("go build -o $GOPATH/bin/mybin"));
}
