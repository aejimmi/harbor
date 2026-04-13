mod config;
mod deploy_cmd;
mod discover;
mod down;
mod exec_cmd;
mod fleet;
mod generate;
mod init;
mod logs_cmd;
pub mod output;
mod remote;
mod rollback_cmd;
mod server;
mod ssh_cmd;
mod status_cmd;
mod up;

#[cfg(test)]
mod cli_test;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

/// Server orchestration for Hetzner Cloud.
#[derive(Parser)]
#[command(name = "harbor", version, about, disable_help_flag = true)]
pub struct Cli {
    /// Path to user config file.
    #[arg(short = 'c', long = "config", global = true)]
    pub config: Option<PathBuf>,

    /// Print help.
    #[arg(long, global = true)]
    pub help: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    // --- Orchestration ---
    /// Create server, provision, and deploy.
    Up {
        #[arg(long)]
        debug: bool,
    },

    /// Destroy server.
    Down,

    /// Pull, rebuild, and restart.
    Deploy {
        #[arg(long)]
        debug: bool,
    },

    /// Rollback to a previous version.
    Rollback {
        /// Git SHA to rollback to. Omit for previous version.
        version: Option<String>,
        #[arg(long)]
        debug: bool,
    },

    /// Run a command on the server.
    Exec {
        /// Command and arguments to run.
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Show server and app state.
    Status,

    /// Shell into server.
    Ssh,

    /// Stream service logs.
    Logs {
        /// Service name (e.g. blissd). Omit for all.
        service: Option<String>,
    },

    // --- Infrastructure ---
    /// Manage individual servers.
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// Manage server fleets.
    #[command(alias = "env")]
    Fleet {
        #[command(subcommand)]
        action: FleetAction,
    },

    // --- Configuration ---
    /// Initialize harbor configuration.
    Init,

    /// Manage deployment configurations.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Generate setup commands for existing servers.
    Generate {
        /// Setup config file path.
        setup_config: PathBuf,
        /// Optional hostname.
        hostname: Option<String>,
    },

    /// Generate shell completion scripts.
    Completion {
        /// Shell to generate completions for.
        shell: clap_complete::Shell,
    },

    /// Show version information.
    Version,
}

#[derive(Debug, clap::Subcommand)]
pub enum ServerAction {
    /// Create a single server.
    Create {
        /// Server name.
        name: String,
        /// SSH key name (required).
        #[arg(long)]
        ssh_key: String,
        /// Server type.
        #[arg(long, default_value = "cax11")]
        r#type: String,
        /// Server location.
        #[arg(long, default_value = "nbg1")]
        location: String,
        /// Server image.
        #[arg(long, default_value = "ubuntu-24.04")]
        image: String,
        /// Hostname for DNS record.
        #[arg(long)]
        hostname: Option<String>,
        /// Setup configuration file.
        #[arg(long)]
        setup_config: Option<PathBuf>,
        /// Enable debug output.
        #[arg(long)]
        debug: bool,
        /// Minimize output.
        #[arg(long)]
        quiet: bool,
    },

    /// Delete a single server.
    Delete {
        /// Server name.
        name: String,
        /// Custom hostname for DNS cleanup.
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        quiet: bool,
    },

    /// List running servers.
    List,
}

#[derive(Debug, clap::Subcommand)]
pub enum FleetAction {
    /// Create and provision fleet servers.
    Up {
        /// Fleet name (e.g. staging, production).
        name: String,
        /// Fleet config file.
        #[arg(short = 'f', long, default_value = "fleet.yaml")]
        file: PathBuf,
        /// Run operations sequentially.
        #[arg(long)]
        sequential: bool,
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        quiet: bool,
    },

    /// Destroy all fleet servers.
    Down {
        /// Fleet name.
        name: String,
        /// Fleet config file.
        #[arg(short = 'f', long, default_value = "fleet.yaml")]
        file: PathBuf,
        #[arg(long)]
        debug: bool,
        #[arg(long)]
        quiet: bool,
    },

