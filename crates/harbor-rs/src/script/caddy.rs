use super::{ScriptComponent, status_echo};

/// Install Caddy web server from official repository.
pub struct CaddyComponent;

impl ScriptComponent for CaddyComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Installing Caddy"),
            "apt-get install -y debian-keyring debian-archive-keyring apt-transport-https"
                .to_owned(),
            "curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
             | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg"
                .to_owned(),
            "curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
             | tee /etc/apt/sources.list.d/caddy-stable.list > /dev/null"
                .to_owned(),
            "apt-get update".to_owned(),
            "apt-get install -y caddy".to_owned(),
        ]
    }
}
