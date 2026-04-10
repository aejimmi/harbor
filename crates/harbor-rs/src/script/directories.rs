use crate::config::DirectorySpec;

use super::{ScriptComponent, status_echo};

/// Create directories with specified ownership and permissions.
pub struct DirectoriesComponent {
    pub dirs: Vec<DirectorySpec>,
}

impl ScriptComponent for DirectoriesComponent {
    fn render(&self) -> Vec<String> {
        if self.dirs.is_empty() {
            return Vec::new();
        }

        let mut lines = vec![status_echo("Creating directories")];

        for dir in &self.dirs {
            lines.push(format!("mkdir -p {}", dir.path));
            lines.push(format!("chown {}:{} {}", dir.owner, dir.group, dir.path));
            lines.push(format!("chmod {} {}", dir.mode, dir.path));
        }

        lines
    }
}
