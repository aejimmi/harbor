use crate::config::{ContainerRuntime, ServiceSpec};

use super::{ScriptComponent, status_echo};

/// Generate systemd service unit files.
pub struct ServicesComponent {
    pub services: Vec<ServiceSpec>,
}

impl ScriptComponent for ServicesComponent {
    fn render(&self) -> Vec<String> {
        if self.services.is_empty() {
            return Vec::new();
        }

        let mut lines = vec![status_echo("Setting up systemd services")];

        for svc in &self.services {
            if svc.image.is_some() {
                match svc.runtime {
                    ContainerRuntime::Docker => render_docker_unit(svc, &mut lines),
                    ContainerRuntime::Podman => render_podman_quadlet(svc, &mut lines),
                }
            } else {
                render_native_unit(svc, &mut lines);
            }
            render_enable_start(svc, &mut lines);
        }

        lines
    }
}

/// Render a native systemd unit — the pre-container code path, unchanged.
fn render_native_unit(svc: &ServiceSpec, lines: &mut Vec<String>) {
    if svc.exec_start.is_empty() {
        lines.push(format!("# Configure service {}", svc.name));
        lines.push("systemctl daemon-reload".to_owned());
        return;
    }
    lines.push(format!("# Create systemd service for {}", svc.name));
    lines.push(format!(
        "cat > /etc/systemd/system/{}.service << 'EOF'",
        svc.name
    ));
    lines.extend(native_unit_body(svc));
    lines.push("EOF".to_owned());
    lines.push("systemctl daemon-reload".to_owned());
}

/// Body of a native systemd `.service` file — the text between the
/// heredoc markers. Extracted so `render_native_unit` stays short.
fn native_unit_body(svc: &ServiceSpec) -> Vec<String> {
    vec![
        "[Unit]".to_owned(),
        format!("Description={} service", svc.name),
        "After=network.target".to_owned(),
        String::new(),
        "[Service]".to_owned(),
        "Type=simple".to_owned(),
        format!("User={}", svc.user),
        format!("WorkingDirectory={}", svc.working_directory),
        format!("ExecStart={}", svc.exec_start),
        format!("Restart={}", svc.restart),
        format!("RestartSec={}", svc.restart_sec),
        "StandardOutput=journal".to_owned(),
        "StandardError=journal".to_owned(),
        String::new(),
        "[Install]".to_owned(),
        "WantedBy=multi-user.target".to_owned(),
    ]
}

/// Render a Docker-backed systemd unit to
/// `/etc/systemd/system/<name>.service`.
///
/// When `svc.env` is non-empty, a `/etc/harbor/env/<name>.env` file is
/// written with mode `0600 root:root` before the unit so that the unit
/// can reference it via `--env-file` without leaking secrets through
/// the world-readable systemd directory.
fn render_docker_unit(svc: &ServiceSpec, lines: &mut Vec<String>) {
    let image = svc.image.as_deref().unwrap_or_default();
    let name = &svc.name;
    if !svc.env.is_empty() {
        push_env_file_script(svc, lines);
    }
    lines.push(format!("# Create Docker systemd service for {name}"));
    lines.push(format!("cat > /etc/systemd/system/{name}.service << 'EOF'"));
    lines.extend(docker_unit_body(svc, image));
    lines.push("EOF".to_owned());
    lines.push("systemctl daemon-reload".to_owned());
}

/// Body of a Docker `.service` file. `--log-driver=journald` is
/// hard-coded and `docker run` has `--rm` but no `--restart` — systemd
/// is the sole supervisor.
fn docker_unit_body(svc: &ServiceSpec, image: &str) -> Vec<String> {
    let (restart, restart_sec) = restart_defaults(svc);
    let name = &svc.name;
    vec![
        "[Unit]".to_owned(),
        format!("Description={name}"),
        "After=network-online.target docker.service".to_owned(),
        "Requires=docker.service".to_owned(),
        String::new(),
        "[Service]".to_owned(),
        format!("Restart={restart}"),
        format!("RestartSec={restart_sec}"),
        format!("ExecStartPre=-/usr/bin/docker rm -f {name}"),
        format!("ExecStartPre=-/usr/bin/docker pull {image}"),
        format!("ExecStart={}", docker_run_command(svc, image)),
        format!("ExecStop=/usr/bin/docker stop -t 10 {name}"),
        String::new(),
        "[Install]".to_owned(),
        "WantedBy=multi-user.target".to_owned(),
    ]
}

/// Build the `docker run` command line from a container `ServiceSpec`.
///
/// Flag order: `--rm`, `--name`, `--log-driver=journald`, then ports in
/// declaration order, volumes in declaration order, a single
/// `--env-file` reference (only if `svc.env` is non-empty), then the
/// image. Env values are never rendered inline — they live in
/// `/etc/harbor/env/<name>.env` with mode `0600 root:root`.
fn docker_run_command(svc: &ServiceSpec, image: &str) -> String {
    let mut parts: Vec<String> = vec![
        "/usr/bin/docker run --rm".to_owned(),
        format!("--name {}", svc.name),
        "--log-driver=journald".to_owned(),
    ];
    for port in &svc.ports {
        parts.push(format!("-p {port}"));
    }
    for vol in &svc.volumes {
        parts.push(format!("-v {vol}"));
    }
    if !svc.env.is_empty() {
        parts.push(format!("--env-file /etc/harbor/env/{}.env", svc.name));
    }
    parts.push(image.to_owned());
    parts.join(" ")
}

