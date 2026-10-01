use super::*;
use std::path::{Path, PathBuf};

/// Repo-root `examples/` directory.
fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

#[test]
fn test_examples_single_server_configs_load() {
    let dir = examples_dir();
    let entries = std::fs::read_dir(&dir).expect("examples/ directory should exist");
    let mut loaded = 0;
    for entry in entries {
        let path = entry.expect("readable examples/ entry").path();
        if path.extension().is_some_and(|e| e == "yaml") {
            let result = SetupConfig::load(&path);
            assert!(
                result.as_ref().is_ok_and(|c| c.server.is_some()),
                "{} failed to load or has no server: {:?}",
                path.display(),
                result.err()
            );
            loaded += 1;
        }
    }
    assert!(
        loaded >= 2,
        "expected at least two example configs, found {loaded}"
    );
}

#[test]
fn test_examples_fleet_validates() {
    let dir = examples_dir().join("fleet");
    let fleet = FleetConfig::load(&dir.join("fleet.yaml")).expect("fleet.yaml should parse");
    let result = fleet.validate(&dir);
    assert!(result.is_ok(), "fleet example invalid: {:?}", result.err());
}
