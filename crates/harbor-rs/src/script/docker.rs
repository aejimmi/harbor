use super::ScriptComponent;

/// Install Docker CE and plugins.
pub struct DockerComponent;

impl ScriptComponent for DockerComponent {
    fn render(&self) -> Vec<String> {
        vec![
            "echo 'Setting up Docker'".to_owned(),
            "install -m 0755 -d /etc/apt/keyrings".to_owned(),
            "curl -fsSL https://download.docker.com/linux/ubuntu/gpg \
             | gpg --dearmor -o /etc/apt/keyrings/docker.gpg"
                .to_owned(),
            "chmod a+r /etc/apt/keyrings/docker.gpg".to_owned(),
            "echo \"deb [arch=$(dpkg --print-architecture) \
             signed-by=/etc/apt/keyrings/docker.gpg] \
             https://download.docker.com/linux/ubuntu \
             $(. /etc/os-release && echo $VERSION_CODENAME) stable\" \
             | tee /etc/apt/sources.list.d/docker.list > /dev/null"
                .to_owned(),
            "apt-get update".to_owned(),
            "apt-get install -y docker-ce docker-ce-cli containerd.io \
             docker-buildx-plugin docker-compose-plugin"
                .to_owned(),
            "systemctl enable docker".to_owned(),
            "systemctl start docker".to_owned(),
        ]
    }
}
