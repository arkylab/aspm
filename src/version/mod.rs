//! Version parsing and comparison

mod semantic;

pub use semantic::SemanticVersion;

use regex::Regex;

/// Parse version string from tag or branch name
pub fn parse_version(s: &str) -> Option<SemanticVersion> {
    // Remove 'v' prefix if present
    let s = s.strip_prefix('v').unwrap_or(s);
    
    // Try to parse as semantic version
    SemanticVersion::parse(s).ok()
}

/// Check if a string looks like a version
#[allow(dead_code)]
pub fn is_version_like(s: &str) -> bool {
    // Remove 'v' prefix if present
    let s = s.strip_prefix('v').unwrap_or(s);
    
    // Check if it matches semantic version pattern
    let re = Regex::new(r"^\d+\.\d+\.\d+").unwrap();
    re.is_match(s)
}

/// Compare two version strings
#[allow(dead_code)]
pub fn compare_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let va = parse_version(a)?;
    let vb = parse_version(b)?;
    Some(va.cmp(&vb))
}
