//! aspm - AI Skill Package Manager

mod cli;
mod config;
mod git;
mod install;
mod publish;
mod resolver;
mod version;

use anyhow::Result;
use clap::Parser;

use crate::cli::Commands;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    if cli.version {
        cli::handle_version();
        return Ok(());
    }

    match cli.command {
        Some(Commands::Init(args)) => cli::handle_init(args),
        Some(Commands::Install(args)) => cli::handle_install(args),
        Some(Commands::Cache(args)) => cli::handle_cache(args),
        Some(Commands::Version) => {
            cli::handle_version();
            Ok(())
        }
        None => {
            cli::Cli::parse_from(["aspm", "--help"]);
            Ok(())
        }
    }
}
