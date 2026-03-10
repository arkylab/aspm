//! settings.local.json read/write helpers for Claude install mode

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Minimal structure for .claude-plugin/marketplace.json
#[derive(Deserialize)]
struct MarketplaceJson {
    name: String,
    plugins: Vec<MarketplacePlugin>,
}

#[derive(Deserialize)]
struct MarketplacePlugin {
    name: String,
}

/// Metadata for auto-generating Claude plugin files
pub struct PluginMeta {
    pub package_name: String,
    pub version: String,
    pub owner_name: Option<String>,
    pub owner_email: Option<String>,
}

impl PluginMeta {
    /// Create plugin meta from dependency info
    pub fn new(
        package_name: String,
        version: String,
        owner_name: Option<String>,
        owner_email: Option<String>,
    ) -> Self {
        Self {
            package_name,
            version,
            owner_name,
            owner_email,
        }
    }
}

/// Read .claude-plugin/marketplace.json from the installed package directory.
/// Returns Some((marketplace_name, plugin_names)) if file exists.
/// Returns None if file does not exist (caller should auto-generate).
fn read_marketplace_meta(pkg_dir: &Path) -> Result<Option<(String, Vec<String>)>> {
    let meta_path = pkg_dir.join(".claude-plugin").join("marketplace.json");
    
    if !meta_path.exists() {
        return Ok(None);
    }
    
    let content = fs::read_to_string(&meta_path)
        .with_context(|| format!("Failed to read {}", meta_path.display()))?;
    let meta: MarketplaceJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", meta_path.display()))?;
    let plugin_names = meta.plugins.into_iter().map(|p| p.name).collect();
    Ok(Some((meta.name, plugin_names)))
}

/// Generate .claude-plugin/marketplace.json and plugin.json files
fn generate_plugin_meta(pkg_dir: &Path, meta: &PluginMeta) -> Result<()> {
    let plugin_dir = pkg_dir.join(".claude-plugin");
    fs::create_dir_all(&plugin_dir)?;
    
    let description = format!("Auto-generated for {} by aspm", meta.package_name);
    let marketplace_name = format!("{}-dev", meta.package_name);
    
    let owner_name = meta.owner_name.as_deref().unwrap_or("unknown");
    let owner_email = meta.owner_email.as_deref().unwrap_or("unknown@unknown.unknown");
    
    // Generate marketplace.json
    let marketplace = serde_json::json!({
        "name": &marketplace_name,
        "description": &description,
        "owner": {
            "name": owner_name,
            "email": owner_email
        },
        "plugins": [
            {
                "name": &meta.package_name,
                "description": &description,
                "version": &meta.version,
                "source": "./"
            }
        ]
    });
    
    // Generate plugin.json
    let plugin = serde_json::json!({
        "name": &meta.package_name,
        "description": &description,
        "version": &meta.version,
        "author": {
            "name": owner_name,
            "email": owner_email
        }
    });
    
    // Write files
    let marketplace_path = plugin_dir.join("marketplace.json");
    let plugin_path = plugin_dir.join("plugin.json");
    
    fs::write(&marketplace_path, serde_json::to_string_pretty(&marketplace)?)?;
    fs::write(&plugin_path, serde_json::to_string_pretty(&plugin)?)?;
    
    Ok(())
}

