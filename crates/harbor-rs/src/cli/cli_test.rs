use super::*;
use clap::Parser;

#[test]
fn test_cli_init_parses() {
    let cli = Cli::try_parse_from(["harbor", "init"]).expect("parse init");
    assert!(matches!(cli.command, Commands::Init));
}

#[test]
fn test_cli_version_parses() {
    let cli = Cli::try_parse_from(["harbor", "version"]).expect("parse version");
    assert!(matches!(cli.command, Commands::Version));
}

#[test]
fn test_cli_server_list_parses() {
    let cli = Cli::try_parse_from(["harbor", "server", "list"]).expect("parse server list");
    assert!(matches!(
        cli.command,
        Commands::Server {
            action: ServerAction::List
        }
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
        Commands::Server {
            action:
                ServerAction::Create {
                    name,
                    ssh_key,
                    r#type,
                    location,
                    ..
                },
        } => {
            assert_eq!(name, "myserver");
            assert_eq!(ssh_key, "mykey");
            assert_eq!(r#type, "cax11"); // default
            assert_eq!(location, "nbg1"); // default
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
        "tergar",
        "--debug",
        "--quiet",
    ])
    .expect("parse server create with all flags");

    match cli.command {
        Commands::Server {
            action:
                ServerAction::Create {
                    name,
                    ssh_key,
                    r#type,
                    location,
                    image,
                    hostname,
                    debug,
                    quiet,
                    ..
                },
        } => {
            assert_eq!(name, "myserver");
            assert_eq!(ssh_key, "mykey");
            assert_eq!(r#type, "cpx31");
            assert_eq!(location, "fsn1");
            assert_eq!(image, "debian-12");
            assert_eq!(hostname.as_deref(), Some("tergar"));
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
        "tergar",
    ])
    .expect("parse server delete");

    match cli.command {
        Commands::Server {
            action: ServerAction::Delete { name, hostname, .. },
        } => {
            assert_eq!(name, "myserver");
            assert_eq!(hostname.as_deref(), Some("tergar"));
        }
        other => panic!("expected Server Delete, got {other:?}"),
    }
}

#[test]
fn test_cli_env_deploy_parses() {
    let cli = Cli::try_parse_from(["harbor", "env", "deploy", "production.yaml", "--sequential"])
        .expect("parse env deploy");

    match cli.command {
        Commands::Env {
            action:
                EnvAction::Deploy {
                    config_file,
                    sequential,
                    ..
                },
        } => {
            assert_eq!(config_file.to_str().expect("utf8"), "production.yaml");
            assert!(sequential);
        }
        other => panic!("expected Env Deploy, got {other:?}"),
    }
}

#[test]
fn test_cli_env_destroy_parses() {
    let cli =
        Cli::try_parse_from(["harbor", "env", "destroy", "prod.yaml"]).expect("parse env destroy");

    assert!(matches!(
        cli.command,
        Commands::Env {
            action: EnvAction::Destroy { .. }
        }
    ));
}

#[test]
fn test_cli_env_list_parses() {
    let cli = Cli::try_parse_from(["harbor", "env", "list"]).expect("parse env list");
    assert!(matches!(
        cli.command,
        Commands::Env {
            action: EnvAction::List
        }
    ));
}

#[test]
fn test_cli_environment_alias_parses() {
    let cli =
        Cli::try_parse_from(["harbor", "environment", "list"]).expect("parse environment alias");
    assert!(matches!(
        cli.command,
        Commands::Env {
            action: EnvAction::List
        }
    ));
}

#[test]
fn test_cli_config_list_parses() {
    let cli = Cli::try_parse_from(["harbor", "config", "list"]).expect("parse config list");
    assert!(matches!(
        cli.command,
        Commands::Config {
            action: ConfigAction::List
        }
    ));
}

#[test]
fn test_cli_completion_fish_parses() {
    let cli = Cli::try_parse_from(["harbor", "completion", "fish"]).expect("parse completion fish");
    assert!(matches!(cli.command, Commands::Completion { .. }));
}

#[test]
fn test_cli_generate_parses() {
    let cli = Cli::try_parse_from(["harbor", "generate", "setup.yaml", "myhost"])
        .expect("parse generate");
    match cli.command {
        Commands::Generate {
            setup_config,
            hostname,
        } => {
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
        Commands::Generate { hostname, .. } => assert!(hostname.is_none()),
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
fn test_cli_no_args_errors() {
    let result = Cli::try_parse_from(["harbor"]);
    assert!(result.is_err());
}

#[test]
fn test_cli_unknown_command_errors() {
    let result = Cli::try_parse_from(["harbor", "nonexistent"]);
    assert!(result.is_err());
}
