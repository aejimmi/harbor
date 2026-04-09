use super::ScriptComponent;

/// A resolved file to deploy: content already read from source.
pub struct ResolvedFile {
    pub target: String,
    pub content: String,
    pub owner: String,
    pub group: String,
    pub mode: String,
}

/// Deploy local files to server paths via heredoc.
pub struct FilesComponent {
    pub files: Vec<ResolvedFile>,
}

impl ScriptComponent for FilesComponent {
    fn render(&self) -> Vec<String> {
        if self.files.is_empty() {
            return Vec::new();
        }

        let mut lines = vec!["echo 'Deploying configuration files'".to_owned()];

        for file in &self.files {
            lines.push(format!("mkdir -p $(dirname {})", file.target));
            lines.push(format!("cat > {} << 'HARBOR_EOF'", file.target));
            for line in file.content.lines() {
                lines.push(line.to_owned());
            }
            lines.push("HARBOR_EOF".to_owned());

            if !file.owner.is_empty() && !file.group.is_empty() {
                lines.push(format!(
                    "chown {}:{} {}",
                    file.owner, file.group, file.target
                ));
            }
            if !file.mode.is_empty() {
                lines.push(format!("chmod {} {}", file.mode, file.target));
            }

            lines.push(format!("echo 'Deployed {}'", file.target));
        }

        lines
    }
}
