use super::{ScriptComponent, status_echo};

/// Install a specific Go version.
pub struct GoComponent {
    pub version: String,
}

impl ScriptComponent for GoComponent {
    fn render(&self) -> Vec<String> {
        let v = &self.version;
        vec![
            status_echo(&format!("Installing Go {v}")),
            format!(
                "ARCH=$(dpkg --print-architecture) && \
                 if [ \"$ARCH\" = \"arm64\" ]; then GOARCH=\"arm64\"; \
                 else GOARCH=\"amd64\"; fi && \
                 echo \"Downloading Go for architecture: $GOARCH\" && \
                 wget -O go.tar.gz \
                 https://go.dev/dl/go{v}.linux-$GOARCH.tar.gz \
                 || {{ echo 'Go download failed'; exit 1; }}"
            ),
            status_echo("Extracting Go"),
            "rm -rf /usr/local/go && tar -C /usr/local -xzf go.tar.gz \
             || { echo 'Go extraction failed'; exit 1; }"
                .to_owned(),
            "rm go.tar.gz".to_owned(),
            status_echo("Go installation completed"),
        ]
    }
}
