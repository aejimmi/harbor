use super::ScriptComponent;

/// Install system packages via apt-get.
pub struct PackagesComponent {
    pub packages: Vec<String>,
}

impl ScriptComponent for PackagesComponent {
    fn render(&self) -> Vec<String> {
        if self.packages.is_empty() {
            return Vec::new();
        }
        vec![
            "echo 'Installing packages'".to_owned(),
            format!("apt-get install -y {}", self.packages.join(" ")),
        ]
    }
}
