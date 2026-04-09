use super::output::FilteredOutput;

#[test]
fn test_filtered_output_empty_data_no_crash() {
    let mut out = FilteredOutput::new("test-server", true, true);
    out.write_stdout(b"");
    out.write_stderr(b"");
    assert!(out.stdout_pending().is_empty());
    assert!(out.stderr_pending().is_empty());
}

#[test]
fn test_filtered_output_complete_line_drains_buffer() {
    let mut out = FilteredOutput::new("srv", true, true);
    out.write_stdout(b"$ echo hello\n");
    // After a complete line, buffer should be empty (no trailing data).
    assert!(out.stdout_pending().is_empty());
}

#[test]
fn test_filtered_output_partial_line_buffered() {
    let mut out = FilteredOutput::new("srv", true, true);
    out.write_stdout(b"partial");
    // No newline yet — data stays in buffer.
    assert_eq!(out.stdout_pending(), b"partial");

    // Complete the line.
    out.write_stdout(b" line\n");
    // Now the buffer should be empty.
    assert!(out.stdout_pending().is_empty());
}

#[test]
fn test_filtered_output_multiple_lines_drain_correctly() {
    let mut out = FilteredOutput::new("srv", true, true);
    out.write_stdout(b"line1\nline2\npartial");
    // "line1\n" and "line2\n" should be drained, "partial" remains.
    assert_eq!(out.stdout_pending(), b"partial");
}

#[test]
fn test_filtered_output_stderr_independent_of_stdout() {
    let mut out = FilteredOutput::new("srv", true, true);
    out.write_stdout(b"stdout partial");
    out.write_stderr(b"stderr partial");
    assert_eq!(out.stdout_pending(), b"stdout partial");
    assert_eq!(out.stderr_pending(), b"stderr partial");

    out.write_stdout(b"\n");
    assert!(out.stdout_pending().is_empty());
    // stderr unchanged
    assert_eq!(out.stderr_pending(), b"stderr partial");
}

#[test]
fn test_filtered_output_silent_mode_no_crash() {
    let mut out = FilteredOutput::new("srv", false, false);
    out.write_stdout(b"$ command\nregular output\n# comment\n");
    out.write_stderr(b"error line\n");
    assert!(out.stdout_pending().is_empty());
    assert!(out.stderr_pending().is_empty());
}

#[test]
fn test_filtered_output_newline_only() {
    let mut out = FilteredOutput::new("srv", true, true);
    out.write_stdout(b"\n\n\n");
    assert!(out.stdout_pending().is_empty());
}

#[test]
fn test_filtered_output_large_chunk() {
    let mut out = FilteredOutput::new("srv", false, false);
    let mut data = Vec::new();
    for i in 0..100 {
        data.extend_from_slice(format!("line {i}\n").as_bytes());
    }
    out.write_stdout(&data);
    assert!(out.stdout_pending().is_empty());
}
