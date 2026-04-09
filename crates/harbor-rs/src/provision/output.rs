/// Filtered output writer that prefixes lines with server name.
///
/// Matches Go's `filteredWriter` behavior:
/// - Lines starting with `$` or `#` are "command lines" — shown if `show_commands`
/// - All other lines shown only if `verbose`
/// - Each line prefixed with `[server_name] `
pub(crate) struct FilteredOutput {
    prefix: String,
    show_commands: bool,
    verbose: bool,
    stdout_buffer: Vec<u8>,
    stderr_buffer: Vec<u8>,
}

impl FilteredOutput {
    pub fn new(server_name: &str, show_commands: bool, verbose: bool) -> Self {
        Self {
            prefix: format!("[{server_name}] "),
            show_commands,
            verbose,
            stdout_buffer: Vec::new(),
            stderr_buffer: Vec::new(),
        }
    }

    pub fn write_stdout(&mut self, data: &[u8]) {
        self.stdout_buffer.extend_from_slice(data);
        drain_complete_lines(
            &mut self.stdout_buffer,
            &self.prefix,
            self.show_commands,
            self.verbose,
        );
    }

    pub fn write_stderr(&mut self, data: &[u8]) {
        self.stderr_buffer.extend_from_slice(data);
        drain_complete_lines(
            &mut self.stderr_buffer,
            &self.prefix,
            self.show_commands,
            self.verbose,
        );
    }

    /// Returns any remaining buffered bytes (incomplete trailing lines).
    #[cfg(test)]
    pub fn stdout_pending(&self) -> &[u8] {
        &self.stdout_buffer
    }

    /// Returns any remaining buffered bytes (incomplete trailing lines).
    #[cfg(test)]
    pub fn stderr_pending(&self) -> &[u8] {
        &self.stderr_buffer
    }
}

/// Drain all complete lines from `buffer`, print them, and leave any
/// incomplete trailing data in the buffer.
fn drain_complete_lines(buffer: &mut Vec<u8>, prefix: &str, show_commands: bool, verbose: bool) {
    // Find the last newline — everything before it (inclusive) is complete lines.
    let Some(last_newline) = buffer.iter().rposition(|&b| b == b'\n') else {
        return; // No complete lines yet.
    };

    // Split off the complete lines.
    let remaining = buffer.split_off(last_newline + 1);
    let complete = std::mem::replace(buffer, remaining);

    for line in complete.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }

        let is_command = line.first() == Some(&b'$') || line.first() == Some(&b'#');
        let should_print = if is_command { show_commands } else { verbose };

        if should_print {
            let text = String::from_utf8_lossy(line);
            eprintln!("{prefix}{text}");
        }
    }
}
