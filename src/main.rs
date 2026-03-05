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

    match cli.command {
        Commands::Init(args) => cli::handle_init(args),
        Commands::Install(args) => cli::handle_install(args),
        Commands::Cache(args) => cli::handle_cache(args),
    }
}
