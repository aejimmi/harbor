use super::output::{FilteredOutput, extract_status};

#[test]
fn test_filtered_output_empty_data_no_crash() {
    let mut out = FilteredOutput::new("test-server", None, true);
    out.write_stdout(b"");
    out.write_stderr(b"");
    assert!(out.stdout_pending().is_empty());
    assert!(out.stderr_pending().is_empty());
}

#[test]
fn test_filtered_output_complete_line_drains_buffer() {
    let mut out = FilteredOutput::new("srv", None, true);
    out.write_stdout(b"$ echo hello\n");
    assert!(out.stdout_pending().is_empty());
}

#[test]
fn test_filtered_output_partial_line_buffered() {
    let mut out = FilteredOutput::new("srv", None, true);
    out.write_stdout(b"partial");
    assert_eq!(out.stdout_pending(), b"partial");

    out.write_stdout(b" line\n");
    assert!(out.stdout_pending().is_empty());
}

#[test]
fn test_filtered_output_multiple_lines_drain_correctly() {
    let mut out = FilteredOutput::new("srv", None, true);
    out.write_stdout(b"line1\nline2\npartial");
    assert_eq!(out.stdout_pending(), b"partial");
}

#[test]
fn test_filtered_output_stderr_independent_of_stdout() {
    let mut out = FilteredOutput::new("srv", None, true);
    out.write_stdout(b"stdout partial");
    out.write_stderr(b"stderr partial");
    assert_eq!(out.stdout_pending(), b"stdout partial");
    assert_eq!(out.stderr_pending(), b"stderr partial");

    out.write_stdout(b"\n");
    assert!(out.stdout_pending().is_empty());
    assert_eq!(out.stderr_pending(), b"stderr partial");
}

#[test]
fn test_filtered_output_silent_mode_no_crash() {
    let mut out = FilteredOutput::new("srv", None, false);
    out.write_stdout(b"$ command\nregular output\n# comment\n");
    out.write_stderr(b"error line\n");
    assert!(out.stdout_pending().is_empty());
    assert!(out.stderr_pending().is_empty());
}

#[test]
fn test_filtered_output_newline_only() {
    let mut out = FilteredOutput::new("srv", None, true);
    out.write_stdout(b"\n\n\n");
    assert!(out.stdout_pending().is_empty());
}

#[test]
fn test_extract_status_accepts_sentinel_line() {
    let got = extract_status("::step:: Installing Docker");
    assert_eq!(got.as_deref(), Some("Installing Docker"));
}

#[test]
fn test_extract_status_trims_inner_whitespace() {
    let got = extract_status("::step::   Configuring UFW  ");
    assert_eq!(got.as_deref(), Some("Configuring UFW"));
}

#[test]
fn test_extract_status_rejects_dpkg_noise() {
    // The exact shape of the leak that motivated the sentinel fix.
    let dpkg = "Installing new version of config file \
                /etc/cloud/templates/sources.list.debian.deb822.tmpl ... (1 of 7)";
    assert_eq!(extract_status(dpkg), None);
}

#[test]
fn test_extract_status_rejects_unrelated_lines() {
    assert_eq!(extract_status(""), None);
    assert_eq!(extract_status("Reading package lists..."), None);
    assert_eq!(
        extract_status("Setting up libc6:amd64 (2.39-0ubuntu8.4)"),
        None
    );
}

#[test]
fn test_filtered_output_large_chunk() {
    let mut out = FilteredOutput::new("srv", None, false);
    let mut data = Vec::new();
    for i in 0..100 {
        data.extend_from_slice(format!("line {i}\n").as_bytes());
    }
    out.write_stdout(&data);
    assert!(out.stdout_pending().is_empty());
}
