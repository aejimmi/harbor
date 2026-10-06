use super::setup_validate::validate;
use super::*;

fn check(setup_yaml: &str) -> Result<(), ConfigError> {
    let yaml = format!("name: app\nsetup:\n{setup_yaml}");
    let config: SetupConfig = serde_yaml::from_str(&yaml).expect("test yaml should parse");
    validate(&config)
}

#[test]
fn test_validate_accepts_typical_config() {
    let r = check(
        "  services:\n    - { name: worker@1, enabled: true }\n\
         \x20 environment: { APP_ENV: \"it's fine\" }\n\
         \x20 directories:\n    - { path: /opt/app, owner: app, group: app, mode: \"0755\" }\n",
    );
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn test_validate_rejects_service_name_with_shell_chars() {
    assert!(check("  services:\n    - { name: \"web;reboot\" }\n").is_err());
}

#[test]
fn test_validate_rejects_service_name_starting_with_dash() {
    assert!(check("  services:\n    - { name: \"-x\" }\n").is_err());
}

#[test]
fn test_validate_rejects_bad_project_name() {
    let config: SetupConfig =
        serde_yaml::from_str("name: \"a b\"\nsetup: {}\n").expect("test yaml should parse");
    assert!(validate(&config).is_err());
}

#[test]
fn test_validate_rejects_env_value_with_newline() {
    assert!(check("  services:\n    - { name: web, env: { A: \"x\\nB=y\" } }\n").is_err());
}

#[test]
fn test_validate_rejects_env_key_not_identifier() {
    assert!(check("  environment: { \"1BAD\": x }\n").is_err());
}

#[test]
fn test_validate_rejects_environment_value_with_double_quote() {
    assert!(check("  environment: { A: 'say \"hi\"' }\n").is_err());
}

#[test]
fn test_validate_rejects_relative_file_target() {
    assert!(check("  files:\n    - { source: a, target: etc/a }\n").is_err());
}

#[test]
fn test_validate_rejects_file_target_traversal() {
    assert!(check("  files:\n    - { source: a, target: /etc/../root/a }\n").is_err());
}

#[test]
fn test_validate_rejects_directory_path_with_space() {
    assert!(check("  directories:\n    - { path: \"/opt/my app\" }\n").is_err());
}

#[test]
fn test_validate_rejects_non_octal_mode() {
    assert!(check("  directories:\n    - { path: /opt/a, mode: \"u+x\" }\n").is_err());
}

#[test]
fn test_validate_accepts_ufw_rule_from_ip_and_cidr() {
    for from in ["148.251.183.125", "10.0.0.0/8", "2001:db8::/48", "::1"] {
        let r = check(&format!(
            "  security:\n    ufw:\n      enabled: true\n      rules:\n        - {{ port: 50000, from: \"{from}\" }}\n"
        ));
        assert!(r.is_ok(), "{from}: {r:?}");
    }
}

#[test]
fn test_validate_rejects_bad_ufw_from() {
    for from in ["any", "1.2.3.4/33", "1.2.3.4; reboot", "::/129", "1.2.3"] {
        let r = check(&format!(
            "  security:\n    ufw:\n      enabled: true\n      rules:\n        - {{ port: 50000, from: \"{from}\" }}\n"
        ));
        assert!(r.is_err(), "{from} should be rejected");
    }
}

#[test]
fn test_validate_rejects_bad_ufw_proto() {
    let r = check(
        "  security:\n    ufw:\n      rules:\n        - { port: 53, proto: \"tcp; reboot\" }\n",
    );
    assert!(r.is_err());
}

#[test]
fn test_ufw_rule_rejects_unknown_field() {
    let yaml = "name: app\nsetup:\n  security:\n    ufw:\n      rules:\n        - { port: 5432, form: 10.0.0.1 }\n";
    assert!(serde_yaml::from_str::<SetupConfig>(yaml).is_err());
}
