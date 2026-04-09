use super::ScriptComponent;

/// Install a specific Go version.
pub struct GoComponent {
    pub version: String,
}

impl ScriptComponent for GoComponent {
    fn render(&self) -> Vec<String> {
        let v = &self.version;
        vec![
            format!("echo 'Installing Go {v}'"),
            format!(
                "ARCH=$(dpkg --print-architecture) && \
                 if [ \"$ARCH\" = \"arm64\" ]; then GOARCH=\"arm64\"; \
                 else GOARCH=\"amd64\"; fi && \
                 echo \"Downloading Go for architecture: $GOARCH\" && \
                 wget -O go.tar.gz \
                 https://go.dev/dl/go{v}.linux-$GOARCH.tar.gz \
                 || {{ echo 'Go download failed'; exit 1; }}"
            ),
            "echo 'Extracting Go...'".to_owned(),
            "rm -rf /usr/local/go && tar -C /usr/local -xzf go.tar.gz \
             || { echo 'Go extraction failed'; exit 1; }"
                .to_owned(),
            "rm go.tar.gz".to_owned(),
            "echo 'Go installation completed'".to_owned(),
        ]
    }
}
