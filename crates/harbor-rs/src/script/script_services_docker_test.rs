#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::container_svc;
use super::*;
use crate::config::ContainerRuntime;

// --- Container service render tests (spec 007) ---

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
