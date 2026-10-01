#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::script_test_helpers::container_svc;
use super::*;
use crate::config::ContainerRuntime;

// --- Container security hardening tests ---

// 15. Docker: --security-opt=no-new-privileges is always present
#[test]
fn test_services_docker_unit_no_new_privileges() {
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
        exec_start.contains("--security-opt=no-new-privileges"),
        "must always include --security-opt=no-new-privileges: {exec_start}"
    );
}

// 16. Docker: --pids-limit=256 is always present
#[test]
fn test_services_docker_unit_pids_limit() {
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
        exec_start.contains("--pids-limit=256"),
        "must always include --pids-limit=256: {exec_start}"
    );
}

// 17. Docker: cap_drop renders --cap-drop flags
#[test]
fn test_services_docker_unit_cap_drop() {
    let mut svc = container_svc("web", "nginx:latest", ContainerRuntime::Docker);
    svc.cap_drop = vec!["ALL".to_owned()];
    svc.cap_add = vec!["NET_BIND_SERVICE".to_owned()];
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    assert!(
        exec_start.contains("--cap-drop=ALL"),
        "cap_drop must render: {exec_start}"
    );
    assert!(
        exec_start.contains("--cap-add=NET_BIND_SERVICE"),
        "cap_add must render: {exec_start}"
    );
}

// 18. Docker: read_only renders --read-only and --tmpfs /tmp
#[test]
fn test_services_docker_unit_read_only() {
    let mut svc = container_svc("web", "nginx:latest", ContainerRuntime::Docker);
    svc.read_only = true;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    assert!(
        exec_start.contains("--read-only"),
        "read_only must render --read-only: {exec_start}"
    );
    assert!(
        exec_start.contains("--tmpfs /tmp:size=64m"),
        "read_only must add size-bounded --tmpfs /tmp: {exec_start}"
    );
}

// 18b. Docker: pids_limit=0 disables --pids-limit flag
#[test]
fn test_services_docker_unit_pids_limit_zero_disables() {
    let mut svc = container_svc("web", "nginx:latest", ContainerRuntime::Docker);
    svc.pids_limit = 0;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    assert!(
        !exec_start.contains("--pids-limit"),
        "pids_limit=0 must omit --pids-limit: {exec_start}"
    );
}

// 18c. Docker: custom pids_limit renders the user value
#[test]
fn test_services_docker_unit_pids_limit_custom() {
    let mut svc = container_svc("db", "postgres:16", ContainerRuntime::Docker);
    svc.pids_limit = 1024;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    let exec_start = lines
        .iter()
        .find(|l| l.starts_with("ExecStart="))
        .expect("ExecStart line");
    assert!(
        exec_start.contains("--pids-limit=1024"),
        "custom pids_limit must render: {exec_start}"
    );
}

// 19. Docker: no cap_drop/cap_add/read_only flags when defaults
#[test]
fn test_services_docker_unit_no_security_flags_when_default() {
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
        !exec_start.contains("--cap-drop"),
        "no --cap-drop when cap_drop empty: {exec_start}"
    );
    assert!(
        !exec_start.contains("--cap-add"),
        "no --cap-add when cap_add empty: {exec_start}"
    );
    assert!(
        !exec_start.contains("--read-only"),
        "no --read-only when read_only false: {exec_start}"
    );
}

// 20. Podman: NoNewPrivileges=true in [Service] section
#[test]
fn test_services_podman_unit_no_new_privileges() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "NoNewPrivileges=true"),
        "Podman unit must have NoNewPrivileges=true in [Service]"
    );
}

// 21. Podman: PidsLimit=256 in [Container] section
#[test]
fn test_services_podman_unit_pids_limit() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "PidsLimit=256"),
        "Podman unit must have PidsLimit=256 in [Container]"
    );
}

// 22. Podman: DropCapability / AddCapability rendered
#[test]
fn test_services_podman_unit_cap_drop_add() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.cap_drop = vec!["ALL".to_owned()];
    svc.cap_add = vec!["CHOWN".to_owned(), "SETUID".to_owned()];
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "DropCapability=ALL"),
        "Podman must render DropCapability=ALL"
    );
    assert!(
        lines.iter().any(|l| l == "AddCapability=CHOWN"),
        "Podman must render AddCapability=CHOWN"
    );
    assert!(
        lines.iter().any(|l| l == "AddCapability=SETUID"),
        "Podman must render AddCapability=SETUID"
    );
}

// 23. Podman: ReadOnly=true and Tmpfs=/tmp when read_only
#[test]
fn test_services_podman_unit_read_only() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.read_only = true;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "ReadOnly=true"),
        "Podman must render ReadOnly=true"
    );
    assert!(
        lines.iter().any(|l| l == "Tmpfs=/tmp:size=64m"),
        "Podman must render size-bounded Tmpfs=/tmp when read_only"
    );
}

// 23b. Podman: pids_limit=0 disables PidsLimit
#[test]
fn test_services_podman_unit_pids_limit_zero_disables() {
    let mut svc = container_svc("api", "img:v1", ContainerRuntime::Podman);
    svc.pids_limit = 0;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        !lines.iter().any(|l| l.starts_with("PidsLimit=")),
        "pids_limit=0 must omit PidsLimit="
    );
}

// 23c. Podman: custom pids_limit renders the user value
#[test]
fn test_services_podman_unit_pids_limit_custom() {
    let mut svc = container_svc("db", "postgres:16", ContainerRuntime::Podman);
    svc.pids_limit = 1024;
    let c = ServicesComponent {
        services: vec![svc],
    };
    let lines = c.render();
    assert!(
        lines.iter().any(|l| l == "PidsLimit=1024"),
        "custom pids_limit must render PidsLimit=1024"
    );
}

// 24. Podman: no DropCapability/ReadOnly when defaults
#[test]
fn test_services_podman_unit_no_security_flags_when_default() {
    let c = ServicesComponent {
        services: vec![container_svc("api", "img:v1", ContainerRuntime::Podman)],
    };
    let lines = c.render();
    assert!(
        !lines.iter().any(|l| l.starts_with("DropCapability=")),
        "no DropCapability= when cap_drop empty"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("AddCapability=")),
        "no AddCapability= when cap_add empty"
    );
    assert!(
        !lines.iter().any(|l| l == "ReadOnly=true"),
        "no ReadOnly= when read_only false"
    );
}
