mod cli;
mod config;
mod dns;
mod provider;
mod provision;
mod script;

use anyhow::Result;
use clap::Parser;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    tokio::select! {
        result = cli::run(cli) => result,
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nReceived interrupt signal, cleaning up...");
            Ok(())
        }
    }
}
