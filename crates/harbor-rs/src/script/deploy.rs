use super::{ScriptComponent, status_echo};

/// Clone a repo (or pull if exists) and run build/install steps.
pub struct DeployComponent {
    pub repo: String,
    pub steps: Vec<String>,
}

impl DeployComponent {
    /// Extract the short repo name from a URL (e.g. `myapp` from `github.com/user/myapp`).
    #[must_use]
    pub fn repo_name(repo: &str) -> &str {
        repo.rsplit('/')
            .next()
            .unwrap_or(repo)
            .trim_end_matches(".git")
    }

    /// Build an HTTPS clone URL from a repo string.
    #[must_use]
    pub fn clone_url(repo: &str) -> String {
        if repo.starts_with("http") {
            repo.to_owned()
        } else {
            format!("https://{repo}")
        }
    }
}

impl ScriptComponent for DeployComponent {
    fn render(&self) -> Vec<String> {
        if self.steps.is_empty() {
            return Vec::new();
        }

        let repo_name = Self::repo_name(&self.repo);
        let clone_url = Self::clone_url(&self.repo);

        let mut lines = vec![
            status_echo(&format!("Deploying {}", self.repo)),
            format!("if [ -d \"$HOME/{repo_name}\" ]; then"),
            format!("  {}", status_echo("Updating existing repo")),
            format!("  cd $HOME/{repo_name} && git pull"),
            "else".to_owned(),
            format!("  {}", status_echo("Cloning repo")),
            format!("  cd $HOME && git clone {clone_url} {repo_name}"),
            "fi".to_owned(),
            format!("cd $HOME/{repo_name}"),
        ];

        for step in &self.steps {
            lines.push(step.clone());
        }

        // Record deploy version
        lines.push("mkdir -p ~/.harbor".to_owned());
        lines.push(
            "echo \"$(date -u +%Y-%m-%dT%H:%M:%SZ) $(whoami) $(git rev-parse HEAD) deploy\" >> ~/.harbor/deploys.log"
                .to_owned(),
        );

        lines.push(status_echo(&format!("Deploy of {} complete", self.repo)));
        lines
    }
}

/// Rollback to a specific git SHA and re-run deploy steps.
pub struct RollbackComponent {
    pub repo: String,
    pub version: String,
    pub steps: Vec<String>,
}

impl ScriptComponent for RollbackComponent {
    fn render(&self) -> Vec<String> {
        let repo_name = DeployComponent::repo_name(&self.repo);

        let mut lines = vec![
            status_echo(&format!("Rolling back to {}", self.version)),
            format!("cd $HOME/{repo_name}"),
            format!("git fetch --all"),
            format!("git checkout {}", self.version),
        ];

        for step in &self.steps {
            lines.push(step.clone());
        }

        // Record rollback version
        lines.push("mkdir -p ~/.harbor".to_owned());
        lines.push(
            "echo \"$(date -u +%Y-%m-%dT%H:%M:%SZ) $(whoami) $(git rev-parse HEAD) rollback\" >> ~/.harbor/deploys.log"
                .to_owned(),
        );

        lines.push(status_echo(&format!(
            "Rollback to {} complete",
            self.version
        )));
        lines
    }
}