/// Add a package entry to settings.local.json without touching other keys.
/// Reads marketplace name and plugin names from .claude-plugin/marketplace.json inside pkg_dir.
/// Creates the file if it doesn't exist.
/// If marketplace.json is missing and auto_meta is provided, generates it automatically.
pub fn register_plugin(
    settings_path: &Path,
    pkg_dir: &Path,
    plugins_dir: &Path,
    auto_meta: Option<&PluginMeta>,
) -> Result<()> {
    let (marketplace_name, plugin_names) = match read_marketplace_meta(pkg_dir)? {
        Some(meta) => meta,
        None => {
            // Try to auto-generate if metadata provided
            if let Some(meta) = auto_meta {
                eprintln!(
                    "Warning: Package '{}' does not have .claude-plugin/marketplace.json.",
                    meta.package_name
                );
                eprintln!("  Auto-generating Claude plugin metadata for compatibility.");
                generate_plugin_meta(pkg_dir, meta)?;
                read_marketplace_meta(pkg_dir)?
                    .expect("Generated metadata should be readable")
            } else {
                anyhow::bail!(
                    "Package does not have .claude-plugin/marketplace.json and no auto-generation metadata provided"
                );
            }
        }
    };

    let mut root = read_settings(settings_path)?;

    // extraKnownMarketplaces.<marketplace_name> = { source: { source: "directory", path: "<plugins_dir>/<pkg_dir_name>" } }
    let rel_path = plugins_dir.join(pkg_dir.file_name().unwrap_or_default());
    let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");

    root["extraKnownMarketplaces"][&marketplace_name] = serde_json::json!({
        "source": {
            "source": "directory",
            "path": rel_path_str
        }
    });

    // enabledPlugins.<plugin_name>@<marketplace_name> = true
    for plugin_name in &plugin_names {
        let key = format!("{}@{}", plugin_name, marketplace_name);
        root["enabledPlugins"][key] = Value::Bool(true);
    }

    write_settings(settings_path, &root)
}

/// Remove a package entry from settings.local.json without touching other keys.
/// Reads marketplace name and plugin names from .claude-plugin/marketplace.json inside pkg_dir.
/// If marketplace.json is missing, uses package_name as fallback.
#[allow(dead_code)]
pub fn unregister_plugin(
    settings_path: &Path,
    pkg_dir: &Path,
    package_name: &str,
) -> Result<()> {
    if !settings_path.exists() {
        return Ok(());
    }

    let (marketplace_name, plugin_names) = match read_marketplace_meta(pkg_dir)? {
        Some(meta) => meta,
        None => {
            // Fallback to package_name when marketplace.json is missing
            (package_name.to_string(), vec![package_name.to_string()])
        }
    };

    let mut root = read_settings(settings_path)?;

    // Remove the marketplace entry
    if let Some(map) = root["extraKnownMarketplaces"].as_object_mut() {
        map.remove(&marketplace_name);
    }

    // Remove enabled plugins
    for plugin_name in &plugin_names {
        let key = format!("{}@{}", plugin_name, marketplace_name);
        if let Some(map) = root["enabledPlugins"].as_object_mut() {
            map.remove(&key);
        }
    }

    write_settings(settings_path, &root)
}

/// Clean up settings.local.json by removing entries whose paths no longer exist.
/// Paths in extraKnownMarketplaces are relative to base_dir.
/// Only processes entries whose path contains PLUGINS_DIR to avoid accidentally removing unrelated entries.
pub fn cleanup_settings(settings_path: &Path, base_dir: &Path) -> Result<()> {
    if !settings_path.exists() {
        return Ok(());
    }

    let mut root = read_settings(settings_path)?;

    let mut to_remove = Vec::new();

    // Find entries whose paths don't exist
    if let Some(map) = root["extraKnownMarketplaces"].as_object() {
        for (marketplace_name, entry) in map {
            if let Some(path_str) = entry["source"]["path"].as_str() {
                // Skip if path doesn't contain PLUGINS_DIR (not managed by aspm)
                if !path_str.contains(super::PLUGINS_DIR) {
                    continue;
                }

                // Path is relative to base_dir
                let path = base_dir.join(path_str);
                if !path.exists() {
                    to_remove.push(marketplace_name.clone());
                }
            }
        }
    }

    if to_remove.is_empty() {
        return Ok(());
    }

    // Remove marketplace entries
    if let Some(map) = root["extraKnownMarketplaces"].as_object_mut() {
        for marketplace_name in &to_remove {
            map.remove(marketplace_name);
        }
    }

    // Remove corresponding enabled plugins
    if let Some(map) = root["enabledPlugins"].as_object_mut() {
        // Collect all keys to remove
        let keys_to_remove: Vec<String> = map.keys()
            .filter(|key| {
                // key format: plugin_name@marketplace_name
                key.split('@')
                    .nth(1)
                    .map(|marketplace| to_remove.contains(&marketplace.to_string()))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        
        for key in keys_to_remove {
            println!("  Cleaning up stale entry: {}", key);
            map.remove(&key);
        }
    }

    write_settings(settings_path, &root)
}

fn read_settings(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&content)?;
    Ok(value)
}

fn write_settings(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, content)?;
    Ok(())
}
