#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::*;
use crate::config::{DirectorySpec, PathMode, ServiceSpec, UfwRule};

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
            .any(|l| l.contains("export GITHUB_TOKEN='secret123'"))
    );
    assert!(lines.iter().any(|l| l.contains("export APP_NAME='myapp'")));
}

#[test]
fn test_env_component_quotes_shell_metacharacters() {
    let mut vars = std::collections::HashMap::new();
    vars.insert("GREETING".to_owned(), "it's $HOME `id`".to_owned());
    let lines = EnvComponent { vars }.render();
    assert!(
        lines
            .iter()
            .any(|l| l == "export GREETING='it'\\''s $HOME `id`'"),
        "{lines:#?}"
    );
}

#[test]
fn test_files_component_delimiter_avoids_content_lines() {
    let c = FilesComponent {
        files: vec![ResolvedFile {
            target: "/etc/x".to_owned(),
            content: "a\nHARBOR_EOF\nrm -rf /\n".to_owned(),
            owner: String::new(),
            group: String::new(),
            mode: String::new(),
        }],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l == "cat > /etc/x << 'HARBOR_EOF_'"));
    assert!(lines.iter().any(|l| l == "HARBOR_EOF_"));
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
            cap_drop: Vec::new(),
            cap_add: Vec::new(),
            read_only: false,
            pids_limit: 256,
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
            cap_drop: Vec::new(),
            cap_add: Vec::new(),
            read_only: false,
            pids_limit: 256,
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
            cap_drop: Vec::new(),
            cap_add: Vec::new(),
            read_only: false,
            pids_limit: 256,
        }],
    };
    let lines = c.render();
    assert!(lines.iter().any(|l| l.contains("systemctl enable caddy")));
    assert!(lines.iter().any(|l| l.contains("systemctl restart caddy")));
}

#[test]
fn test_ufw_component_adds_ssh_when_missing() {
    let rules = vec![crate::config::UfwRule {
        port: 443,
        proto: "tcp".to_owned(),
        limit: false,
        from: None,
    }];
    let lines = UfwComponent::from_config(&[], &rules).render();
    let ssh = lines.iter().position(|l| l == "ufw limit 22/tcp");
    let enable = lines.iter().position(|l| l == "ufw --force enable");
    assert!(ssh.is_some() && ssh < enable, "{lines:#?}");
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
                from: None,
            },
            UfwRule {
                port: 443,
                proto: "tcp".to_owned(),
                limit: false,
                from: None,
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
fn test_ufw_rule_with_source_restricts_to_it() {
    let rules = vec![UfwRule {
        port: 50000,
        proto: "tcp".to_owned(),
        limit: false,
        from: Some("148.251.183.125".to_owned()),
    }];
    let lines = UfwComponent::from_config(&[], &rules).render();
    assert!(
        lines.contains(&"ufw allow from 148.251.183.125 to any port 50000 proto tcp".to_owned()),
        "{lines:#?}"
    );
    assert!(!lines.iter().any(|l| l == "ufw allow 50000/tcp"));
}

#[test]
fn test_ufw_rule_with_source_and_limit() {
    let rules = vec![UfwRule {
        port: 22,
        proto: "tcp".to_owned(),
        limit: true,
        from: Some("10.0.0.0/8".to_owned()),
    }];
    let lines = UfwComponent::from_config(&[], &rules).render();
    assert!(lines.contains(&"ufw limit from 10.0.0.0/8 to any port 22 proto tcp".to_owned()));
}
