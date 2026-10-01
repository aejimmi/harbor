#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::container_svc;
use super::*;
use crate::config::{ContainerRuntime, ServiceSpec, SetupConfig};
use std::path::Path;

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
        cap_drop: Vec::new(),
        cap_add: Vec::new(),
        read_only: false,
        pids_limit: 256,
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
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
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
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
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
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
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
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
    let script = builder.build();
    assert!(!script.contains("Setting up Docker"));
    assert!(!script.contains("Setting up Podman"));
}
