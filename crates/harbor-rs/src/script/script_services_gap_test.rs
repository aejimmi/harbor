#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::container_svc;
use super::*;
use crate::config::{ContainerRuntime, SetupConfig};
use std::path::Path;

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
    let builder =
        ScriptBuilder::from_setup_config(&config, "", Path::new("."), None).expect("build");
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
// `config_setup_test::test_from_setup_config_rejects_empty_string_image` —
// the check lives next to the `image` + `exec_start` conflict
// validation in the same pass.
