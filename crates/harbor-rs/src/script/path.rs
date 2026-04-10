use crate::config::PathMode;

use super::{ScriptComponent, status_echo};

/// Configure the system PATH via `/etc/profile.d/custom-path.sh`.
pub struct PathComponent {
    pub mode: PathMode,
    pub paths: Vec<String>,
}

impl ScriptComponent for PathComponent {
    fn render(&self) -> Vec<String> {
        if self.paths.is_empty() {
            return Vec::new();
        }

        let mut lines = vec![status_echo("Configuring PATH")];

        for p in &self.paths {
            let line = match self.mode {
                PathMode::Prepend => {
                    format!("echo 'export PATH=\"{p}:$PATH\"' >> /etc/profile.d/custom-path.sh")
                }
                PathMode::Append => {
                    format!("echo 'export PATH=\"$PATH:{p}\"' >> /etc/profile.d/custom-path.sh")
                }
                PathMode::Overwrite => {
                    format!("echo 'export PATH=\"{p}\"' >> /etc/profile.d/custom-path.sh")
                }
            };
            lines.push(line);
        }

        lines.push("chmod +x /etc/profile.d/custom-path.sh".to_owned());
        lines.push("source /etc/profile.d/custom-path.sh".to_owned());
        lines
    }
}
