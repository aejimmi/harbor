use super::ScriptComponent;

/// Clone a repo (or pull if exists) and run build/install steps.
pub struct DeployComponent {
    pub repo: String,
    pub steps: Vec<String>,
}

impl ScriptComponent for DeployComponent {
    fn render(&self) -> Vec<String> {
        if self.steps.is_empty() {
            return Vec::new();
        }

        let repo_name = self
            .repo
            .rsplit('/')
            .next()
            .unwrap_or(&self.repo)
            .trim_end_matches(".git");

        let clone_url = if self.repo.starts_with("http") {
            self.repo.clone()
        } else {
            format!("https://{}", self.repo)
        };

        let mut lines = vec![
            format!("echo 'Deploying {}'", self.repo),
            format!("if [ -d \"$HOME/{repo_name}\" ]; then"),
            format!("  echo 'Updating existing repo'"),
            format!("  cd $HOME/{repo_name} && git pull"),
            "else".to_owned(),
            format!("  echo 'Cloning repo'"),
            format!("  cd $HOME && git clone {clone_url} {repo_name}"),
            "fi".to_owned(),
            format!("cd $HOME/{repo_name}"),
        ];

        for step in &self.steps {
            lines.push(step.clone());
        }

        lines.push(format!("echo 'Deploy of {} complete'", self.repo));
        lines
    }
}
