#![allow(clippy::panic, clippy::indexing_slicing, clippy::unwrap_used)]

use super::*;
use clap::Parser;

#[test]
fn test_cli_init_parses() {
    let cli = Cli::try_parse_from(["harbor", "init"]).expect("parse init");
    assert!(matches!(cli.command, Some(Commands::Init)));
}

#[test]
fn test_cli_version_parses() {
    let cli = Cli::try_parse_from(["harbor", "version"]).expect("parse version");
    assert!(matches!(cli.command, Some(Commands::Version)));
}

#[test]
fn test_cli_up_parses() {
    let cli = Cli::try_parse_from(["harbor", "up"]).expect("parse up");
    assert!(matches!(cli.command, Some(Commands::Up { debug: false })));
}

#[test]
fn test_cli_up_debug_parses() {
    let cli = Cli::try_parse_from(["harbor", "up", "--debug"]).expect("parse up --debug");
    assert!(matches!(cli.command, Some(Commands::Up { debug: true })));
}

#[test]
fn test_cli_down_parses() {
    let cli = Cli::try_parse_from(["harbor", "down"]).expect("parse down");
    assert!(matches!(cli.command, Some(Commands::Down)));
}

#[test]
fn test_cli_deploy_with_name_parses() {
    let cli = Cli::try_parse_from(["harbor", "deploy", "web"]).expect("parse deploy web");
    match cli.command {
        Some(Commands::Deploy { name, all, debug }) => {
            assert_eq!(name.as_deref(), Some("web"));
            assert!(!all);
            assert!(!debug);
        }
        other => panic!("expected Deploy, got {other:?}"),
    }
}

#[test]
fn test_cli_deploy_all_flag_parses() {
    let cli = Cli::try_parse_from(["harbor", "deploy", "--all"]).expect("parse deploy --all");
    match cli.command {
        Some(Commands::Deploy { name, all, .. }) => {
            assert!(name.is_none());
            assert!(all);
        }
        other => panic!("expected Deploy, got {other:?}"),
    }
}

#[test]
fn test_cli_deploy_debug_with_name_parses() {
    let cli = Cli::try_parse_from(["harbor", "deploy", "api", "--debug"])
        .expect("parse deploy api --debug");
    match cli.command {
        Some(Commands::Deploy { name, debug, .. }) => {
            assert_eq!(name.as_deref(), Some("api"));
            assert!(debug);
        }
        other => panic!("expected Deploy, got {other:?}"),
    }
}

#[test]
fn test_cli_deploy_without_name_or_all_errors() {
    // Neither a name nor --all is supplied — clap ArgGroup enforces one.
    let result = Cli::try_parse_from(["harbor", "deploy"]);
    assert!(
        result.is_err(),
        "deploy with no name and no --all must error"
    );
}

#[test]
fn test_cli_deploy_name_and_all_conflict_errors() {
    let result = Cli::try_parse_from(["harbor", "deploy", "web", "--all"]);
    assert!(
        result.is_err(),
        "deploy with both <name> and --all must error"
    );
}

#[test]
fn test_cli_status_parses() {
    let cli = Cli::try_parse_from(["harbor", "status"]).expect("parse status");
    assert!(matches!(cli.command, Some(Commands::Status)));
}

#[test]
fn test_cli_ssh_parses() {
    let cli = Cli::try_parse_from(["harbor", "ssh"]).expect("parse ssh");
    assert!(matches!(cli.command, Some(Commands::Ssh)));
}

#[test]
fn test_cli_logs_parses() {
    let cli = Cli::try_parse_from(["harbor", "logs"]).expect("parse logs");
    assert!(matches!(
        cli.command,
        Some(Commands::Logs { service: None })
    ));
}

#[test]
fn test_cli_logs_with_service_parses() {
    let cli = Cli::try_parse_from(["harbor", "logs", "blissd"]).expect("parse logs blissd");
    match cli.command {
        Some(Commands::Logs { service }) => assert_eq!(service.as_deref(), Some("blissd")),
        other => panic!("expected Logs, got {other:?}"),
    }
}

#[test]
fn test_cli_server_list_parses() {
    let cli = Cli::try_parse_from(["harbor", "server", "list"]).expect("parse server list");
    assert!(matches!(
        cli.command,
        Some(Commands::Server {
            action: ServerAction::List
        })
    ));
}

