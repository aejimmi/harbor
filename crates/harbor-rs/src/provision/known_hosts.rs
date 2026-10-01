//! Host key storage for SSH server identity verification.
//!
//! Keys are stored in `~/.harbor/known_hosts` in OpenSSH format:
//! `<ip> <algorithm> <base64-key>`, one entry per line. Compatible with
//! the `ssh` binary via `-o UserKnownHostsFile=~/.harbor/known_hosts`.
//!
//! Writes are atomic (write to temp file, then rename) and serialized by
//! a process-wide lock, so concurrent fleet provisions don't lose each
//! other's entries in the read-modify-write.
//!
//! Read errors other than "file not found" are surfaced rather than
//! treated as "no key pinned" — silently accepting any key would defeat
//! the pinning.

use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};

/// Serializes read-modify-write cycles on the known_hosts file.
static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Path to harbor's known_hosts file.
fn default_path() -> Result<PathBuf> {
    Ok(crate::config::harbor_dir()?.join("known_hosts"))
}

/// Save a host key for an IP address.
///
/// Replaces any existing entry for the same IP, then appends the new
/// key. Creates the parent directory if it does not exist. The write
/// is atomic (temp file + rename).
pub fn save(ip: IpAddr, openssh_key: &str) -> Result<()> {
    save_to(&default_path()?, ip, openssh_key)
}

/// Load the pinned host key for an IP address.
///
/// `Ok(None)` when no entry exists (or the file doesn't exist yet).
pub fn lookup(ip: IpAddr) -> Result<Option<String>> {
    lookup_in(&default_path()?, ip)
}

/// Remove the host key entry for an IP address.
///
/// No-op if the file does not exist or the IP is not found.
pub fn remove(ip: IpAddr) -> Result<()> {
    remove_from(&default_path()?, ip)
}

// --- Internal functions (testable with arbitrary paths) ---

/// Save a host key to a specific file path, atomically.
pub(crate) fn save_to(file_path: &Path, ip: IpAddr, openssh_key: &str) -> Result<()> {
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).context("creating harbor config directory")?;
    }
    let _guard = FILE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut entries = load_all(file_path)?;
    let prefix = format!("{ip} ");
    entries.retain(|line| !line.starts_with(&prefix));
    entries.push(format!("{prefix}{openssh_key}"));
    write_all(file_path, &entries)
}

/// Look up a host key in a specific file.
pub(crate) fn lookup_in(file_path: &Path, ip: IpAddr) -> Result<Option<String>> {
    let _guard = FILE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let prefix = format!("{ip} ");
    Ok(load_all(file_path)?
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix).map(ToOwned::to_owned)))
}

/// Remove a host key entry from a specific file, atomically.
pub(crate) fn remove_from(file_path: &Path, ip: IpAddr) -> Result<()> {
    let _guard = FILE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut entries = load_all(file_path)?;
    let prefix = format!("{ip} ");
    let before = entries.len();
    entries.retain(|line| !line.starts_with(&prefix));
    if entries.len() == before {
        return Ok(());
    }
    write_all(file_path, &entries)
}

/// Read all non-empty lines. A missing file is an empty list; any other
/// read error is returned.
fn load_all(file_path: &Path) -> Result<Vec<String>> {
    let data = match std::fs::read_to_string(file_path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(e).with_context(|| format!("reading {}", file_path.display()));
        }
    };
    Ok(data
        .lines()
        .filter(|l| !l.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

/// Atomically replace the file with `entries`. Caller holds `FILE_LOCK`.
fn write_all(file_path: &Path, entries: &[String]) -> Result<()> {
    let tmp_path = file_path.with_extension("tmp");
    {
        let mut file =
            std::fs::File::create(&tmp_path).context("creating known_hosts temp file")?;
        for entry in entries {
            writeln!(file, "{entry}").context("writing known_hosts entry")?;
        }
        file.sync_all().context("syncing known_hosts temp file")?;
    }
    std::fs::rename(&tmp_path, file_path).context("renaming known_hosts temp file")?;
    Ok(())
}
