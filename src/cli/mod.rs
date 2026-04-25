use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::{
    merge_resolved_dependencies, parse_and_flatten, print_merge_log, AspkgConfig, AspubConfig,
    InstallMode, InstallTarget, InstallTargets, SourceFile,
};
use crate::install::Installer;
use crate::resolver::DependencyResolver;

#[derive(Parser)]
#[command(name = "aspm")]
#[command(about = "AI Skill Package Manager", long_about = None)]
pub struct Cli {
    /// Show version
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new project
    Init(InitArgs),
    /// Install dependencies
    Install(InstallArgs),
    /// Manage cache
    Cache(CacheArgs),
    /// Add a dependency
    Add(AddArgs),
    /// Remove a dependency
    Remove(RemoveArgs),
    /// Show version
    Version,
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
    /// Install to specified directories (can be used multiple times)
    /// Format: <path> or <path>::<mode> (mode: plain|claude|compatible)
    /// Examples: --to .claude --to .cursor::plain --to .qwen::compatible
    #[arg(long, value_name = "TARGET")]
    pub to: Vec<String>,
    
    /// Extra dependencies config file (dependencies in this file will override aspkg.yaml)
    #[arg(long)]
    pub extra: Option<PathBuf>,
    
    /// Path to aspkg.yaml (default: ./aspkg.yaml)
    #[arg(long)]
    pub aspkg: Option<PathBuf>,
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

#[derive(Parser)]
pub struct AddArgs {
    /// Dependency name
    pub name: String,
    
    /// Git repository URL
    #[arg(long)]
    pub git: String,
    
    /// Branch name
    #[arg(long, group = "ref")]
    pub branch: Option<String>,
    
    /// Tag name
    #[arg(long, group = "ref")]
    pub tag: Option<String>,
    
    /// Commit hash
    #[arg(long, group = "ref")]
    pub commit: Option<String>,
    
    /// Overwrite existing dependency
    #[arg(long)]
    pub overwrite: bool,
    
    /// Path to aspkg.yaml
    #[arg(long, group = "config")]
    pub aspkg: Option<PathBuf>,
    
    /// Use aspub.yaml in current directory
    #[arg(long, group = "config")]
    pub aspub: bool,
}

#[derive(Parser)]
pub struct RemoveArgs {
    /// Dependency name to remove
    pub name: String,
    
    /// Path to aspkg.yaml
    #[arg(long, group = "config")]
    pub aspkg: Option<PathBuf>,
    
    /// Use aspub.yaml in current directory
    #[arg(long, group = "config")]
    pub aspub: bool,
}

// Command handlers

/// Parse a single --to argument into InstallTarget
/// Format: <path> or <path>::<mode>
fn parse_install_target(value: &str) -> Result<InstallTarget> {
    let parts: Vec<&str> = value.splitn(2, "::").collect();
    let path = PathBuf::from(parts[0].trim());
    
    if path.as_os_str().is_empty() {
        anyhow::bail!("path cannot be empty");
    }
    
    let mode = if parts.len() == 1 || parts[1].is_empty() {
        InstallMode::Auto
    } else {
        match parts[1].to_lowercase().as_str() {
            "plain" => InstallMode::Plain,
            "claude" => InstallMode::Claude,
            "compatible" => InstallMode::Compatible,
            other => anyhow::bail!("invalid mode '{}', expected 'plain', 'claude' or 'compatible'", other),
        }
    };
    
    Ok(InstallTarget::new(path, mode))
}


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
    // Get current working directory as project root
    let project_root = std::env::current_dir()?;
    
    // Determine aspkg.yaml path (convert relative to absolute if needed)
    let aspkg_path = match args.aspkg {
        Some(ref path) => {
            if path.is_absolute() {
                path.clone()
            } else {
                project_root.join(path)
            }
        }
        None => project_root.join("aspkg.yaml"),
    };
    if !aspkg_path.exists() {
        anyhow::bail!("No aspkg.yaml found at {}. Run 'aspm init --consumer' first.", 
            aspkg_path.display());
    }

    // 1. Parse and flatten aspkg.yaml
    println!("Reading config from {}...", aspkg_path.display());
    let base_deps = parse_and_flatten(&aspkg_path, SourceFile::Aspkg)?;

    // 2. Parse and flatten extra.yaml (if provided)
    let extra_deps = if let Some(extra_path) = &args.extra {
        if !extra_path.exists() {
            anyhow::bail!("Extra config file not found: {}", extra_path.display());
        }
        println!("Reading extra config from {}...", extra_path.display());
        parse_and_flatten(extra_path, SourceFile::Extra(extra_path.clone()))?
    } else {
        vec![]
    };

    // 3. Merge dependencies (extra overrides aspkg.yaml)
    println!("Merging dependencies (extra overrides aspkg.yaml)...");
    let merged_deps = merge_resolved_dependencies(base_deps, extra_deps);

