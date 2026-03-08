//! Dependency source specification

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Macro to generate field validation for map deserialization.
/// Usage: `field_validator!(fields => [field1, field2, ...])`
/// Returns a closure that validates field names and returns a formatted error message.
macro_rules! field_validator {
    ($fields:ident => [$($field:literal),* $(,)?]) => {
        |key: &str| {
            const FIELDS: &[&str] = &[$($field),*];
            if !FIELDS.contains(&key) {
                return Err(serde::de::Error::custom(format!(
                    "unknown field '{}', expected one of: {}",
                    key,
                    FIELDS.join(", ")
                )));
            }
            Ok(())
        }
    };
}

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
#[derive(Debug, Clone, PartialEq)]
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
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct InstallTargetVisitor;

        impl<'de> Visitor<'de> for InstallTargetVisitor {
            type Value = InstallTarget;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string path or an object with 'path' and optional 'mode'")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(InstallTarget::new(PathBuf::from(v), InstallMode::Auto))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(InstallTarget::new(PathBuf::from(v), InstallMode::Auto))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut path: Option<String> = None;
                let mut mode: Option<String> = None;
                let validate = field_validator!(fields => ["path", "mode"]);

                while let Some(key) = map.next_key::<String>()? {
                    validate(&key)?;
                    match key.as_str() {
                        "path" => path = Some(map.next_value()?),
                        "mode" => mode = Some(map.next_value()?),
                        _ => unreachable!(),
                    }
                }

                let path = path.ok_or_else(|| de::Error::missing_field("path"))?;
                
                let install_mode = match mode.as_deref() {
                    Some("plain") => InstallMode::Plain,
                    Some("claude") => InstallMode::Claude,
                    None => InstallMode::Auto,
                    Some(other) => {
                        return Err(de::Error::custom(format!(
                            "unknown install mode '{}', expected 'plain' or 'claude'",
                            other
                        )));
                    }
                };

                Ok(InstallTarget::new(PathBuf::from(path), install_mode))
            }
        }

        deserializer.deserialize_any(InstallTargetVisitor)
    }
}

impl Serialize for InstallTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstallTargets(pub Vec<InstallTarget>);

impl InstallTargets {
    #[allow(dead_code)]
    pub fn as_slice(&self) -> &[InstallTarget] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for InstallTargets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let targets = Vec::<InstallTarget>::deserialize(deserializer)?;
        Ok(InstallTargets(targets))
    }
}

impl Serialize for InstallTargets {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

/// Dependency source specification
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum DependencySource {
    /// Simple version string (>= semantic)
    Simple(String),
    
    /// Detailed source specification
    Detailed {
        /// Git repository URL
        #[serde(skip_serializing_if = "Option::is_none")]
        git: Option<String>,
        
        /// Version requirement (>=)
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        
        /// Git tag
        #[serde(skip_serializing_if = "Option::is_none")]
        tag: Option<String>,
        
        /// Git branch
        #[serde(skip_serializing_if = "Option::is_none")]
        branch: Option<String>,
        
        /// Git commit hash
        #[serde(skip_serializing_if = "Option::is_none")]
        commit: Option<String>,
        
        /// Local path
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
        
        /// Install targets for this dependency (overrides global install_to)
        #[serde(skip_serializing_if = "Option::is_none")]
        install_to: Option<InstallTargets>,
    },
}

impl<'de> Deserialize<'de> for DependencySource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct DependencySourceVisitor;

        impl<'de> Visitor<'de> for DependencySourceVisitor {
            type Value = DependencySource;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string or a dependency configuration object")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DependencySource::Simple(v.to_string()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DependencySource::Simple(v))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut git: Option<String> = None;
                let mut version: Option<String> = None;
                let mut tag: Option<String> = None;
                let mut branch: Option<String> = None;
                let mut commit: Option<String> = None;
                let mut path: Option<PathBuf> = None;
                let mut install_to: Option<InstallTargets> = None;
                let validate = field_validator!(fields => [
                    "git", "version", "tag", "branch", "commit", "path", "install_to"
                ]);

                while let Some(key) = map.next_key::<String>()? {
                    validate(&key)?;
                    match key.as_str() {
                        "git" => git = Some(map.next_value()?),
                        "version" => version = Some(map.next_value()?),
                        "tag" => tag = Some(map.next_value()?),
                        "branch" => branch = Some(map.next_value()?),
                        "commit" => commit = Some(map.next_value()?),
                        "path" => path = Some(map.next_value()?),
                        "install_to" => install_to = Some(map.next_value()?),
                        _ => unreachable!(),
                    }
                }

                Ok(DependencySource::Detailed {
                    git,
                    version,
                    tag,
                    branch,
                    commit,
                    path,
                    install_to,
                })
            }
        }

        deserializer.deserialize_any(DependencySourceVisitor)
    }
}

impl DependencySource {
    #[allow(dead_code)]
    pub fn git_url(&self) -> Option<&str> {
        match self {
            DependencySource::Simple(_) => None,
            DependencySource::Detailed { git, .. } => git.as_deref(),
        }
    }
    
    #[allow(dead_code)]
    pub fn version(&self) -> Option<&str> {
        match self {
            DependencySource::Simple(v) => Some(v),
            DependencySource::Detailed { version, .. } => version.as_deref(),
        }
    }
    
    #[allow(dead_code)]
    pub fn tag(&self) -> Option<&str> {
        match self {
            DependencySource::Simple(_) => None,
            DependencySource::Detailed { tag, .. } => tag.as_deref(),
        }
    }
    
    #[allow(dead_code)]
    pub fn branch(&self) -> Option<&str> {
        match self {
            DependencySource::Simple(_) => None,
            DependencySource::Detailed { branch, .. } => branch.as_deref(),
        }
    }
    
    #[allow(dead_code)]
    pub fn commit(&self) -> Option<&str> {
        match self {
            DependencySource::Simple(_) => None,
            DependencySource::Detailed { commit, .. } => commit.as_deref(),
        }
    }
    
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            DependencySource::Simple(_) => None,
            DependencySource::Detailed { path, .. } => path.as_ref(),
        }
    }
    
    pub fn install_to(&self) -> Option<&InstallTargets> {
        match self {
            DependencySource::Simple(_) => None,
            DependencySource::Detailed { install_to, .. } => install_to.as_ref(),
        }
    }
    
    #[allow(dead_code)]
    pub fn is_local(&self) -> bool {
        self.path().is_some()
    }
    
    #[allow(dead_code)]
    pub fn is_git(&self) -> bool {
        self.git_url().is_some()
    }
}

impl fmt::Display for DependencySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencySource::Simple(v) => write!(f, ">={}", v),
            DependencySource::Detailed { git, version, tag, branch, commit, path, install_to: _ } => {
                if let Some(p) = path {
                    write!(f, "path:{}", p.display())
                } else if let Some(g) = git {
                    write!(f, "git:{}", g)?;
                    if let Some(v) = version {
                        write!(f, " >={}", v)?;
                    }
                    if let Some(t) = tag {
                        write!(f, " tag:{}", t)?;
                    }
                    if let Some(b) = branch {
                        write!(f, " branch:{}", b)?;
                    }
                    if let Some(c) = commit {
                        write!(f, " commit:{}", c)?;
                    }
                    Ok(())
                } else {
                    write!(f, "unknown")
                }
            }
        }
    }
}
