use crate::config::UfwRule;

use super::ScriptComponent;

/// Configure UFW firewall rules.
pub struct UfwComponent {
    pub rules: Vec<UfwRule>,
}

impl UfwComponent {
    /// Build from legacy `allow_ports` (backward compat) or rich `rules`.
    pub fn from_config(allow_ports: &[u16], rules: &[UfwRule]) -> Self {
        if rules.is_empty() {
            Self {
                rules: allow_ports
                    .iter()
                    .map(|&port| UfwRule {
                        port,
                        proto: "tcp".to_owned(),
                        limit: false,
                    })
                    .collect(),
            }
        } else {
            Self {
                rules: rules.to_vec(),
            }
        }
    }
}

impl ScriptComponent for UfwComponent {
    fn render(&self) -> Vec<String> {
        let mut lines = vec![
            "echo 'Configuring UFW'".to_owned(),
            "ufw --force reset".to_owned(),
            "ufw default deny incoming".to_owned(),
            "ufw default allow outgoing".to_owned(),
        ];

        for rule in &self.rules {
            let port_proto = format!("{}/{}", rule.port, rule.proto);
            if rule.limit {
                // Rate limiting (e.g. SSH: 6 connections/30s per IP)
                lines.push(format!("ufw allow {port_proto}"));
                lines.push(format!("ufw limit {port_proto}"));
            } else {
                lines.push(format!("ufw allow {port_proto}"));
            }
        }

        lines.push("ufw --force enable".to_owned());
        lines
    }
}
