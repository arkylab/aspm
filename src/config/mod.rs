//! Configuration file parsing (aspub.yaml / aspkg.yaml)

mod aspkg;
mod aspub;
mod dependency;

pub use aspkg::{AspkgConfig, EffectiveMode, InstallMode, InstallTarget, InstallTargets};
pub use aspub::AspubConfig;
pub use dependency::DependencySource;

use anyhow::{bail, Result, Context};
use std::collections::HashMap;
use std::path::Path;

/// Detect project type and load config
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
