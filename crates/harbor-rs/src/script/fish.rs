use super::{ScriptComponent, status_echo};

/// Install Fish shell from official PPA.
///
/// `apt-add-repository` ships with `software-properties-common`,
/// which is NOT in base Ubuntu 24.04 minimal images. We install it
/// first so the PPA add always works, regardless of what packages
/// the user declared in `packages:`.
pub struct FishComponent;

impl ScriptComponent for FishComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Installing Fish shell"),
            "apt-get install -y software-properties-common".to_owned(),
            "apt-add-repository -y ppa:fish-shell/release-4".to_owned(),
            "apt-get update".to_owned(),
            "apt-get install -y fish".to_owned(),
            "chsh -s /usr/bin/fish".to_owned(),
        ]
    }
}
