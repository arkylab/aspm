//! Semantic version parsing and comparison

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// Semantic version
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
}

impl SemanticVersion {
    #[allow(dead_code)]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
        }
    }
    
    pub fn parse(s: &str) -> Result<Self, ParseVersionError> {
        // Remove 'v' prefix if present (common in git tags like "v1.0.0")
        let s = s.strip_prefix('v').unwrap_or(s);
        
        // Handle prerelease suffix (e.g., "1.0.0-beta")
        let (version_part, prerelease) = if let Some(idx) = s.find('-') {
            (&s[..idx], Some(s[idx + 1..].to_string()))
        } else {
            (s, None)
        };
        
        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(ParseVersionError::InvalidFormat(s.to_string()));
        }
        
        let major = parts[0]
            .parse()
            .map_err(|_| ParseVersionError::InvalidPart(parts[0].to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| ParseVersionError::InvalidPart(parts[1].to_string()))?;
        let patch = if parts.len() == 3 {
            parts[2]
                .parse()
                .map_err(|_| ParseVersionError::InvalidPart(parts[2].to_string()))?
        } else {
            0
        };
        
        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
        })
    }
    
    /// Check if this version satisfies >= requirement
    #[allow(dead_code)]
    pub fn satisfies_gte(&self, requirement: &str) -> bool {
        let req = match Self::parse(requirement) {
            Ok(v) => v,
            Err(_) => return false,
        };
        self >= &req
    }
}

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemanticVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => match self.patch.cmp(&other.patch) {
                    Ordering::Equal => {
                        // When version numbers are equal, prefer shorter version string
                        // This means stable versions (no prerelease) are preferred over prerelease versions
                        let self_len = self.to_string().len();
                        let other_len = other.to_string().len();
                        other_len.cmp(&self_len) // Shorter = higher priority
                    }
                    ord => ord,
                },
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.prerelease {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

impl FromStr for SemanticVersion {
    type Err = ParseVersionError;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[derive(Debug, Clone)]
pub enum ParseVersionError {
    InvalidFormat(String),
    InvalidPart(String),
}

impl fmt::Display for ParseVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseVersionError::InvalidFormat(s) => write!(f, "Invalid version format: {}", s),
            ParseVersionError::InvalidPart(s) => write!(f, "Invalid version part: {}", s),
        }
    }
}

impl std::error::Error for ParseVersionError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_version() {
        let v = SemanticVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        
        let v = SemanticVersion::parse("v2.0.0").unwrap();
        assert_eq!(v.major, 2);
        
        let v = SemanticVersion::parse("1.0.0-beta").unwrap();
        assert_eq!(v.prerelease, Some("beta".to_string()));
    }
    
    #[test]
    fn test_version_comparison() {
        let v1 = SemanticVersion::parse("1.0.0").unwrap();
        let v2 = SemanticVersion::parse("2.0.0").unwrap();
        let v3 = SemanticVersion::parse("1.1.0").unwrap();
        let v4 = SemanticVersion::parse("1.0.1").unwrap();
        
        assert!(v2 > v1);
        assert!(v3 > v1);
        assert!(v4 > v1);
        assert!(v3 < v2);
    }
    
    #[test]
    fn test_satisfies_gte() {
        let v = SemanticVersion::parse("2.1.0").unwrap();
        assert!(v.satisfies_gte("1.0.0"));
        assert!(v.satisfies_gte("2.0.0"));
        assert!(v.satisfies_gte("2.1.0"));
        assert!(!v.satisfies_gte("2.2.0"));
        assert!(!v.satisfies_gte("3.0.0"));
    }
}
