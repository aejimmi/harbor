use super::ScriptComponent;

/// Create a system user account.
pub struct SystemUserComponent {
    pub name: String,
    pub home: String,
    pub shell: String,
}

impl ScriptComponent for SystemUserComponent {
    fn render(&self) -> Vec<String> {
        vec![
            "echo 'Creating system user'".to_owned(),
            format!(
                "useradd --system --home-dir {} --shell {} --create-home {} \
                 || echo 'User already exists'",
                self.home, self.shell, self.name
            ),
        ]
    }
}
