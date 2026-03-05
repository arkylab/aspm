//! aspub.yaml configuration (publish project)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{DependencySource, InstallTargets};

/// Publish project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AspubConfig {
    /// Package name (required)
    pub name: String,

    /// Package version (required)
    pub version: String,

    /// Package description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Author name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// License
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Install target directories (for this package's own dependencies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_to: Option<InstallTargets>,

    /// Resources to publish (key is type, value is list of paths)
    /// Each path may be absolute or relative (relative to aspub.yaml directory).
    /// Each segment may be a regex. Trailing slash means directory, otherwise file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish: Option<HashMap<String, Vec<String>>>,

    /// Dependencies
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySource>,
}

impl Default for AspubConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.1.0".to_string(),
            description: None,
            author: None,
            license: None,
            install_to: None,
            publish: None,
            dependencies: HashMap::new(),
        }
    }
}

impl AspubConfig {
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