#[test]
fn test_cli_server_create_parses() {
    let cli = Cli::try_parse_from([
        "harbor",
        "server",
        "create",
        "myserver",
        "--ssh-key",
        "mykey",
    ])
    .expect("parse server create");

    match cli.command {
        Some(Commands::Server {
            action:
                ServerAction::Create(CreateArgs {
                    name,
                    ssh_key,
                    r#type,
                    location,
                    ..
                }),
        }) => {
            assert_eq!(name, "myserver");
            assert_eq!(ssh_key, "mykey");
            assert_eq!(r#type, "cax11");
            assert_eq!(location, "nbg1");
        }
        other => panic!("expected Server Create, got {other:?}"),
    }
}

#[test]
fn test_cli_server_create_with_all_flags() {
    let cli = Cli::try_parse_from([
        "harbor",
        "server",
        "create",
        "myserver",
        "--ssh-key",
        "mykey",
        "--type",
        "cpx31",
        "--location",
        "fsn1",
        "--image",
        "debian-12",
        "--hostname",
        "myapp",
        "--debug",
        "--quiet",
    ])
    .expect("parse server create with all flags");

    match cli.command {
        Some(Commands::Server {
            action:
                ServerAction::Create(CreateArgs {
                    name,
                    ssh_key,
                    r#type,
                    location,
                    image,
                    hostname,
                    debug,
                    quiet,
                    ..
                }),
        }) => {
            assert_eq!(name, "myserver");
            assert_eq!(ssh_key, "mykey");
            assert_eq!(r#type, "cpx31");
            assert_eq!(location, "fsn1");
            assert_eq!(image, "debian-12");
            assert_eq!(hostname.as_deref(), Some("myapp"));
            assert!(debug);
            assert!(quiet);
        }
        other => panic!("expected Server Create, got {other:?}"),
    }
}

#[test]
fn test_cli_server_create_missing_ssh_key_errors() {
    let result = Cli::try_parse_from(["harbor", "server", "create", "myserver"]);
    assert!(result.is_err());
}

#[test]
fn test_cli_server_delete_parses() {
    let cli = Cli::try_parse_from([
        "harbor",
        "server",
        "delete",
        "myserver",
        "--hostname",
        "myapp",
    ])
    .expect("parse server delete");

    match cli.command {
        Some(Commands::Server {
            action: ServerAction::Delete { name, hostname, .. },
        }) => {
            assert_eq!(name, "myserver");
            assert_eq!(hostname.as_deref(), Some("myapp"));
        }
        other => panic!("expected Server Delete, got {other:?}"),
    }
}

#[test]
fn test_cli_fleet_up_parses() {
    let cli = Cli::try_parse_from(["harbor", "fleet", "up", "staging", "--sequential"])
        .expect("parse fleet up");

    match cli.command {
        Some(Commands::Fleet {
            action: FleetAction::Up {
                name, sequential, ..
            },
        }) => {
            assert_eq!(name, "staging");
            assert!(sequential);
        }
        other => panic!("expected Fleet Up, got {other:?}"),
    }
}

#[test]
fn test_cli_fleet_down_parses() {
    let cli =
        Cli::try_parse_from(["harbor", "fleet", "down", "staging"]).expect("parse fleet down");

    assert!(matches!(
        cli.command,
        Some(Commands::Fleet {
            action: FleetAction::Down { .. }
        })
    ));
}

#[test]
fn test_cli_fleet_status_parses() {
    let cli =
        Cli::try_parse_from(["harbor", "fleet", "status", "staging"]).expect("parse fleet status");
    assert!(matches!(
        cli.command,
        Some(Commands::Fleet {
            action: FleetAction::Status { .. }
        })
    ));
}

#[test]
fn test_cli_env_alias_parses() {
    let cli = Cli::try_parse_from(["harbor", "env", "status", "staging"]).expect("parse env alias");
    assert!(matches!(
        cli.command,
        Some(Commands::Fleet {
            action: FleetAction::Status { .. }
        })
    ));
}

#[test]
fn test_cli_config_list_parses() {
    let cli = Cli::try_parse_from(["harbor", "config", "list"]).expect("parse config list");
    assert!(matches!(
        cli.command,
        Some(Commands::Config {
            action: ConfigAction::List
        })
    ));
}

#[test]
fn test_cli_completion_fish_parses() {
    let cli = Cli::try_parse_from(["harbor", "completion", "fish"]).expect("parse completion fish");
    assert!(matches!(cli.command, Some(Commands::Completion { .. })));
}

#[test]
fn test_cli_generate_parses() {
    let cli = Cli::try_parse_from(["harbor", "generate", "setup.yaml", "myhost"])
        .expect("parse generate");
    match cli.command {
        Some(Commands::Generate {
            setup_config,
            hostname,
        }) => {
            assert_eq!(setup_config.to_str().expect("utf8"), "setup.yaml");
            assert_eq!(hostname.as_deref(), Some("myhost"));
        }
        other => panic!("expected Generate, got {other:?}"),
    }
}

#[test]
fn test_cli_generate_without_hostname_parses() {
    let cli =
        Cli::try_parse_from(["harbor", "generate", "setup.yaml"]).expect("parse generate no host");
    match cli.command {
        Some(Commands::Generate { hostname, .. }) => assert!(hostname.is_none()),
        other => panic!("expected Generate, got {other:?}"),
    }
}

#[test]
fn test_cli_global_config_flag() {
    let cli = Cli::try_parse_from(["harbor", "-c", "/custom/config.yaml", "version"])
        .expect("parse with global config");
    assert_eq!(
        cli.config.as_deref().and_then(|p| p.to_str()),
        Some("/custom/config.yaml")
    );
}

#[test]
fn test_cli_no_args_shows_help() {
    // No args now succeeds (shows help) instead of erroring
    let cli = Cli::try_parse_from(["harbor"]).expect("parse no args");
    assert!(cli.command.is_none());
}

#[test]
fn test_cli_unknown_command_errors() {
    let result = Cli::try_parse_from(["harbor", "nonexistent"]);
    assert!(result.is_err());
}
