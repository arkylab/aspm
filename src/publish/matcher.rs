//! Publish item matching with regex support

use anyhow::{bail, Result};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Regex metacharacters that indicate a segment is a regex pattern
const REGEX_CHARS: &str = "^$.*+?[](){}|\\";

/// Check if string is a regex pattern
pub fn is_regex_pattern(s: &str) -> bool {
    s.chars().any(|c| REGEX_CHARS.contains(c))
}

/// A resolved publish item (after path expansion)
#[derive(Debug, Clone)]
pub struct ResolvedItem {
    /// Actual filesystem path of the source
    pub source_path: PathBuf,
    /// Name used for installation (last segment of matched path)
    pub install_name: String,
    /// Whether the source is a directory
    pub is_dir: bool,
}

/// Determine if a raw path string is absolute.
/// Supports Unix (`/foo`) and Windows (`C:/foo`) absolute paths.
fn is_absolute_path(raw: &str) -> bool {
    if raw.starts_with('/') {
        return true;
    }
    // Windows drive letter: C:/...
    let bytes = raw.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'/'
}

/// Resolve a publish path string against a base directory.
///
/// Rules:
/// - Only `/` is treated as path separator; `\` may appear inside a regex segment.
/// - Absolute paths: start with `/` (Unix) or `[A-Za-z]:/` (Windows).
/// - Relative paths: resolved relative to `base_dir`.
/// - Each segment may be a regex pattern (detected by presence of regex metacharacters).
/// - Trailing `/` → match directories; no trailing `/` → match files.
pub fn resolve_publish_path(raw: &str, base_dir: &Path) -> Result<Vec<ResolvedItem>> {
    if raw.is_empty() {
        bail!("Empty publish path");
    }

    let match_dirs = raw.ends_with('/');

    let segments: Vec<&str> = raw.split('/').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        bail!("Publish path '{}' has no segments", raw);
    }

    // Determine the starting path and which segments to walk
    let (start, effective_segments): (PathBuf, &[&str]) = if is_absolute_path(raw) {
        if raw.starts_with('/') {
            (PathBuf::from("/"), &segments)
        } else {
            // Windows: first segment is "C:", root is "C:/"
            (PathBuf::from(format!("{}/", segments[0])), &segments[1..])
        }
    } else {
        (base_dir.to_path_buf(), &segments)
    };

    let mut current_paths: Vec<PathBuf> = vec![start];
    let last_idx = effective_segments.len().saturating_sub(1);

    for (i, segment) in effective_segments.iter().enumerate() {
        let is_last = i == last_idx;
        let mut next_paths: Vec<PathBuf> = Vec::new();

        for current in &current_paths {
            if is_regex_pattern(segment) {
                let re = Regex::new(segment).map_err(|e| {
                    anyhow::anyhow!("Invalid regex '{}' in path '{}': {}", segment, raw, e)
                })?;

                if !current.is_dir() {
                    continue;
                }

                for entry in std::fs::read_dir(current)? {
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_string();
                    if re.is_match(&name) {
                        let path = current.join(&name);
                        if is_last {
                            if match_dirs && path.is_dir() {
                                next_paths.push(path);
                            } else if !match_dirs && path.is_file() {
                                next_paths.push(path);
                            }
                        } else if path.is_dir() {
                            next_paths.push(path);
                        }
                    }
                }
            } else {
                let path = current.join(segment);
                if is_last {
                    if match_dirs && path.is_dir() {
                        next_paths.push(path);
                    } else if !match_dirs && path.is_file() {
                        next_paths.push(path);
                    } else if path.exists() {
                        next_paths.push(path);
                    }
                } else if path.is_dir() {
                    next_paths.push(path);
                }
            }
        }

        current_paths = next_paths;
    }

    let results = current_paths
        .into_iter()
        .map(|p| {
            let install_name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let is_dir = p.is_dir();
            ResolvedItem {
                source_path: p,
                install_name,
                is_dir,
            }
        })
        .collect();

    Ok(results)
}

/// Resolve all publish paths for a resource type and return expanded list
pub fn resolve_all_publish_paths(
    paths: &[String],
    base_dir: &Path,
    resource_type: &str,
) -> Result<Vec<ResolvedItem>> {
    let mut all_items: Vec<ResolvedItem> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for raw in paths {
        let items = resolve_publish_path(raw, base_dir)?;
        if items.is_empty() {
            if raw.ends_with('/') {
                eprintln!(
                    "Warning: Path '{}' matched no directories for {}. Did publisher mean to match files? Remove trailing '/'.",
                    raw, resource_type
                );
            } else {
                eprintln!(
                    "Warning: Path '{}' matched no files for {}. Did publisher mean to match directories? Use trailing '/'.",
                    raw, resource_type
                );
            }
        }
        for item in items {
            if seen.insert(item.source_path.clone()) {
                all_items.push(item);
            }
        }
    }

    Ok(all_items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_regex_pattern() {
        assert!(is_regex_pattern("^com-.*"));
        assert!(is_regex_pattern("com-*"));
        assert!(is_regex_pattern("test+"));
        assert!(!is_regex_pattern("simple-name"));
        assert!(!is_regex_pattern("com-brainstorming"));
    }

    #[test]
    fn test_is_absolute_path() {
        assert!(is_absolute_path("/home/user/skills/"));
        assert!(is_absolute_path("C:/Users/skills/"));
        assert!(is_absolute_path("c:/users/skills/"));
        assert!(!is_absolute_path(".aspm/skills/"));
        assert!(!is_absolute_path("relative/path/"));
    }
}
