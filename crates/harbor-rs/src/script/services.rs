use crate::config::ServiceSpec;

use super::ScriptComponent;

/// Generate systemd service unit files.
pub struct ServicesComponent {
    pub services: Vec<ServiceSpec>,
}

impl ScriptComponent for ServicesComponent {
    fn render(&self) -> Vec<String> {
        if self.services.is_empty() {
            return Vec::new();
        }

        let mut lines = vec!["echo 'Setting up systemd services'".to_owned()];

        for svc in &self.services {
            if svc.exec_start.is_empty() {
                // Unit file deployed separately (e.g. via files component)
                lines.push(format!("# Configure service {}", svc.name));
                lines.push("systemctl daemon-reload".to_owned());
            } else {
                // Generate unit file
                lines.push(format!("# Create systemd service for {}", svc.name));
                lines.push(format!(
                    "cat > /etc/systemd/system/{}.service << 'EOF'",
                    svc.name
                ));
                lines.push("[Unit]".to_owned());
                lines.push(format!("Description={} service", svc.name));
                lines.push("After=network.target".to_owned());
                lines.push(String::new());
                lines.push("[Service]".to_owned());
                lines.push("Type=simple".to_owned());
                lines.push(format!("User={}", svc.user));
                lines.push(format!("WorkingDirectory={}", svc.working_directory));
                lines.push(format!("ExecStart={}", svc.exec_start));
                lines.push(format!("Restart={}", svc.restart));
                lines.push(format!("RestartSec={}", svc.restart_sec));
                lines.push("StandardOutput=journal".to_owned());
                lines.push("StandardError=journal".to_owned());
                lines.push(String::new());
                lines.push("[Install]".to_owned());
                lines.push("WantedBy=multi-user.target".to_owned());
                lines.push("EOF".to_owned());
                lines.push("systemctl daemon-reload".to_owned());
            }

            if svc.enabled {
                lines.push(format!("systemctl enable {}", svc.name));
            }

            if svc.start {
                lines.push(format!("systemctl start {}", svc.name));
            }

            if svc.enabled || svc.start {
                let action = match (svc.enabled, svc.start) {
                    (true, true) => "enabled and started",
                    (true, false) => "enabled",
                    (false, true) => "started",
                    _ => unreachable!(),
                };
                lines.push(format!("echo 'Service {} {action}'", svc.name));
            }
        }

        lines
    }
}
