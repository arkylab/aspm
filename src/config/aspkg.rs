//! aspkg.yaml configuration (consumer project)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::{DependencySource, InstallTargets, InstallTarget, InstallMode};

/// Consumer project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AspkgConfig {
    /// Install target directories
    #[serde(default = "default_install_to")]
    pub install_to: InstallTargets,

    /// Dependencies
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySource>,
}

fn default_install_to() -> InstallTargets {
    InstallTargets(vec![InstallTarget::new(
        PathBuf::from(".aspm"),
        InstallMode::Auto,
    )])
}

impl Default for AspkgConfig {
    fn default() -> Self {
        Self {
            install_to: default_install_to(),
            dependencies: HashMap::new(),
        }
    }
}

impl AspkgConfig {
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
