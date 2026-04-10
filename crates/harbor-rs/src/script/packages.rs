use super::{ScriptComponent, status_echo};

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
            status_echo("Installing packages"),
            format!("apt-get install -y {}", self.packages.join(" ")),
        ]
    }
}
