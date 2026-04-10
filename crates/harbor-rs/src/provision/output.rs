use super::Spinner;

/// Output handler for provisioning scripts.
///
/// - **Debug mode**: prints every line with `[server] ` prefix.
/// - **Normal mode**: extracts status lines (from `echo '...'` in components)
///   and forwards them to a `Spinner` for display.
pub(crate) struct FilteredOutput<'a> {
    prefix: String,
    debug: bool,
    spinner: Option<&'a Spinner>,
    stdout_buffer: Vec<u8>,
    stderr_buffer: Vec<u8>,
}

impl<'a> FilteredOutput<'a> {
    pub fn new(server_name: &str, spinner: Option<&'a Spinner>, debug: bool) -> Self {
        Self {
            prefix: format!("[{server_name}] "),
            debug,
            spinner,
            stdout_buffer: Vec::new(),
            stderr_buffer: Vec::new(),
        }
    }

    pub fn write_stdout(&mut self, data: &[u8]) {
        self.stdout_buffer.extend_from_slice(data);
        self.drain_lines(false);
    }

    pub fn write_stderr(&mut self, data: &[u8]) {
        self.stderr_buffer.extend_from_slice(data);
        self.drain_lines(true);
    }

    fn drain_lines(&mut self, is_stderr: bool) {
        let buffer = if is_stderr {
            &mut self.stderr_buffer
        } else {
            &mut self.stdout_buffer
        };

        let Some(last_newline) = buffer.iter().rposition(|&b| b == b'\n') else {
            return;
        };

        let remaining = buffer.split_off(last_newline + 1);
        let complete = std::mem::replace(buffer, remaining);

        for line in complete.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }

            let text = String::from_utf8_lossy(line);

            if self.debug {
                eprintln!("{}{text}", self.prefix);
            } else if !is_stderr
                && let Some(status) = extract_status(text.trim())
                && let Some(spinner) = self.spinner
            {
                spinner.set_step(status);
            }
        }
    }

    #[cfg(test)]
    pub fn stdout_pending(&self) -> &[u8] {
        &self.stdout_buffer
    }

    #[cfg(test)]
    pub fn stderr_pending(&self) -> &[u8] {
        &self.stderr_buffer
    }
}

/// Whitelist approach: only extract lines that are our own status messages.
/// These come from `echo '...'` in component render() methods.
fn extract_status(line: &str) -> Option<String> {
    const STATUS_PREFIXES: &[&str] = &[
        "Starting ",
        "Installing ",
        "Configuring ",
        "Creating ",
        "Deploying ",
        "Deploy of ",
        "Deployed ",
        "Applying ",
        "Setting ",
        "Enable ",
        "Service ",
        "Performing ",
        "Setup completed",
        "Updating ",
        "Cloning ",
        "Extracting ",
        "Successfully ",
    ];

    for prefix in STATUS_PREFIXES {
        if line.starts_with(prefix) {
            return Some(line.to_owned());
        }
    }

    if line.starts_with("Rust ") || line.starts_with("Go ") {
        return Some(line.to_owned());
    }

    None
}
