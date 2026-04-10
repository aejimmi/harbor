use std::collections::HashMap;

use super::{ScriptComponent, status_echo};

/// Set environment variables in `/etc/environment`.
pub struct EnvComponent {
    pub vars: HashMap<String, String>,
}

impl ScriptComponent for EnvComponent {
    fn render(&self) -> Vec<String> {
        if self.vars.is_empty() {
            return Vec::new();
        }

        let mut lines = vec![status_echo("Setting up environment variables")];

        for (key, value) in &self.vars {
            let upper_key = key.to_uppercase();
            let is_sensitive = upper_key.contains("TOKEN") || upper_key.contains("KEY");

            if is_sensitive {
                lines.push(status_echo(&format!("Setting up {key}")));
            }
            lines.push(format!(
                "echo 'export {key}=\"{value}\"' >> /etc/environment"
            ));
            lines.push(format!("export {key}=\"{value}\""));
        }

        lines
    }
}
