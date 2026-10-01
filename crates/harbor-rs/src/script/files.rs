use super::{ScriptComponent, status_echo};

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

        let mut lines = vec![status_echo("Deploying configuration files")];

        for file in &self.files {
            lines.push(format!("mkdir -p $(dirname {})", file.target));
            let delim = heredoc_delimiter(&file.content);
            lines.push(format!("cat > {} << '{delim}'", file.target));
            lines.extend(file.content.lines().map(ToOwned::to_owned));
            lines.push(delim);

            if !file.owner.is_empty() && !file.group.is_empty() {
                lines.push(format!(
                    "chown {}:{} {}",
                    file.owner, file.group, file.target
                ));
            }
            if !file.mode.is_empty() {
                lines.push(format!("chmod {} {}", file.mode, file.target));
            }

            lines.push(status_echo(&format!("Deployed {}", file.target)));
        }

        lines
    }
}

/// `HARBOR_EOF`, suffixed until no line of `content` equals it — a file
/// line matching the delimiter would end the heredoc early and run the
/// rest of the file as root bash.
fn heredoc_delimiter(content: &str) -> String {
    let mut delim = "HARBOR_EOF".to_owned();
    while content.lines().any(|l| l == delim) {
        delim.push('_');
    }
    delim
}
