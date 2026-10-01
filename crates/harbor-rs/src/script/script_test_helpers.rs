//! Shared fixtures for the split test modules.

#![allow(
    clippy::indexing_slicing,
    clippy::needless_raw_string_hashes,
    clippy::unwrap_used,
    clippy::panic
)]

use super::*;
use crate::config::{BackupSchedule, BackupTransport, ContainerRuntime, ServiceSpec};

/// Build a container `ServiceSpec` with only the fields a test cares
/// about. All other fields take their empty / default values.
pub(super) fn container_svc(name: &str, image: &str, runtime: ContainerRuntime) -> ServiceSpec {
    ServiceSpec {
        name: name.to_owned(),
        enabled: true,
        start: true,
        user: String::new(),
        working_directory: String::new(),
        exec_start: String::new(),
        restart: String::new(),
        restart_sec: 0,
        image: Some(image.to_owned()),
        runtime,
        ports: Vec::new(),
        volumes: Vec::new(),
        env: std::collections::BTreeMap::new(),
        cap_drop: Vec::new(),
        cap_add: Vec::new(),
        read_only: false,
        pids_limit: 256,
    }
}

/// Construct a component that covers every non-default branch so
/// tests can inspect the full rendered output.
pub(super) fn full_backup_component() -> BackupComponent {
    BackupComponent {
        project: "blissd".to_owned(),
        transport: BackupTransport::Rc,
        destination: "s3://bucket/prefix".to_owned(),
        endpoint: "https://account.r2.cloudflarestorage.com".to_owned(),
        schedule: BackupSchedule::Daily,
        retention_days: 7,
        stop_services: vec!["blissd".to_owned(), "worker".to_owned()],
        paths: vec!["/opt/blissd/db".to_owned(), "/opt/blissd/audio".to_owned()],
        access_key_id: "AKID".to_owned(),
        secret_access_key: "S3CRET".to_owned(),
    }
}
