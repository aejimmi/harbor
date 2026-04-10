use super::{ScriptComponent, status_echo};

/// Configure git HTTPS authentication via token.
pub struct GitAuthComponent {
    pub token: String,
}

impl ScriptComponent for GitAuthComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Configuring git HTTPS authentication"),
            format!(
                "git config --global url.\"https://x-access-token:{}@github.com/\".insteadOf \"https://github.com/\"",
                self.token
            ),
        ]
    }
}
