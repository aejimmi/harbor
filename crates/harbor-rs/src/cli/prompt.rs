//! Interactive yes/no confirmation for dangerous commands.
//!
//! Two layers: a public `confirm` that reads from stdin, and a
//! testable `confirm_read<R: BufRead>` that the public helper
//! wraps. Matches the existing output pattern — one public, one
//! testable — and writes all prompts to stderr so they interleave
//! cleanly with `output::*` messages.

use std::io::{BufRead, BufReader, Write};

use anyhow::{Context, Result};

/// Read a yes/no answer from stdin.
///
/// Default on empty input is `false` — restore and similar
/// dangerous commands should require an explicit `y`.
pub fn confirm(question: &str) -> Result<bool> {
    let stdin = std::io::stdin();
    let locked = stdin.lock();
    confirm_read(&mut BufReader::new(locked), question)
}

/// Testable core. The public `confirm` wraps this with the real
/// stdin. Writes the question to stderr so it plays nicely with
/// `eprintln!`-based `output::*`.
pub fn confirm_read<R: BufRead>(reader: &mut R, question: &str) -> Result<bool> {
    eprintln!("{question}");
    eprint!("  [y/N] ");
    std::io::stderr()
        .flush()
        .context("flushing stderr for prompt")?;
    let mut buf = String::new();
    reader
        .read_line(&mut buf)
        .context("reading confirm response")?;
    let answer = buf.trim().to_ascii_lowercase();
    Ok(answer == "y" || answer == "yes")
}
