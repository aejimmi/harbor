use super::ScriptComponent;

/// Install Rust via rustup.
pub struct RustComponent;

impl ScriptComponent for RustComponent {
    fn render(&self) -> Vec<String> {
        vec![
            "echo 'Installing Rust toolchain'".to_owned(),
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_owned(),
            "source $HOME/.cargo/env".to_owned(),
            "echo \"Rust $(rustc --version) installed\"".to_owned(),
        ]
    }
}
