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
