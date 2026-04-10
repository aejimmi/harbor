use std::fmt::Write;

use super::ScriptComponent;

/// Create a system user account.
pub struct SystemUserComponent {
    pub name: String,
    pub home: String,
    pub shell: String,
}

impl ScriptComponent for SystemUserComponent {
    fn render(&self) -> Vec<String> {
        let mut cmd = "useradd --system".to_owned();

        if self.home.is_empty() {
            cmd.push_str(" --no-create-home");
        } else {
            let _ = write!(cmd, " --home-dir {} --create-home", self.home);
        }

        if !self.shell.is_empty() {
            let _ = write!(cmd, " --shell {}", self.shell);
        }

        let _ = write!(cmd, " {} || true", self.name);

        vec![
            "echo 'Creating system user'".to_owned(),
            cmd,
            // Verify the user exists (fail loud if it doesn't)
            format!(
                "id {} > /dev/null 2>&1 || {{ echo 'Failed to create user {}'; exit 1; }}",
                self.name, self.name
            ),
        ]
    }
}
