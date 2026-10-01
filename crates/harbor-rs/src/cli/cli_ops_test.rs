#![allow(clippy::panic, clippy::indexing_slicing, clippy::unwrap_used)]

use super::*;
use clap::Parser;

#[test]
fn test_cli_rollback_with_name_parses() {
    let cli = Cli::try_parse_from(["harbor", "rollback", "web"]).expect("parse rollback web");
    match cli.command {
        Some(Commands::Rollback {
            name,
            version,
            debug,
        }) => {
            assert_eq!(name, "web");
            assert!(version.is_none());
            assert!(!debug);
        }
        other => panic!("expected Rollback, got {other:?}"),
    }
}

#[test]
fn test_cli_rollback_with_name_and_version_parses() {
    let cli = Cli::try_parse_from(["harbor", "rollback", "web", "abc123f"])
        .expect("parse rollback web abc123f");
    match cli.command {
        Some(Commands::Rollback { name, version, .. }) => {
            assert_eq!(name, "web");
            assert_eq!(version.as_deref(), Some("abc123f"));
        }
        other => panic!("expected Rollback, got {other:?}"),
    }
}

#[test]
fn test_cli_rollback_debug_parses() {
    let cli = Cli::try_parse_from(["harbor", "rollback", "web", "--debug"])
        .expect("parse rollback web --debug");
    match cli.command {
        Some(Commands::Rollback { name, debug, .. }) => {
            assert_eq!(name, "web");
            assert!(debug);
        }
        other => panic!("expected Rollback, got {other:?}"),
    }
}

#[test]
fn test_cli_rollback_without_name_errors() {
    let result = Cli::try_parse_from(["harbor", "rollback"]);
    assert!(result.is_err(), "rollback with no name must error");
}

#[test]
fn test_cli_exec_parses() {
    let cli = Cli::try_parse_from(["harbor", "exec", "systemctl", "restart", "blissd"])
        .expect("parse exec");
    match cli.command {
        Some(Commands::Exec { command }) => {
            assert_eq!(command, vec!["systemctl", "restart", "blissd"]);
        }
        other => panic!("expected Exec, got {other:?}"),
    }
}

#[test]
fn test_cli_exec_single_arg_parses() {
    let cli = Cli::try_parse_from(["harbor", "exec", "uptime"]).expect("parse exec uptime");
    match cli.command {
        Some(Commands::Exec { command }) => {
            assert_eq!(command, vec!["uptime"]);
        }
        other => panic!("expected Exec, got {other:?}"),
    }
}

#[test]
fn test_cli_exec_requires_command() {
    let result = Cli::try_parse_from(["harbor", "exec"]);
    assert!(result.is_err());
}

// --- harbor backup (spec 015 R1) ---

#[test]
fn test_cli_backup_alone_parses_as_run() {
    // No subcommand = trigger a backup now. Project is resolved
    // from `harbor.yaml` in cwd, matching `harbor up`/`deploy`.
    let cli = Cli::try_parse_from(["harbor", "backup"]).expect("parse");
    match cli.command {
        Some(Commands::Backup { action, debug }) => {
            assert!(action.is_none(), "bare `backup` has no subcommand");
            assert!(!debug);
        }
        other => panic!("expected Backup (no action), got {other:?}"),
    }
}

#[test]
fn test_cli_backup_with_debug_flag_parses() {
    let cli = Cli::try_parse_from(["harbor", "backup", "--debug"]).expect("parse");
    match cli.command {
        Some(Commands::Backup { action, debug }) => {
            assert!(action.is_none());
            assert!(debug);
        }
        other => panic!("expected Backup, got {other:?}"),
    }
}

#[test]
fn test_cli_backup_list_parses() {
    let cli = Cli::try_parse_from(["harbor", "backup", "list"]).expect("parse");
    match cli.command {
        Some(Commands::Backup { action, .. }) => {
            assert!(
                matches!(action, Some(BackupAction::List)),
                "expected Backup List subcommand"
            );
        }
        other => panic!("expected Backup List, got {other:?}"),
    }
}

#[test]
fn test_help_contains_backup_line() {
    // The grouped help block is a const string in `print_help`.
    // Render it via `Cli::command` + long help and look for the word.
    let mut cmd = <Cli as clap::CommandFactory>::command();
    let help = cmd.render_long_help().to_string();
    assert!(
        help.contains("backup"),
        "clap help must list the backup command: {help}"
    );
}

// --- harbor restore (spec 016 R1) ---

#[test]
fn test_cli_restore_bare_parses() {
    let cli = Cli::try_parse_from(["harbor", "restore"]).expect("parse");
    match cli.command {
        Some(Commands::Restore { at, yes, debug }) => {
            assert!(at.is_none());
            assert!(!yes);
            assert!(!debug);
        }
        other => panic!("expected Restore, got {other:?}"),
    }
}

#[test]
fn test_cli_restore_with_at_parses() {
    let cli =
        Cli::try_parse_from(["harbor", "restore", "--at", "20260420T130000Z"]).expect("parse");
    match cli.command {
        Some(Commands::Restore { at, .. }) => {
            assert_eq!(at.as_deref(), Some("20260420T130000Z"));
        }
        other => panic!("expected Restore, got {other:?}"),
    }
}

#[test]
fn test_cli_restore_with_yes_parses() {
    let cli = Cli::try_parse_from(["harbor", "restore", "--yes"]).expect("parse");
    match cli.command {
        Some(Commands::Restore { yes, .. }) => assert!(yes),
        other => panic!("expected Restore, got {other:?}"),
    }
}

#[test]
fn test_cli_restore_with_at_and_yes_parses() {
    let cli = Cli::try_parse_from(["harbor", "restore", "--at", "20260420T130000Z", "--yes"])
        .expect("parse");
    match cli.command {
        Some(Commands::Restore { at, yes, .. }) => {
            assert_eq!(at.as_deref(), Some("20260420T130000Z"));
            assert!(yes);
        }
        other => panic!("expected Restore, got {other:?}"),
    }
}

#[test]
fn test_help_contains_restore_line() {
    let mut cmd = <Cli as clap::CommandFactory>::command();
    let help = cmd.render_long_help().to_string();
    assert!(
        help.contains("restore"),
        "clap help must list the restore command: {help}"
    );
}
