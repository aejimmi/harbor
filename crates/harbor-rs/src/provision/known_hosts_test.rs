use std::net::{IpAddr, Ipv4Addr};

use super::known_hosts;

fn ip(last: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
}

#[test]
fn test_save_and_lookup() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    let key = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITest";
    known_hosts::save_to(&path, ip(1), key).unwrap();

    let found = known_hosts::lookup_in(&path, ip(1)).unwrap();
    assert_eq!(found, Some(key.to_owned()));
}

#[test]
fn test_lookup_returns_none_for_unknown_ip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAAKnown").unwrap();

    assert!(known_hosts::lookup_in(&path, ip(2)).unwrap().is_none());
}

#[test]
fn test_lookup_returns_none_for_missing_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nonexistent");

    assert!(known_hosts::lookup_in(&path, ip(1)).unwrap().is_none());
}

#[test]
fn test_save_replaces_existing_entry() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAAOldKey").unwrap();
    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAANewKey").unwrap();

    let found = known_hosts::lookup_in(&path, ip(1)).unwrap();
    assert_eq!(found, Some("ssh-ed25519 AAAANewKey".to_owned()));

    // Only one entry for this IP.
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        content.lines().filter(|l| !l.is_empty()).count(),
        1,
        "should have exactly one entry after overwrite"
    );
}

#[test]
fn test_save_preserves_other_entries() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAAKey1").unwrap();
    known_hosts::save_to(&path, ip(2), "ssh-ed25519 AAAAKey2").unwrap();

    assert_eq!(
        known_hosts::lookup_in(&path, ip(1)).unwrap(),
        Some("ssh-ed25519 AAAAKey1".to_owned())
    );
    assert_eq!(
        known_hosts::lookup_in(&path, ip(2)).unwrap(),
        Some("ssh-ed25519 AAAAKey2".to_owned())
    );
}

#[test]
fn test_remove_deletes_entry() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAAKey1").unwrap();
    known_hosts::save_to(&path, ip(2), "ssh-ed25519 AAAAKey2").unwrap();

    known_hosts::remove_from(&path, ip(1)).unwrap();

    assert!(known_hosts::lookup_in(&path, ip(1)).unwrap().is_none());
    assert_eq!(
        known_hosts::lookup_in(&path, ip(2)).unwrap(),
        Some("ssh-ed25519 AAAAKey2".to_owned())
    );
}

#[test]
fn test_remove_noop_for_missing_ip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAAKey1").unwrap();

    // Removing a non-existent IP should not affect existing entries.
    known_hosts::remove_from(&path, ip(99)).unwrap();

    assert_eq!(
        known_hosts::lookup_in(&path, ip(1)).unwrap(),
        Some("ssh-ed25519 AAAAKey1".to_owned())
    );
}

#[test]
fn test_remove_noop_for_missing_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nonexistent");

    // Should not panic or error.
    known_hosts::remove_from(&path, ip(1)).unwrap();
}

#[test]
fn test_save_creates_parent_directory() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nested").join("dir").join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAAKey").unwrap();

    assert_eq!(
        known_hosts::lookup_in(&path, ip(1)).unwrap(),
        Some("ssh-ed25519 AAAAKey".to_owned())
    );
}

#[test]
fn test_file_format_is_openssh_compatible() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("known_hosts");

    known_hosts::save_to(&path, ip(1), "ssh-ed25519 AAAA1").unwrap();
    known_hosts::save_to(&path, ip(2), "ssh-rsa AAAA2").unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

    // Each line must be: <ip> <algorithm> <base64>
    assert_eq!(lines.len(), 2);
    assert_eq!(lines.first().copied(), Some("10.0.0.1 ssh-ed25519 AAAA1"));
    assert_eq!(lines.get(1).copied(), Some("10.0.0.2 ssh-rsa AAAA2"));
}
