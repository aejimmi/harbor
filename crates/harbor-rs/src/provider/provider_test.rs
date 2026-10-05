use super::*;

#[test]
fn test_server_status_equality() {
    assert_eq!(ServerStatus::Running, ServerStatus::Running);
    assert_ne!(ServerStatus::Running, ServerStatus::Off);
}

#[test]
fn test_server_debug_format() {
    let server = Server {
        id: 123,
        name: "test-server".to_owned(),
        status: ServerStatus::Running,
        ip: Some("1.2.3.4".parse().expect("valid ip")),
        server_type: "cpx31".to_owned(),
        location: "nbg1".to_owned(),
    };
    let debug = format!("{server:?}");
    assert!(debug.contains("test-server"));
    assert!(debug.contains("Running"));
}

fn parse_action(body: &str) -> hetzner_volume::ActionState {
    serde_json::from_str::<hetzner_volume::ActionEnvelope>(body)
        .expect("parses")
        .action
}

#[test]
fn test_parse_action_success_without_error_key() {
    // Hetzner omits `error` when it is null.
    let action = parse_action(
        r#"{"action":{"command":"attach_volume","finished":"2026-10-05T10:50:47Z",
        "id":1,"progress":100,"resources":[],"started":"2026-10-05T10:50:32Z","status":"success"}}"#,
    );
    assert_eq!(action.id, 1);
    assert_eq!(action.status, "success");
    assert!(action.error.is_none());
}

#[test]
fn test_parse_action_running_with_null_error() {
    let action = parse_action(
        r#"{"action":{"command":"create_volume","error":null,"finished":null,
        "id":2,"progress":0,"resources":[],"started":"2026-10-05T10:50:32Z","status":"running"}}"#,
    );
    assert_eq!(action.status, "running");
    assert!(action.error.is_none());
}

#[test]
fn test_parse_action_error_carries_message() {
    let action = parse_action(
        r#"{"action":{"command":"attach_volume","error":{"code":"action_failed",
        "message":"volume busy"},"id":3,"progress":100,"resources":[],"status":"error"}}"#,
    );
    assert_eq!(action.status, "error");
    assert_eq!(action.error.expect("error set").message, "volume busy");
}

#[test]
fn test_parse_action_rejects_missing_action() {
    assert!(serde_json::from_str::<hetzner_volume::ActionEnvelope>("{}").is_err());
}

#[test]
fn test_parse_created_volume_without_error_keys() {
    let body = r#"{"volume":{"id":107040632,"name":"data","linux_device":"/dev/disk/by-id/scsi-0HC_Volume_107040632",
        "size":10,"server":5},"action":{"command":"create_volume","id":10,"status":"running"},
        "next_actions":[{"command":"attach_volume","id":11,"status":"running"}]}"#;
    let created: hetzner_volume::CreatedVolume = serde_json::from_str(body).expect("parses");
    assert_eq!(created.volume.id, 107_040_632);
    assert!(created.volume.linux_device.ends_with("_107040632"));
    assert_eq!(created.action.id, 10);
    let ids: Vec<i64> = created.next_actions.iter().map(|a| a.id).collect();
    assert_eq!(ids, [11]);
}

#[test]
fn test_parse_created_volume_without_next_actions() {
    let body = r#"{"volume":{"id":1,"linux_device":"/dev/x"},"action":{"command":"create_volume","id":2,"status":"running"}}"#;
    let created: hetzner_volume::CreatedVolume = serde_json::from_str(body).expect("parses");
    assert!(created.next_actions.is_empty());
}

#[test]
fn test_reuse_conflict_other_location() {
    let why = hetzner_volume::reuse_conflict("data", "nbg1", None, "fsn1", None).expect("conflict");
    assert!(why.contains("nbg1") && why.contains("fsn1"));
}

#[test]
fn test_reuse_conflict_detached_same_location() {
    assert!(hetzner_volume::reuse_conflict("data", "nbg1", None, "nbg1", None).is_none());
    assert!(hetzner_volume::reuse_conflict("data", "nbg1", None, "nbg1", Some(7)).is_none());
}

#[test]
fn test_reuse_conflict_attached_to_this_server() {
    assert!(hetzner_volume::reuse_conflict("data", "nbg1", Some(7), "nbg1", Some(7)).is_none());
}

#[test]
fn test_reuse_conflict_attached_elsewhere() {
    let why =
        hetzner_volume::reuse_conflict("data", "nbg1", Some(9), "nbg1", Some(7)).expect("conflict");
    assert!(why.contains("another server"));
}

#[test]
fn test_reuse_conflict_attached_before_server_exists() {
    // No server yet, so any attachment belongs to someone else.
    assert!(hetzner_volume::reuse_conflict("data", "nbg1", Some(9), "nbg1", None).is_some());
}
