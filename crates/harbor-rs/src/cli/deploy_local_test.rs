#![allow(clippy::indexing_slicing)]

use super::deploy_local::*;
use crate::config::setup::DeployConfig;

fn elf(machine: u16) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(b"\x7fELF");
    h[18..20].copy_from_slice(&machine.to_le_bytes());
    h
}

fn entry() -> DeployConfig {
    DeployConfig {
        repo: "github.com/tell-rs/tell".to_owned(),
        binary: ".release/tell".to_owned(),
        install: "/usr/local/bin/tell".to_owned(),
        steps: vec![],
        services: vec!["tell".to_owned()],
    }
}

#[test]
fn test_expected_machine_maps_server_types() {
    assert_eq!(expected_machine("cx33"), 62);
    assert_eq!(expected_machine("ccx33"), 62);
    assert_eq!(expected_machine("cax21"), 183);
}

#[test]
fn test_check_elf_accepts_matching_machine() {
    assert!(check_elf(&elf(62), 62).is_ok());
    assert!(check_elf(&elf(183), 183).is_ok());
}

#[test]
fn test_check_elf_rejects_wrong_arch() {
    let err = check_elf(&elf(183), 62).expect_err("arm binary on x86 server");
    assert!(err.to_string().contains("183"));
}

#[test]
fn test_check_elf_rejects_macho_and_short_input() {
    let macho = [0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0];
    assert!(
        check_elf(&macho, 62)
            .unwrap_err()
            .to_string()
            .contains("not a Linux ELF")
    );
    assert!(check_elf(b"\x7fE", 62).is_err());
    assert!(check_elf(&elf(62)[..10], 62).is_err());
}

#[test]
fn test_local_script_installs_under_content_label_and_restarts() {
    let script = build_local_script("tell", &entry(), "/tmp/harbor-upload-tell");
    assert!(
        script.contains("mkdir ~/.harbor/deploy.lock"),
        "takes the deploy lock"
    );
    assert!(script.contains(r#"SHA="local-$(sha256sum "/tmp/harbor-upload-tell" | cut -c1-12)""#));
    assert!(script.contains(r#"install -m 755 "/tmp/harbor-upload-tell" "$VERSION_DIR/tell""#));
    assert!(script.contains(r#"mv -T "/usr/local/bin/tell.new" "/usr/local/bin/tell""#));
    assert!(script.contains("systemctl restart tell"));
    assert!(script.contains("deploys.log"), "rollback can find it");
    assert!(!script.contains("git "), "no git on the server");
}

#[test]
fn test_local_script_removes_upload_after_install() {
    let script = build_local_script("tell", &entry(), "/tmp/harbor-upload-tell");
    let install = script.find("install -m 755").expect("install line");
    let rm = script
        .find(r#"rm -f "/tmp/harbor-upload-tell""#)
        .expect("cleanup line");
    assert!(install < rm);
}
