//! aspkg.yaml configuration (consumer project)

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::path::PathBuf;

use super::DependencySource;

/// Install mode for a target directory
#[derive(Debug, Clone, PartialEq)]
pub enum InstallMode {
    /// Auto-detect: use Claude mode if path ends with `.claude`, otherwise Plain
    Auto,
    Plain,
    Claude,
}

impl Default for InstallMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// Resolved effective mode (after Auto is evaluated)
#[derive(Debug, Clone, PartialEq)]
pub enum EffectiveMode {
    Plain,
    Claude,
}

/// A single install target with path and optional mode override
#[derive(Debug, Clone)]
pub struct InstallTarget {
    pub path: PathBuf,
    pub mode: InstallMode,
}

impl InstallTarget {
    pub fn new(path: PathBuf, mode: InstallMode) -> Self {
        Self { path, mode }
    }

    /// Resolve the effective mode (evaluate Auto based on path name)
    pub fn effective_mode(&self) -> EffectiveMode {
        match self.mode {
            InstallMode::Plain => EffectiveMode::Plain,
            InstallMode::Claude => EffectiveMode::Claude,
            InstallMode::Auto => {
                if self.path.file_name().map(|n| n == ".claude").unwrap_or(false) {
                    EffectiveMode::Claude
                } else {
                    EffectiveMode::Plain
                }
            }
        }
    }
}

impl<'de> Deserialize<'de> for InstallTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawTarget {
            /// Plain string: "path/to/dir"
            String(String),
            /// Object: { path: "...", mode: "plain"|"claude" }
            Object { path: String, mode: Option<String> },
        }

        let raw = RawTarget::deserialize(deserializer)?;
        match raw {
            RawTarget::String(s) => Ok(InstallTarget::new(PathBuf::from(s), InstallMode::Auto)),
            RawTarget::Object { path, mode } => {
                let install_mode = match mode.as_deref() {
                    Some("plain") => InstallMode::Plain,
                    Some("claude") => InstallMode::Claude,
                    None => InstallMode::Auto,
                    Some(other) => {
                        return Err(serde::de::Error::custom(format!(
                            "unknown install mode '{}', expected 'plain' or 'claude'",
                            other
                        )))
                    }
                };
                Ok(InstallTarget::new(PathBuf::from(path), install_mode))
            }
        }
    }
}

impl Serialize for InstallTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        match self.mode {
            InstallMode::Auto => {
                // Serialize as plain string when mode is Auto
                serializer.serialize_str(self.path.to_str().unwrap_or(".aspm"))
            }
            InstallMode::Plain => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("path", self.path.to_str().unwrap_or(".aspm"))?;
                map.serialize_entry("mode", "plain")?;
                map.end()
            }
            InstallMode::Claude => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("path", self.path.to_str().unwrap_or(".aspm"))?;
                map.serialize_entry("mode", "claude")?;
                map.end()
            }
        }
    }
}

/// List of install targets
#[derive(Debug, Clone, Default)]
pub struct InstallTargets(pub Vec<InstallTarget>);

impl InstallTargets {
    pub fn as_slice(&self) -> &[InstallTarget] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for InstallTargets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let targets = Vec::<InstallTarget>::deserialize(deserializer)?;
        Ok(InstallTargets(targets))
    }
}

impl Serialize for InstallTargets {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

/// Consumer project configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AspkgConfig {
    /// Install target directories
    #[serde(default)]
    pub install_to: InstallTargets,

    /// Dependencies
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySource>,
}

impl Default for AspkgConfig {
    fn default() -> Self {
        Self {
            install_to: InstallTargets(vec![InstallTarget::new(
                PathBuf::from(".aspm"),
                InstallMode::Auto,
            )]),
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
