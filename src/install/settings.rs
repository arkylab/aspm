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

/// Read .claude-plugin/marketplace.json from the installed package directory.
/// Returns (marketplace_name, plugin_names).
fn read_marketplace_meta(pkg_dir: &Path) -> Result<(String, Vec<String>)> {
    let meta_path = pkg_dir.join(".claude-plugin").join("marketplace.json");
    let content = fs::read_to_string(&meta_path).with_context(|| {
        format!(
            "This does not appear to be a Claude plugin repository: \
            .claude-plugin/marketplace.json not found in '{}'. \
            Claude mode requires a valid marketplace.json.",
            pkg_dir.display()
        )
    })?;
    let meta: MarketplaceJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", meta_path.display()))?;
    let plugin_names = meta.plugins.into_iter().map(|p| p.name).collect();
    Ok((meta.name, plugin_names))
}

/// Add a package entry to settings.local.json without touching other keys.
/// Reads marketplace name and plugin names from .claude-plugin/marketplace.json inside pkg_dir.
/// Creates the file if it doesn't exist.
pub fn register_plugin(settings_path: &Path, pkg_dir: &Path, plugins_dir: &Path) -> Result<()> {
    let (marketplace_name, plugin_names) = read_marketplace_meta(pkg_dir)?;

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
pub fn unregister_plugin(settings_path: &Path, pkg_dir: &Path) -> Result<()> {
    if !settings_path.exists() {
        return Ok(());
    }

    let (marketplace_name, plugin_names) = read_marketplace_meta(pkg_dir)?;

    let mut root = read_settings(settings_path)?;

    if let Some(map) = root["extraKnownMarketplaces"].as_object_mut() {
        map.remove(&marketplace_name);
    }

    for plugin_name in &plugin_names {
        let key = format!("{}@{}", plugin_name, marketplace_name);
        if let Some(map) = root["enabledPlugins"].as_object_mut() {
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
