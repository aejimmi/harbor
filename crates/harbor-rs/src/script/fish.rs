use super::{ScriptComponent, status_echo};

/// Install Fish shell from official PPA.
pub struct FishComponent;

impl ScriptComponent for FishComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Installing Fish shell"),
            "apt-add-repository -y ppa:fish-shell/release-4".to_owned(),
            "apt-get update".to_owned(),
            "apt-get install -y fish".to_owned(),
            "chsh -s /usr/bin/fish".to_owned(),
        ]
    }
}
