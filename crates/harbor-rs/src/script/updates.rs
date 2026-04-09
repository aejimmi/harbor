use super::ScriptComponent;

/// Run system updates, optional kernel upgrade, optional reboot.
pub struct UpdatesComponent {
    pub auto_upgrade: bool,
    pub upgrade_kernel: bool,
    pub reboot_after_kernel: bool,
}

impl ScriptComponent for UpdatesComponent {
    fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();

        if self.auto_upgrade {
            lines.push("echo 'Performing system updates'".to_owned());
            lines.push("apt-get upgrade -y".to_owned());

            if self.upgrade_kernel {
                lines.push("apt-get dist-upgrade -y".to_owned());
            }
        }

        if self.reboot_after_kernel {
            lines.push("echo 'Scheduling reboot in 1 minute'".to_owned());
            lines.push("shutdown -r +1".to_owned());
        }

        lines
    }
}
