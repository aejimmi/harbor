use super::ScriptComponent;

/// Set the system hostname.
pub struct HostnameComponent {
    pub hostname: String,
}

impl ScriptComponent for HostnameComponent {
    fn render(&self) -> Vec<String> {
        let h = &self.hostname;
        vec![
            format!("echo 'Setting hostname to {h}'"),
            format!("hostnamectl set-hostname {h}"),
            format!("echo '127.0.1.1 {h}' >> /etc/hosts"),
        ]
    }
}

/// Set the system timezone.
pub struct TimezoneComponent {
    pub timezone: String,
}

impl ScriptComponent for TimezoneComponent {
    fn render(&self) -> Vec<String> {
        let tz = &self.timezone;
        vec![
            format!("echo 'Setting timezone to {tz}'"),
            format!("timedatectl set-timezone {tz}"),
        ]
    }
}
