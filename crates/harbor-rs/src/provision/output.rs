use super::Spinner;
use crate::script::STATUS_SENTINEL;

/// Output handler for provisioning scripts.
///
/// - **Debug mode**: prints every line with `[server] ` prefix (with the
///   status sentinel stripped so it doesn't leak).
/// - **Normal mode**: extracts sentinel-prefixed status lines emitted by
///   harbor's `status_echo` helper and forwards them to a `Spinner`.
///   Arbitrary apt/dpkg output is silently dropped.
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
            let trimmed = text.trim();
            let status = extract_status(trimmed);

            if self.debug {
                let display = status.as_deref().unwrap_or(trimmed);
                eprintln!("{}{display}", self.prefix);
            } else if !is_stderr
                && let Some(status) = status
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

/// Extract a harbor status message by stripping the sentinel prefix emitted
/// by `script::status_echo`. Returns `None` for any line without the sentinel
/// (apt/dpkg chatter, shell traces, diagnostics) so they don't drive the
/// spinner.
pub(super) fn extract_status(line: &str) -> Option<String> {
    line.strip_prefix(STATUS_SENTINEL)
        .map(|rest| rest.trim().to_owned())
}
