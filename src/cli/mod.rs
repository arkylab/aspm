use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::{AspkgConfig, AspubConfig, ConfigType, InstallMode, InstallTarget};
use crate::install::Installer;
use crate::resolver::DependencyResolver;

#[derive(Parser)]
#[command(name = "aspm")]
#[command(about = "AI Skill Package Manager", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new project
    Init(InitArgs),
    /// Install dependencies
    Install(InstallArgs),
    /// Manage cache
    Cache(CacheArgs),
}

#[derive(Parser)]
pub struct InitArgs {
    /// Package name (required for publish project)
    pub name: Option<String>,

    /// Initialize as consumer project (install only)
    #[arg(long)]
    pub consumer: bool,

    /// Initial version for publish project
    #[arg(long, default_value = "0.1.0")]
    pub version: String,
}

#[derive(Parser)]
pub struct InstallArgs {
    /// Install to specified directory
    #[arg(long)]
    pub to: Option<PathBuf>,
}

#[derive(Parser)]
pub struct CacheArgs {
    #[command(subcommand)]
    pub action: CacheAction,
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// Clear all cache
    Clean,
    /// Show cache directory
    Dir,
    /// List cached repositories
    List,
}

// Command handlers

pub fn handle_init(args: InitArgs) -> Result<()> {
    if args.name.is_none() && !args.consumer {
        anyhow::bail!("Error: must specify package name or use --consumer flag\n\nUsage:\n  aspm init <name>       Create a publish project\n  aspm init --consumer   Create a consumer project");
    }

    if args.consumer {
        if args.name.is_some() {
            anyhow::bail!("Error: --consumer flag cannot be used with a package name");
        }
        let config = AspkgConfig::default();
        config.save("aspkg.yaml")?;
        println!("Created aspkg.yaml (consumer project)");
    } else {
        let config = AspubConfig {
            name: args.name.unwrap(),
            version: args.version,
            ..Default::default()
        };
        config.save("aspub.yaml")?;
        println!("Created aspub.yaml (publish project)");
    }

    Ok(())
}

pub fn handle_install(args: InstallArgs) -> Result<()> {
    let config_type = ConfigType::detect()?;

    let install_targets = args.to.clone()
        .map(|p| vec![InstallTarget::new(p, InstallMode::Auto)])
        .unwrap_or_else(|| config_type.get_install_to().as_slice().to_vec());

    let dependencies = config_type.get_dependencies();
    let resolver = DependencyResolver::new()?;
    let all_deps = resolver.resolve_all_recursive(dependencies)?;

    let installer = Installer::new(install_targets);
    installer.install_all(&all_deps)?;

    println!("Installed {} dependencies", all_deps.len());
    Ok(())
}

pub fn handle_cache(args: CacheArgs) -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
        .join("aspm")
        .join("repos");

    match args.action {
        CacheAction::Clean => {
            if !cache_dir.exists() {
                println!("Cache directory does not exist: {}", cache_dir.display());
                return Ok(());
            }
            let mut count = 0;
            for entry in std::fs::read_dir(&cache_dir)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                    count += 1;
                }
            }
            println!("Cleaned {} cached repositories in {}", count, cache_dir.display());
        }
        CacheAction::Dir => {
            println!("{}", cache_dir.display());
        }
        CacheAction::List => {
            if !cache_dir.exists() {
                println!("Cache directory does not exist: {}", cache_dir.display());
                return Ok(());
            }
            println!("Cached repositories in {}:", cache_dir.display());
            for entry in std::fs::read_dir(&cache_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let remote_url = std::process::Command::new("git")
                        .args(["-C", path.to_str().unwrap(), "remote", "get-url", "origin"])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    println!("  {} - {}", entry.file_name().to_string_lossy(), remote_url);
                }
            }
        }
    }

    Ok(())
}
