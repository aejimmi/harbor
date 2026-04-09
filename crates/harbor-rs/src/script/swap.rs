use super::ScriptComponent;

/// Create a swap file.
pub struct SwapComponent {
    pub size: String,
}

impl ScriptComponent for SwapComponent {
    fn render(&self) -> Vec<String> {
        let size = &self.size;
        vec![
            format!("echo 'Creating {size} swap file'"),
            format!("fallocate -l {size} /swapfile"),
            "chmod 600 /swapfile".to_owned(),
            "mkswap /swapfile".to_owned(),
            "swapon /swapfile".to_owned(),
            "echo '/swapfile none swap sw 0 0' >> /etc/fstab".to_owned(),
        ]
    }
}
