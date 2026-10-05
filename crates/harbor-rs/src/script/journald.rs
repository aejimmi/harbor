//! Cap the systemd journal so logs cannot fill the root disk.

use super::{ScriptComponent, status_echo};

const DROP_IN: &str = "/etc/systemd/journald.conf.d/harbor.conf";

/// Write a journald drop-in with `SystemMaxUse=` and restart journald.
pub struct JournaldComponent {
    /// Size like `1G` (validated at config load).
    pub max_use: String,
}

impl ScriptComponent for JournaldComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo(&format!("Capping journald at {}", self.max_use)),
            "mkdir -p /etc/systemd/journald.conf.d".to_owned(),
            format!(
                "printf '[Journal]\\nSystemMaxUse={}\\n' > {DROP_IN}",
                self.max_use
            ),
            "systemctl restart systemd-journald".to_owned(),
        ]
    }
}
