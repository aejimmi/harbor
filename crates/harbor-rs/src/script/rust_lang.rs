use super::{STATUS_SENTINEL, ScriptComponent, status_echo};

/// Install Rust via rustup.
pub struct RustComponent;

impl ScriptComponent for RustComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Installing Rust toolchain"),
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_owned(),
            "source $HOME/.cargo/env".to_owned(),
            format!("echo \"{STATUS_SENTINEL} Rust $(rustc --version) installed\""),
        ]
    }
}
