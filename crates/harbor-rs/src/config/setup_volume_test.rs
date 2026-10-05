use super::setup_validate::validate;
use super::*;

/// Parse a config whose `server:` block carries `volumes_yaml`.
fn with_volumes(volumes_yaml: &str) -> Result<SetupConfig, String> {
    let yaml = format!(
        "name: tell\nserver:\n  name: tell-de-01\n  ssh_key: k\n  volumes:\n{volumes_yaml}setup: {{}}\n"
    );
    serde_yaml::from_str(&yaml).map_err(|e| e.to_string())
}

fn check_volumes(volumes_yaml: &str) -> Result<(), String> {
    let config = with_volumes(volumes_yaml)?;
    validate(&config).map_err(|e| e.to_string())
}

fn check_journald(value: &str) -> Result<(), ConfigError> {
    let yaml = format!("name: app\nsetup:\n  system:\n    journald_max_use: \"{value}\"\n");
    let config: SetupConfig = serde_yaml::from_str(&yaml).expect("test yaml should parse");
    validate(&config)
}

#[test]
fn test_volume_parses_with_default_format() {
    let config = with_volumes("    - { name: tell-data, size: 50, mount: /opt/tell }\n")
        .expect("volume yaml should parse");
    assert!(validate(&config).is_ok());
    let server = config.server.expect("server block present");
    let vol = server.volumes.first().expect("one volume");
    assert_eq!(vol.name, "tell-data");
    assert_eq!(vol.size, 50);
    assert_eq!(vol.mount, "/opt/tell");
    assert_eq!(vol.format, VolumeFormat::Ext4);
}

#[test]
fn test_volume_parses_xfs_format() {
    let config = with_volumes("    - { name: d, size: 10, mount: /data, format: xfs }\n")
        .expect("volume yaml should parse");
    let server = config.server.expect("server block present");
    let vol = server.volumes.first().expect("one volume");
    assert_eq!(vol.format.as_str(), "xfs");
}

#[test]
fn test_server_without_volumes_defaults_empty() {
    let config: SetupConfig =
        serde_yaml::from_str("name: a\nserver: { name: s, ssh_key: k }\nsetup: {}\n")
            .expect("test yaml should parse");
    assert!(config.server.expect("server").volumes.is_empty());
}

#[test]
fn test_volume_rejects_unknown_field() {
    let r = with_volumes("    - { name: d, size: 10, mount: /data, automount: true }\n");
    assert!(r.is_err());
}

#[test]
fn test_volume_rejects_unknown_format() {
    let r = with_volumes("    - { name: d, size: 10, mount: /data, format: btrfs }\n");
    assert!(r.is_err());
}

#[test]
fn test_volume_rejects_size_below_hetzner_minimum() {
    let r = check_volumes("    - { name: d, size: 9, mount: /data }\n");
    assert!(r.is_err_and(|e| e.contains("outside")));
}

#[test]
fn test_volume_rejects_size_above_hetzner_maximum() {
    assert!(check_volumes("    - { name: d, size: 10241, mount: /data }\n").is_err());
}

#[test]
fn test_volume_rejects_bad_names() {
    for name in ["-d", "d-", "d;rm", "a b", ""] {
        let r = check_volumes(&format!(
            "    - {{ name: \"{name}\", size: 10, mount: /data }}\n"
        ));
        assert!(r.is_err(), "name {name:?} should be rejected");
    }
}

#[test]
fn test_volume_rejects_name_over_64_chars() {
    let name = "a".repeat(65);
    let r = check_volumes(&format!(
        "    - {{ name: {name}, size: 10, mount: /data }}\n"
    ));
    assert!(r.is_err());
}

#[test]
fn test_volume_rejects_unsafe_mounts() {
    for mount in [
        "data",
        "/opt/../etc",
        "/opt/tell/",
        "/opt/te ll",
        "/opt/$(id)",
    ] {
        let r = check_volumes(&format!(
            "    - {{ name: d, size: 10, mount: \"{mount}\" }}\n"
        ));
        assert!(r.is_err(), "mount {mount:?} should be rejected");
    }
}

#[test]
fn test_volume_rejects_system_directory_mounts() {
    for mount in ["/", "/etc", "/var", "/usr"] {
        let r = check_volumes(&format!(
            "    - {{ name: d, size: 10, mount: \"{mount}\" }}\n"
        ));
        assert!(r.is_err_and(|e| e.contains("system directory") || e.contains("absolute")));
    }
}

#[test]
fn test_volume_accepts_subdirectory_of_var() {
    assert!(check_volumes("    - { name: d, size: 10, mount: /var/lib/tell }\n").is_ok());
}

#[test]
fn test_volume_rejects_duplicate_names() {
    let r = check_volumes(
        "    - { name: d, size: 10, mount: /a }\n    - { name: d, size: 10, mount: /b }\n",
    );
    assert!(r.is_err_and(|e| e.contains("twice")));
}

#[test]
fn test_volume_rejects_shared_mount() {
    let r = check_volumes(
        "    - { name: a, size: 10, mount: /data }\n    - { name: b, size: 10, mount: /data }\n",
    );
    assert!(r.is_err_and(|e| e.contains("share mount")));
}

#[test]
fn test_journald_max_use_accepts_sizes() {
    for value in ["", "1G", "500M", "2048K", "1T", "100"] {
        assert!(
            check_journald(value).is_ok(),
            "{value:?} should be accepted"
        );
    }
}

#[test]
fn test_journald_max_use_rejects_bad_values() {
    for value in ["G", "0G", "1GB", "1.5G", "-1G", "1G; reboot", "1g"] {
        assert!(
            check_journald(value).is_err(),
            "{value:?} should be rejected"
        );
    }
}
