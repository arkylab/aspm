//! Configuration file parsing (aspub.yaml / aspkg.yaml)

mod aspkg;
mod aspub;
mod dependency;

pub use aspkg::AspkgConfig;
pub use aspub::AspubConfig;
pub use dependency::{DependencySource, EffectiveMode, InstallMode, InstallTarget, InstallTargets};

use anyhow::{bail, Result, Context};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Source file for a dependency
#[derive(Debug, Clone)]
pub enum SourceFile {
    Aspkg,
    Extra(PathBuf),
}

impl SourceFile {
    #[allow(dead_code)]
    pub fn display_name(&self) -> String {
        match self {
            SourceFile::Aspkg => "aspkg.yaml".to_string(),
            SourceFile::Extra(path) => path.display().to_string(),
        }
    }
}

/// Resolved dependency config with flattened install_to
#[derive(Debug, Clone)]
pub struct ResolvedDependencyConfig {
    pub name: String,
    pub source: DependencySource,
    pub install_to: InstallTargets,
    pub source_file: SourceFile,
    /// Whether this dependency was overridden by extra config
    pub overridden: bool,
}

impl ResolvedDependencyConfig {
    /// Get the git URL or path for display
    pub fn source_display(&self) -> String {
        match &self.source {
            DependencySource::Simple(v) => format!("version >={}", v),
            DependencySource::Detailed { git, path, .. } => {
                if let Some(p) = path {
                    format!("path:{}", p.display())
                } else if let Some(g) = git {
                    g.clone()
                } else {
                    "unknown".to_string()
                }
            }
        }
    }
}

/// Detect project type and load config
#[allow(dead_code)]
pub enum ConfigType {
    Publish(AspubConfig),
    Consumer(AspkgConfig),
    /// Both publish and consumer configs exist, with merged dependencies
    Both {
        #[allow(dead_code)]
        publish: AspubConfig,
        consumer: AspkgConfig,
        merged_dependencies: HashMap<String, DependencySource>,
    },
}

#[allow(dead_code)]
impl ConfigType {
    pub fn detect() -> Result<Self> {
        let has_aspub = Path::new("aspub.yaml").exists();
        let has_aspkg = Path::new("aspkg.yaml").exists();
        
        match (has_aspub, has_aspkg) {
            (true, true) => {
                let publish = AspubConfig::load("aspub.yaml").context("Failed to load aspub.yaml")?;
                let consumer = AspkgConfig::load("aspkg.yaml").context("Failed to load aspkg.yaml")?;
                
                println!("Detected both aspub.yaml and aspkg.yaml, merging dependencies...");
                
                let merged = Self::merge_dependencies(&publish.dependencies, &consumer.dependencies)?;
                
                Ok(ConfigType::Both {
                    publish,
                    consumer,
                    merged_dependencies: merged,
                })
            }
            (true, false) => {
                let config = AspubConfig::load("aspub.yaml")?;
                Ok(ConfigType::Publish(config))
            }
            (false, true) => {
                let config = AspkgConfig::load("aspkg.yaml")?;
                Ok(ConfigType::Consumer(config))
            }
            (false, false) => {
                bail!("No configuration file found. Run 'aspm init <name>' or 'aspm init --consumer' first.");
            }
        }
    }
    
    /// Merge dependencies from publish and consumer configs
    /// If same dependency exists in both, error out - let user manually resolve
    fn merge_dependencies(
        publish_deps: &HashMap<String, DependencySource>,
        consumer_deps: &HashMap<String, DependencySource>,
    ) -> Result<HashMap<String, DependencySource>> {
        let mut merged = publish_deps.clone();
        
        for (name, consumer_source) in consumer_deps {
            if merged.contains_key(name) {
                bail!("Dependency '{}' defined in both aspub.yaml and aspkg.yaml. Please remove one manually.", name);
            }
            merged.insert(name.clone(), consumer_source.clone());
        }
        
        Ok(merged)
    }
    
    /// Get dependencies based on config type
    pub fn get_dependencies(&self) -> &HashMap<String, DependencySource> {
        match self {
            ConfigType::Publish(config) => &config.dependencies,
            ConfigType::Consumer(config) => &config.dependencies,
            ConfigType::Both { merged_dependencies, .. } => merged_dependencies,
        }
    }
    
    /// Get install_to directories
    pub fn get_install_to(&self) -> InstallTargets {
        match self {
            ConfigType::Publish(config) => {
                config.install_to.clone().unwrap_or_default()
            }
            ConfigType::Consumer(config) => config.install_to.clone(),
            ConfigType::Both { consumer, .. } => consumer.install_to.clone(),
        }
    }
}

// ── Extra config merge functions ─────────────────────────────────────────────

/// Parse a config file and flatten install_to into each dependency
pub fn parse_and_flatten(path: &Path, source_file: SourceFile) -> Result<Vec<ResolvedDependencyConfig>> {
    let config = AspkgConfig::load(path.to_str().context("Invalid path")?)
        .with_context(|| format!("Failed to load {}", path.display()))?;
    
    let global_install_to = config.install_to.clone();
    
    config.dependencies.into_iter().map(|(name, mut source)| {
        // Flatten: if dependency doesn't have its own install_to, use global
        if let DependencySource::Detailed { install_to, .. } = &mut source {
            if install_to.is_none() {
                *install_to = Some(global_install_to.clone());
            }
        }
        
        let install_to = source.install_to().cloned().unwrap_or_else(|| global_install_to.clone());
        
        Ok(ResolvedDependencyConfig {
            name,
            source,
            install_to,
            source_file: source_file.clone(),
            overridden: false,
        })
    }).collect()
}

/// Merge two resolved dependency lists (extra overrides base)
pub fn merge_resolved_dependencies(
    base: Vec<ResolvedDependencyConfig>,
    extra: Vec<ResolvedDependencyConfig>,
) -> Vec<ResolvedDependencyConfig> {
    let mut map: HashMap<String, ResolvedDependencyConfig> = HashMap::new();
    
    // Insert base dependencies
    for dep in base {
        map.insert(dep.name.clone(), dep);
    }
    
    // Override with extra dependencies
    for mut dep in extra {
        // Mark as overridden if it existed in base
        dep.overridden = map.contains_key(&dep.name);
        map.insert(dep.name.clone(), dep);
    }
    
    map.into_values().collect()
}

/// Print merge log with detailed information
pub fn print_merge_log(deps: &[ResolvedDependencyConfig], extra_path: Option<&Path>) {
    if extra_path.is_none() {
        // No extra config, just print simple info
        println!("Loaded {} dependencies from aspkg.yaml", deps.len());
        return;
    }
    
    println!("Merged dependencies:");
    for dep in deps {
        let source_info = match &dep.source_file {
            SourceFile::Aspkg => "(from aspkg.yaml)".to_string(),
            SourceFile::Extra(path) => {
                if dep.overridden {
                    format!("(from {}, overridden)", path.display())
                } else {
                    format!("(from {}, added)", path.display())
                }
            }
        };
        
        let install_to_str: Vec<String> = dep.install_to.0.iter()
            .map(|t| t.path.display().to_string())
            .collect();
        
        println!("  {}:", dep.name);
        println!("    source: {} {}", dep.source_display(), source_info);
        println!("    install_to: [{}]", install_to_str.join(", "));
    }
}
