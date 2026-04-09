use super::ScriptComponent;

/// Install fail2ban-rs from GitHub release script.
pub struct Fail2banRsComponent;

impl ScriptComponent for Fail2banRsComponent {
    fn render(&self) -> Vec<String> {
        vec![
            "echo 'Installing fail2ban-rs'".to_owned(),
            "curl -sSfL https://raw.githubusercontent.com/aejimmi/fail2ban-rs/main/scripts/install.sh | bash"
                .to_owned(),
            "systemctl enable fail2ban-rs".to_owned(),
        ]
    }
}