    // 4. Check for aspub.yaml and merge install_to / check conflicts
    // aspub.yaml is always looked up in the current working directory
    let aspub_path = project_root.join("aspub.yaml");
    let aspub_config = if aspub_path.exists() {
        println!("Reading config from {}...", aspub_path.display());
        Some(AspubConfig::load(aspub_path.to_str().unwrap())?)
    } else {
        None
    };

    // 5. Check conflicts between merged (aspkg+extra) and aspub.yaml
    if let Some(ref config) = aspub_config {
        let merged_names: std::collections::HashSet<_> = merged_deps.iter().map(|d| &d.name).collect();
        for name in config.dependencies.keys() {
            if merged_names.contains(name) {
                anyhow::bail!(
                    "Dependency '{}' defined in both aspub.yaml and aspkg.yaml/extra. Please remove one manually.",
                    name
                );
            }
        }
    }

    // 6. Merge install_to: aspub.yaml's install_to overrides aspkg.yaml's global install_to
    let aspub_install_to = aspub_config.as_ref().and_then(|c| c.install_to.clone());
    let final_deps: Vec<_> = merged_deps.into_iter().map(|mut dep| {
        // If dependency doesn't have its own install_to, use aspub's global (if exists) or keep as is
        if let Some(ref install_to) = aspub_install_to {
            if dep.install_to.0.is_empty() || dep.install_to.0.iter().all(|t| t.path == std::path::PathBuf::from(".aspm")) {
                dep.install_to = install_to.clone();
            }
        }
        dep
    }).collect();

    // 7. Add aspub.yaml dependencies
    let mut all_deps = final_deps;
    if let Some(config) = &aspub_config {
        let aspub_install_to = config.install_to.clone().unwrap_or_default();
        for (name, source) in &config.dependencies {
            let install_to = source.install_to().cloned().unwrap_or_else(|| aspub_install_to.clone());
            all_deps.push(crate::config::ResolvedDependencyConfig {
                name: name.clone(),
                source: source.clone(),
                install_to,
                source_file: SourceFile::Aspkg, // Using Aspkg as placeholder for aspub
                overridden: false,
            });
        }
    }

    // 8. Print merge log
    print_merge_log(&all_deps, args.extra.as_deref());

    // 9. Build dependency map for resolver
    let mut dep_map = std::collections::HashMap::new();
    for dep in &all_deps {
        dep_map.insert(dep.name.clone(), dep.source.clone());
    }

    // 10. Resolve all dependencies recursively
    println!("Resolving dependencies...");
    let resolver = DependencyResolver::new()?;
    let mut resolved_deps = resolver.resolve_all_recursive(&dep_map)?;

    // 11. Attach install_to to resolved dependencies
    let install_to_map: std::collections::HashMap<_, _> = all_deps
        .iter()
        .map(|d| (d.name.clone(), d.install_to.clone()))
        .collect();

    for dep in &mut resolved_deps {
        if let Some(install_to) = install_to_map.get(&dep.name) {
            dep.install_to = Some(install_to.clone());
        }
    }

    // 12. Handle --to override (if specified, all deps install to those directories)
    if !args.to.is_empty() {
        let override_targets: InstallTargets = InstallTargets(
            args.to.iter()
                .map(|s| parse_install_target(s))
                .collect::<Result<Vec<_>>>()?
        );
        println!("Overriding install targets to: {:?}", override_targets.0);
        for dep in &mut resolved_deps {
            dep.install_to = Some(override_targets.clone());
        }
    }


    // 13. Collect all target directories for pruning
    let all_targets: std::collections::HashSet<_> = resolved_deps
        .iter()
        .flat_map(|dep| {
            dep.install_to
                .as_ref()
                .map(|t| t.0.iter().map(|target| target.path.clone()).collect::<Vec<_>>())
                .unwrap_or_default()
        })
        .collect();

    // 14. Prune old packages from all target directories
    println!("Pruning old packages...");
    let keep: std::collections::HashSet<String> = resolved_deps.iter().map(|d| d.name.clone()).collect();
    for target_path in &all_targets {
        let installer = Installer::new(vec![InstallTarget::new(target_path.clone(), InstallMode::Auto)]);
        installer.prune(&keep, &project_root)?;
    }

    // 15. Install each dependency with its own install_to
    println!("Installing {} dependencies...", resolved_deps.len());
    for dep in &resolved_deps {
        let targets = dep.install_to.clone().unwrap_or_else(|| {
            InstallTargets(vec![InstallTarget::new(
                std::path::PathBuf::from(".aspm"),
                InstallMode::Auto,
            )])
        });
        let installer = Installer::new(targets.0);
        installer.install(dep)?;
    }

