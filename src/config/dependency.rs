//! Dependency source specification

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Dependency source specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    },
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
            DependencySource::Detailed { git, version, tag, branch, commit, path } => {
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