    /// Show fleet server status.
    Status {
        /// Fleet name.
        name: String,
        /// Fleet config file.
        #[arg(short = 'f', long, default_value = "fleet.yaml")]
        file: PathBuf,
    },
}

#[derive(Debug, clap::Subcommand)]
pub enum ConfigAction {
    /// Install a harbor config from a project.
    Install {
        /// Path to the harbor.yaml config file.
        path: PathBuf,
    },

    /// List installed configurations.
    List,

    /// Show an installed configuration.
    Show {
        /// Config name (e.g. `blissd`).
        name: String,
    },
}

/// Print grouped help output.
fn print_help() {
    let version = env!("CARGO_PKG_VERSION");
    eprintln!(
        "\
\x1b[1;35mharbor\x1b[0m — Server orchestration for Hetzner Cloud ({version})

\x1b[2mUsage:\x1b[0m harbor <command> [...flags]

\x1b[2mOrchestration:\x1b[0m
  \x1b[36mup\x1b[0m             Create server, provision, and deploy
  \x1b[36mdown\x1b[0m           Destroy server
  \x1b[36mdeploy\x1b[0m         Pull, rebuild, and restart
  \x1b[36mrollback\x1b[0m       Rollback to a previous version
  \x1b[36mstatus\x1b[0m         Show server and app state
  \x1b[36mssh\x1b[0m            Shell into server
  \x1b[36mexec\x1b[0m           Run a command on the server
  \x1b[36mlogs\x1b[0m           Stream service logs

\x1b[2mInfrastructure:\x1b[0m
  \x1b[36mserver\x1b[0m         Manage individual servers
  \x1b[36mfleet\x1b[0m          Manage server fleets

\x1b[2mConfiguration:\x1b[0m
  \x1b[36minit\x1b[0m           Initialize harbor config
  \x1b[36mconfig\x1b[0m         Manage deployment configs
  \x1b[36mgenerate\x1b[0m       Generate setup script

  <command> --help    Print command help"
    );
}

/// Execute the parsed CLI command.
pub async fn run(cli: Cli) -> Result<()> {
    if cli.help || cli.command.is_none() {
        print_help();
        return Ok(());
    }

    match cli.command.expect("command is present after help check") {
        // Orchestration
        Commands::Up { debug } => up::run(debug, cli.config.as_deref()).await,
        Commands::Down => down::run(cli.config.as_deref()).await,
        Commands::Deploy { debug } => deploy_cmd::run(debug, cli.config.as_deref()).await,
        Commands::Rollback { version, debug } => {
            rollback_cmd::run(version, debug, cli.config.as_deref()).await
        }
        Commands::Exec { command } => exec_cmd::run(&command, cli.config.as_deref()).await,
        Commands::Status => status_cmd::run(cli.config.as_deref()).await,
        Commands::Ssh => ssh_cmd::run(cli.config.as_deref()).await,
        Commands::Logs { service } => {
            logs_cmd::run(service.as_deref(), cli.config.as_deref()).await
        }

        // Infrastructure
        Commands::Server { action } => server::run(action, cli.config.as_deref()).await,
        Commands::Fleet { action } => fleet::run(action, cli.config.as_deref()).await,

        // Configuration
        Commands::Init => init::run(),
        Commands::Version => {
            output::header(&format!("Harbor {}", env!("CARGO_PKG_VERSION")));
            Ok(())
        }
        Commands::Config { action } => match action {
            ConfigAction::Install { path } => config::install(&path),
            ConfigAction::List => {
                config::list();
                Ok(())
            }
            ConfigAction::Show { name } => config::show(&name),
        },
        Commands::Generate {
            setup_config,
            hostname,
        } => generate::run(&setup_config, hostname.as_deref()),
        Commands::Completion { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "harbor", &mut std::io::stdout());
            Ok(())
        }
    }
}