    println!("Installed {} dependencies", resolved_deps.len());
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

pub fn handle_version() {
    println!("{}", env!("CARGO_PKG_VERSION"));
}

/// Determine the configuration file path based on arguments
enum ConfigType {
    Aspkg(PathBuf),
    Aspub(PathBuf),
}

fn detect_config_path(aspkg: Option<&PathBuf>, aspub: bool) -> Result<ConfigType> {
    // Check mutual exclusivity
    if aspkg.is_some() && aspub {
        anyhow::bail!("Cannot use both --aspkg and --aspub");
    }
    
    // If --aspkg is specified
    if let Some(path) = aspkg {
        if !path.exists() {
            anyhow::bail!("Configuration file not found: {}", path.display());
        }
        return Ok(ConfigType::Aspkg(path.clone()));
    }
    
    // If --aspub is specified
    if aspub {
        let path = std::path::PathBuf::from("aspub.yaml");
        if !path.exists() {
            anyhow::bail!("No aspub.yaml found in current directory");
        }
        return Ok(ConfigType::Aspub(path));
    }
    
    // Auto-detect: prefer aspkg.yaml, then aspub.yaml
    let aspkg_path = std::path::PathBuf::from("aspkg.yaml");
    if aspkg_path.exists() {
        return Ok(ConfigType::Aspkg(aspkg_path));
    }
    
    let aspub_path = std::path::PathBuf::from("aspub.yaml");
    if aspub_path.exists() {
        return Ok(ConfigType::Aspub(aspub_path));
    }
    
    anyhow::bail!(
        "No configuration file found. Run 'aspm init --consumer' or 'aspm init <name>' first."
    );
}

pub fn handle_add(args: AddArgs) -> Result<()> {
    use crate::config::DependencySource;
    
    // Determine config file path
    let config_type = detect_config_path(args.aspkg.as_ref(), args.aspub)?;
    
    match config_type {
        ConfigType::Aspkg(path) => {
            // Load aspkg.yaml
            let mut config = AspkgConfig::load(path.to_str().unwrap())?;
            
            // Check if dependency already exists
            if config.dependencies.contains_key(&args.name) && !args.overwrite {
                anyhow::bail!(
                    "Dependency '{}' already exists. Use --overwrite to replace it.",
                    args.name
                );
            }
            
            // Create dependency source
            let source = DependencySource::Detailed {
                git: Some(args.git.clone()),
                version: None,
                tag: args.tag.clone(),
                branch: args.branch.clone(),
                commit: args.commit.clone(),
                path: None,
                install_to: None,
            };
            
            // Add dependency
            config.dependencies.insert(args.name.clone(), source);
            
            // Save config
            config.save(path.to_str().unwrap())?;
            
            println!(
                "Dependency '{}' added to {}",
                args.name,
                path.display()
            );
        }
        ConfigType::Aspub(path) => {
            // Load aspub.yaml
            let mut config = AspubConfig::load(path.to_str().unwrap())?;
            
            // Check if dependency already exists
            if config.dependencies.contains_key(&args.name) && !args.overwrite {
                anyhow::bail!(
                    "Dependency '{}' already exists. Use --overwrite to replace it.",
                    args.name
                );
            }
            
            // Create dependency source
            let source = DependencySource::Detailed {
                git: Some(args.git.clone()),
                version: None,
                tag: args.tag.clone(),
                branch: args.branch.clone(),
                commit: args.commit.clone(),
                path: None,
                install_to: None,
            };
            
            // Add dependency
            config.dependencies.insert(args.name.clone(), source);
            
            // Save config
            config.save(path.to_str().unwrap())?;
            
            println!(
                "Dependency '{}' added to {}",
                args.name,
                path.display()
            );
        }
    }
    
    Ok(())
}

pub fn handle_remove(args: RemoveArgs) -> Result<()> {
    // Determine config file path
    let config_type = detect_config_path(args.aspkg.as_ref(), args.aspub)?;
    
    match config_type {
        ConfigType::Aspkg(path) => {
            // Load aspkg.yaml
            let mut config = AspkgConfig::load(path.to_str().unwrap())?;
            
            // Remove dependency (silently ignore if not exists)
            if config.dependencies.remove(&args.name).is_some() {
                // Save config
                config.save(path.to_str().unwrap())?;
                println!(
                    "Dependency '{}' removed from {}",
                    args.name,
                    path.display()
                );
            }
        }
        ConfigType::Aspub(path) => {
            // Load aspub.yaml
            let mut config = AspubConfig::load(path.to_str().unwrap())?;
            
            // Remove dependency (silently ignore if not exists)
            if config.dependencies.remove(&args.name).is_some() {
                // Save config
                config.save(path.to_str().unwrap())?;
                println!(
                    "Dependency '{}' removed from {}",
                    args.name,
                    path.display()
                );
            }
        }
    }
    
    Ok(())
}
