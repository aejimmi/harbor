use crate::config::GithubRepo;

use super::{ScriptComponent, status_echo};

/// Clone, build, and install GitHub repositories.
pub struct GithubReposComponent {
    pub repos: Vec<GithubRepo>,
    pub github_token: String,
    pub system_user: Option<String>,
}

impl ScriptComponent for GithubReposComponent {
    fn render(&self) -> Vec<String> {
        if self.repos.is_empty() {
            return Vec::new();
        }

        let mut lines = vec![
            status_echo("Installing GitHub repositories"),
            "source /etc/environment || true".to_owned(),
            "export GOPATH=/root/go".to_owned(),
            "export PATH=/usr/local/go/bin:$PATH:$GOPATH/bin".to_owned(),
            "# Configure Go for private modules".to_owned(),
            "export GOPRIVATE=\"github.com/usercanal/*\"".to_owned(),
            "export GOSUMDB=off".to_owned(),
        ];

        if self.github_token.is_empty() {
            lines.extend([
                "# Configure git for private repos".to_owned(),
                "echo 'Warning: GITHUB_TOKEN not available, private repo access may fail'"
                    .to_owned(),
            ]);
        } else {
            lines.extend([
                "# Configure git for private repos".to_owned(),
                status_echo("Configuring git with GitHub token"),
                format!("export GITHUB_TOKEN=\"{}\"", self.github_token),
                "git config --global url.\"https://${GITHUB_TOKEN}@github.com/\"\
                 .insteadOf \"https://github.com/\""
                    .to_owned(),
            ]);
        }

        for repo in &self.repos {
            self.render_repo(repo, &mut lines);
        }

        lines
    }
}

impl GithubReposComponent {
    fn render_repo(&self, repo: &GithubRepo, lines: &mut Vec<String>) {
        let base_repo = extract_base_repo(&repo.repo);
        let binary = &repo.binary;

        lines.push(status_echo(&format!("Installing {}", repo.repo)));
        lines.push("mkdir -p $GOPATH/bin || true".to_owned());
        lines.push("# Clone and build repository locally due to replace directives".to_owned());
        lines.push(format!("cd /tmp && rm -rf build-{binary}"));
        lines.push(format!("git clone https://{base_repo}.git build-{binary}"));
        lines.push(format!("cd build-{binary}"));

        if base_repo.contains("usercanal/usercanal") {
            lines.extend([
                "# Clone dependent repositories for replace directives".to_owned(),
                "git clone https://github.com/usercanal/cdp-collector.git cdp-collector \
                 || echo 'cdp-collector clone failed'"
                    .to_owned(),
                "git clone https://github.com/usercanal/cdp-api.git cdp-api \
                 || echo 'cdp-api clone failed'"
                    .to_owned(),
            ]);
        }

        let build_path = extract_build_path(&repo.repo);
        lines.push(format!(
            "/usr/local/go/bin/go build -o $GOPATH/bin/{binary} {build_path} \
             || {{ echo 'Failed to build {}'; exit 1; }}",
            repo.repo
        ));

        if binary.contains("usercanal") {
            self.render_usercanal_config(repo, lines);
        }

        lines.push(format!("cd /tmp && rm -rf build-{binary}"));

        if !repo.install_path.is_empty() && !binary.is_empty() {
            lines.extend([
                format!("if [ -f \"$GOPATH/bin/{binary}\" ]; then"),
                format!("  cp $GOPATH/bin/{binary} {}/{binary}", repo.install_path),
                format!("  chmod +x {}/{binary}", repo.install_path),
                format!(
                    "  {}",
                    status_echo(&format!(
                        "Successfully installed {binary} to {}",
                        repo.install_path
                    ))
                ),
                "else".to_owned(),
                format!("  echo 'Warning: {binary} binary not found in $GOPATH/bin'"),
                "fi".to_owned(),
            ]);
        }
    }

    fn render_usercanal_config(&self, repo: &GithubRepo, lines: &mut Vec<String>) {
        lines.extend([
            "# Install usercanal configuration files".to_owned(),
            "mkdir -p /etc/usercanal".to_owned(),
        ]);

        if !repo.config_source.is_empty() && !repo.config_target.is_empty() {
            lines.push(format!(
                "cp {} {} || echo 'Specific config copy failed'",
                repo.config_source, repo.config_target
            ));
            lines.push(
                "cp configs/apikeys.conf /etc/usercanal/ || echo 'API keys copy failed'".to_owned(),
            );
        } else {
            lines.push("cp -r configs/* /etc/usercanal/ || echo 'Config copy failed'".to_owned());
        }

        let owner = self
            .system_user
            .as_deref()
            .map_or("root:root".to_owned(), |u| format!("{u}:{u}"));

        lines.extend([
            format!("chown -R {owner} /etc/usercanal"),
            "chmod -R 644 /etc/usercanal/*".to_owned(),
            status_echo("Configuration files installed to /etc/usercanal/"),
        ]);
    }
}

/// Extract the base repo path, removing `/cmd/binary` if present.
fn extract_base_repo(repo: &str) -> &str {
    repo.split("/cmd/").next().unwrap_or(repo)
}

/// Extract the build path within the repo.
fn extract_build_path(repo: &str) -> String {
    if let Some((_base, cmd_part)) = repo.split_once("/cmd/") {
        format!("./cmd/{cmd_part}")
    } else {
        ".".to_owned()
    }
}