/// Render a Podman Quadlet `.container` file to
/// `/etc/containers/systemd/<name>.container`.
///
/// When `svc.env` is non-empty, `/etc/harbor/env/<name>.env` is written
/// with mode `0600 root:root` before the `.container` file and the unit
/// references it through `EnvironmentFile=`. Env values are never
/// inlined into the Quadlet file, which lives in a world-readable
/// directory.
fn render_podman_quadlet(svc: &ServiceSpec, lines: &mut Vec<String>) {
    let image = svc.image.as_deref().unwrap_or_default();
    let name = &svc.name;
    if !svc.env.is_empty() {
        push_env_file_script(svc, lines);
    }
    lines.push(format!("# Create Podman Quadlet for {name}"));
    lines.push("mkdir -p /etc/containers/systemd".to_owned());
    lines.push(format!(
        "cat > /etc/containers/systemd/{name}.container << 'EOF'"
    ));
    lines.extend(podman_quadlet_body(svc, image));
    lines.push("EOF".to_owned());
    lines.push("systemctl daemon-reload".to_owned());
}

/// Body of a Podman `.container` Quadlet file. The `[Service]` section
/// deliberately has no `ExecStart=` — Quadlet writes one from the
/// `[Container]` stanza at `daemon-reload` time.
fn podman_quadlet_body(svc: &ServiceSpec, image: &str) -> Vec<String> {
    let (restart, restart_sec) = restart_defaults(svc);
    let name = &svc.name;
    let mut body = vec![
        "[Unit]".to_owned(),
        format!("Description={name}"),
        "After=network-online.target".to_owned(),
        "Wants=network-online.target".to_owned(),
        String::new(),
        "[Container]".to_owned(),
        format!("Image={image}"),
        format!("ContainerName={name}"),
    ];
    append_quadlet_container_lists(svc, &mut body);
    body.extend([
        String::new(),
        "[Service]".to_owned(),
        format!("Restart={restart}"),
        format!("RestartSec={restart_sec}"),
        String::new(),
        "[Install]".to_owned(),
        "WantedBy=multi-user.target".to_owned(),
    ]);
    body
}

/// Append `PublishPort=`, `Volume=`, and `EnvironmentFile=` entries to
/// a Quadlet `[Container]` section body.
///
/// Env values are never inlined via `Environment=KEY=VAL`. When
/// `svc.env` is non-empty, a single `EnvironmentFile=` line is
/// emitted pointing at `/etc/harbor/env/<name>.env`, which lives
/// outside the world-readable Quadlet directory.
fn append_quadlet_container_lists(svc: &ServiceSpec, body: &mut Vec<String>) {
    for port in &svc.ports {
        body.push(format!("PublishPort={port}"));
    }
    for vol in &svc.volumes {
        body.push(format!("Volume={vol}"));
    }
    if !svc.env.is_empty() {
        body.push(format!("EnvironmentFile=/etc/harbor/env/{}.env", svc.name));
    }
}

/// Emit bash to write `/etc/harbor/env/<name>.env` with mode `0600`
/// and root ownership. Content is one `KEY=value` per line in sorted
/// (`BTreeMap`) order. Caller must only invoke this when `svc.env` is
/// non-empty.
///
/// The env file is the only place secret values touch disk; the unit
/// files themselves reference it via `--env-file` (Docker) or
/// `EnvironmentFile=` (Podman Quadlet) so the world-readable systemd
/// directories never hold the plaintext values.
fn push_env_file_script(svc: &ServiceSpec, lines: &mut Vec<String>) {
    let name = &svc.name;
    lines.push(format!("# Write env file for {name}"));
    lines.push("mkdir -p /etc/harbor/env".to_owned());
    lines.push(format!("cat > /etc/harbor/env/{name}.env << 'EOF'"));
    for (key, val) in &svc.env {
        lines.push(format!("{key}={val}"));
    }
    lines.push("EOF".to_owned());
    lines.push(format!("chmod 600 /etc/harbor/env/{name}.env"));
    lines.push(format!("chown root:root /etc/harbor/env/{name}.env"));
}

/// Resolve `restart` and `restart_sec` with the container-unit defaults
/// (`always`, `10`).
fn restart_defaults(svc: &ServiceSpec) -> (&str, u32) {
    let restart = if svc.restart.is_empty() {
        "always"
    } else {
        svc.restart.as_str()
    };
    let restart_sec = if svc.restart_sec == 0 {
        10
    } else {
        svc.restart_sec
    };
    (restart, restart_sec)
}

/// Emit the shared `systemctl enable`/`restart`/echo tail used by all
/// three render paths (native, Docker, Podman Quadlet).
fn render_enable_start(svc: &ServiceSpec, lines: &mut Vec<String>) {
    if svc.enabled {
        lines.push(format!("systemctl enable {}", svc.name));
    }
    if svc.start {
        lines.push(format!("systemctl restart {}", svc.name));
    }
    if svc.enabled || svc.start {
        let action = match (svc.enabled, svc.start) {
            (true, true) => "enabled and started",
            (true, false) => "enabled",
            (false, true) => "started",
            _ => unreachable!(),
        };
        lines.push(status_echo(&format!("Service {} {action}", svc.name)));
    }
}
