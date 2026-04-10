use super::ScriptComponent;

/// Configure git HTTPS authentication via token.
pub struct GitAuthComponent {
    pub token: String,
}

impl ScriptComponent for GitAuthComponent {
    fn render(&self) -> Vec<String> {
        vec![
            "echo 'Configuring git HTTPS authentication'".to_owned(),
            format!(
                "git config --global url.\"https://x-access-token:{}@github.com/\".insteadOf \"https://github.com/\"",
                self.token
            ),
        ]
    }
}
