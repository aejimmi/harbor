#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::panic)]

use super::*;
use std::path::Path;

#[test]
fn test_parse_short_form() {
    let yaml = "roles:\n  web: 3\n  db: 1\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.roles.len(), 2);
    assert_eq!(config.roles["web"].count(), 3);
    assert_eq!(config.roles["db"].count(), 1);
}

#[test]
fn test_parse_long_form() {
    let yaml = "roles:\n  api:\n    count: 2\n    path: ./platform\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.roles["api"].count(), 2);

    let dir = config.roles["api"].role_dir("api", Path::new("/project"));
    assert_eq!(dir, Path::new("/project/./platform"));
}

#[test]
fn test_parse_mixed_forms() {
    let yaml =
        "roles:\n  clickhouse: 1\n  collectors: 3\n  api:\n    count: 2\n    path: ./platform\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.roles.len(), 3);
    assert_eq!(config.roles["clickhouse"].count(), 1);
    assert_eq!(config.roles["collectors"].count(), 3);
    assert_eq!(config.roles["api"].count(), 2);
}

#[test]
fn test_role_dir_short_form() {
    let spec = RoleSpec::Short(3);
    assert_eq!(
        spec.role_dir("web", Path::new("/project")),
        Path::new("/project/web")
    );
}

#[test]
fn test_role_dir_long_form_no_path() {
    let spec = RoleSpec::Long {
        count: 2,
        path: None,
    };
    assert_eq!(
        spec.role_dir("api", Path::new("/project")),
        Path::new("/project/api")
    );
}

#[test]
fn test_role_dir_long_form_with_path() {
    let spec = RoleSpec::Long {
        count: 2,
        path: Some("./platform".into()),
    };
    assert_eq!(
        spec.role_dir("api", Path::new("/project")),
        Path::new("/project/./platform")
    );
}

#[test]
fn test_expand_servers_sorted_alphabetically() {
    let yaml = "roles:\n  platform: 2\n  clickhouse: 1\n  collectors: 3\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    let servers = expand_servers(&config, "staging", Path::new("/project"));

    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "clickhouse-staging-1",
            "collectors-staging-1",
            "collectors-staging-2",
            "collectors-staging-3",
            "platform-staging-1",
            "platform-staging-2",
        ]
    );
}

#[test]
fn test_expand_servers_roles() {
    let yaml = "roles:\n  web: 2\n  db: 1\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    let servers = expand_servers(&config, "prod", Path::new("/project"));

    assert_eq!(servers[0].role, "db");
    assert_eq!(servers[1].role, "web");
    assert_eq!(servers[2].role, "web");
}

#[test]
fn test_expand_servers_role_dirs() {
    let yaml = "roles:\n  web: 1\n  api:\n    count: 1\n    path: ./platform\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    let servers = expand_servers(&config, "staging", Path::new("/project"));

    // api comes before web alphabetically
    assert_eq!(servers[0].role_dir, Path::new("/project/./platform"));
    assert_eq!(servers[1].role_dir, Path::new("/project/web"));
}

#[test]
fn test_validate_rejects_path_traversal() {
    let yaml = "roles:\n  evil:\n    count: 1\n    path: ../../etc\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    let err = config.validate(Path::new("/project")).unwrap_err();
    assert!(err.to_string().contains(".."));
}

#[test]
fn test_validate_rejects_zero_count() {
    let yaml = "roles:\n  web: 0\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    let err = config.validate(Path::new("/project")).unwrap_err();
    assert!(err.to_string().contains("count >= 1"));
}

#[test]
fn test_validate_rejects_missing_directory() {
    let yaml = "roles:\n  nonexistent: 1\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    let err = config.validate(Path::new("/project")).unwrap_err();
    assert!(err.to_string().contains("directory not found"));
}

#[test]
fn test_validate_passes_with_real_dirs() {
    let dir = tempfile::TempDir::new().unwrap();
    let role_dir = dir.path().join("web");
    std::fs::create_dir(&role_dir).unwrap();
    std::fs::write(
        role_dir.join("harbor.yaml"),
        "name: test\nserver:\n  name: test-01\n  ssh_key: mykey\nsetup: {}\n",
    )
    .unwrap();

    let yaml = "roles:\n  web: 2\n";
    let config: FleetConfig = serde_yaml::from_str(yaml).unwrap();
    config.validate(dir.path()).unwrap();
}
