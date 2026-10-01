#![allow(clippy::unwrap_used)]

//! Tests for `prompt::confirm_read` (spec 016 R2).
//!
//! The public `confirm` wraps stdin; these tests exercise the
//! testable core via an in-memory cursor so nothing touches the
//! real stdin.

use std::io::Cursor;

use super::prompt::confirm_read;

#[test]
fn test_confirm_y_returns_true() {
    let mut input = Cursor::new(b"y\n");
    assert!(confirm_read(&mut input, "proceed?").expect("read"));
}

#[test]
fn test_confirm_yes_returns_true() {
    let mut input = Cursor::new(b"yes\n");
    assert!(confirm_read(&mut input, "proceed?").expect("read"));
}

#[test]
fn test_confirm_no_returns_false() {
    let mut input = Cursor::new(b"n\n");
    assert!(!confirm_read(&mut input, "proceed?").expect("read"));
}

#[test]
fn test_confirm_empty_line_returns_false() {
    let mut input = Cursor::new(b"\n");
    assert!(!confirm_read(&mut input, "proceed?").expect("read"));
}

#[test]
fn test_confirm_case_insensitive_yes() {
    let mut input = Cursor::new(b"YES\n");
    assert!(confirm_read(&mut input, "proceed?").expect("read"));
}

#[test]
fn test_confirm_random_string_returns_false() {
    let mut input = Cursor::new(b"maybe\n");
    assert!(!confirm_read(&mut input, "proceed?").expect("read"));
}

#[test]
fn test_confirm_trims_surrounding_whitespace() {
    // Defence-in-depth — trailing spaces before newline should
    // still resolve to yes.
    let mut input = Cursor::new(b"  y  \n");
    assert!(confirm_read(&mut input, "proceed?").expect("read"));
}
