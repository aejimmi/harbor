use super::{ScriptComponent, status_echo};

/// Create a swap file.
pub struct SwapComponent {
    pub size: String,
}

impl ScriptComponent for SwapComponent {
    fn render(&self) -> Vec<String> {
        let size = &self.size;
        // Same anti-pattern avoidance as rc_install: each step is
        // its own bash statement so `set -e` fires on any failure.
        // Idempotent — skip entirely when /swapfile is already
        // mounted as swap.
        vec![
            status_echo(&format!("Creating {size} swap file")),
            "if swapon --show=NAME --noheadings | grep -qx /swapfile; then".to_owned(),
            "    echo 'swapfile already active, skipping'".to_owned(),
            "else".to_owned(),
            format!("    fallocate -l {size} /swapfile"),
            "    chmod 600 /swapfile".to_owned(),
            "    mkswap /swapfile".to_owned(),
            "    swapon /swapfile".to_owned(),
            "    grep -qx '/swapfile none swap sw 0 0' /etc/fstab \
                  || echo '/swapfile none swap sw 0 0' >> /etc/fstab"
                .to_owned(),
            "fi".to_owned(),
        ]
    }
}
