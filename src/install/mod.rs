//! Installation logic
//!
//! Supports two repository formats:
//! 1. aspm format: has aspub.yaml, installs based on publish config
//! 2. Claude plugin format: has skills/agents/commands/hooks/rules directories
//!
//! Supports two install modes per target directory:
//! - Plain: copies resources to <target>/<type>/<pkg>/
//! - Claude: copies package wholesale to <target>/-plugins/<pkg>/, updates settings.local.json

mod settings;

use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{AspubConfig, EffectiveMode, InstallTarget};
use crate::publish::resolve_all_publish_paths;
use crate::resolver::ResolvedDependency;

/// Resource types that can be installed from Claude plugin format
const RESOURCE_TYPES: [&str; 5] = ["skills", "agents", "commands", "hooks", "rules"];

/// Repository format detected during installation
#[derive(Debug, Clone)]
pub enum RepoFormat {
    /// aspm format with aspub.yaml
    Aspm {
        config: AspubConfig,
    },
    /// Claude plugin format with resource directories
    Plugin {
        available_types: Vec<String>,
    },
}

/// Installer for copying resources to target directories
pub struct Installer {
    target_dirs: Vec<InstallTarget>,
}

impl Installer {
    pub fn new(target_dirs: Vec<InstallTarget>) -> Self {
        Self { target_dirs }
    }

    /// Detect repository format
    fn detect_format(repo_path: &Path) -> Result<RepoFormat> {
        // Check for aspub.yaml first (priority)
        let aspub_path = repo_path.join("aspub.yaml");
        if aspub_path.exists() {
            let config = AspubConfig::load(aspub_path.to_str().unwrap())?;
            return Ok(RepoFormat::Aspm { config });
        }

        // Check for Claude plugin format (resource directories at root)
        let available_types: Vec<String> = RESOURCE_TYPES.iter()
            .filter(|&dir| repo_path.join(dir).is_dir())
            .map(|s| s.to_string())
            .collect();

        if !available_types.is_empty() {
            return Ok(RepoFormat::Plugin { available_types });
        }

        bail!(
            "Unrecognized repository format. Missing aspub.yaml or resource directories (skills, agents, commands, hooks, rules)"
        )
    }

    /// Install a single dependency to all target directories
    pub fn install(&self, dep: &ResolvedDependency) -> Result<()> {
        let repo_path = self.get_repo_path(dep)?;
        let format = Self::detect_format(&repo_path)?;

        let mut errors = Vec::new();

        for target in &self.target_dirs {
            if let Err(e) = self.install_to_target(dep, &repo_path, &format, target) {
                errors.push((target.path.clone(), e));
            }
        }

        // Print errors but continue
        for (dir, err) in &errors {
            eprintln!("  Failed to install {} to {}: {}", dep.name, dir.display(), err);
        }

        Ok(())
    }

    /// Get repository path from resolved dependency
    fn get_repo_path(&self, dep: &ResolvedDependency) -> Result<PathBuf> {
        if let Some(cache_path) = &dep.repo_cache_path {
            return Ok(cache_path.clone());
        }

        if let Some(path) = dep.source.path() {
            return Ok(path.clone());
        }

        bail!("Dependency {} has no valid source", dep.name);
    }

    /// Install a single dependency to a single target directory
    fn install_to_target(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        format: &RepoFormat,
        target: &InstallTarget,
    ) -> Result<()> {
        match target.effective_mode() {
            EffectiveMode::Plain => {
                self.install_plain(dep, repo_path, format, &target.path)
            }
            EffectiveMode::Claude => {
                self.install_claude(dep, repo_path, format, &target.path)
            }
        }
    }

    // ── Plain mode ────────────────────────────────────────────────────────────

    fn install_plain(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        format: &RepoFormat,
        target_dir: &Path,
    ) -> Result<()> {
        match format {
            RepoFormat::Aspm { config } => {
                self.install_aspm_plain(dep, repo_path, config, target_dir)
            }
            RepoFormat::Plugin { available_types } => {
                self.install_plugin_plain(dep, repo_path, available_types, target_dir)
            }
        }
    }

    fn install_aspm_plain(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        config: &AspubConfig,
        target_dir: &Path,
    ) -> Result<()> {
        let Some(publish) = &config.publish else {
            // No publish list: copy all known resource type dirs from repo root
            for resource_type in RESOURCE_TYPES {
                let src_type_dir = repo_path.join(resource_type);
                if src_type_dir.is_dir() {
                    let dst_type_dir = target_dir.join(resource_type);
                    self.copy_subdirectories(&src_type_dir, &dst_type_dir, &dep.name)?;
                    self.write_marker(&dst_type_dir.join(&dep.name))?;
                }
            }
            println!("  Installed {} -> {}", dep.name, target_dir.display());
            return Ok(());
        };

        let mut installed_count = 0;
        for (resource_type, paths) in publish {
            let items = resolve_all_publish_paths(paths, repo_path, resource_type)?;
            for item in &items {
                let dst = if item.is_dir {
                    target_dir
                        .join(resource_type)
                        .join(&dep.name)
                        .join(&item.install_name)
                } else {
                    target_dir
                        .join(resource_type)
                        .join(&dep.name)
                        .join(&item.install_name)
                };
                if item.is_dir {
                    self.copy_directory(&item.source_path, &dst)?;
                } else {
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&item.source_path, &dst)?;
                }
                installed_count += 1;
            }
        }

        // Write marker once per package (under the first resource type dir that was written)
        for resource_type in publish.keys() {
            let marker_dir = target_dir.join(resource_type).join(&dep.name);
            if marker_dir.exists() {
                self.write_marker(&marker_dir)?;
                break;
            }
        }

        println!(
            "  Installed {} items from {} to {}",
            installed_count,
            dep.name,
            target_dir.display()
        );
        Ok(())
    }

    fn install_plugin_plain(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        available_types: &[String],
        target_dir: &Path,
    ) -> Result<()> {
        let mut installed_count = 0;
        for resource_type in available_types {
            let src_dir = repo_path.join(resource_type);
            if !src_dir.is_dir() {
                continue;
            }
            let dst_type_dir = target_dir.join(resource_type);
            self.copy_subdirectories(&src_dir, &dst_type_dir, &dep.name)?;
            self.write_marker(&dst_type_dir.join(&dep.name))?;
            for _ in fs::read_dir(&src_dir)? {
                installed_count += 1;
            }
        }
        println!("  Installed {} items from {} to {}", installed_count, dep.name, target_dir.display());
        Ok(())
    }

    // ── Claude mode ───────────────────────────────────────────────────────────

    fn install_claude(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        _format: &RepoFormat,
        target_dir: &Path,
    ) -> Result<()> {
        // Always copy the repo root to <target>/-plugins/<pkg>/ (excluding .git)
        let source_root = repo_path.to_path_buf();

        let plugins_dir = target_dir.join("-plugins");
        let dst = plugins_dir.join(&dep.name);

        // Copy source_root/* -> <target>/-plugins/<pkg>/ (excluding .git)
        if dst.exists() {
            fs::remove_dir_all(&dst)?;
        }
        fs::create_dir_all(&dst)?;
        self.copy_dir_all_excluding_git(&source_root, &dst)?;
        self.write_marker(&dst)?;

        // Update settings.local.json (read marketplace name from .claude-plugin/marketplace.json)
        let settings_path = target_dir.join("settings.local.json");
        settings::register_plugin(&settings_path, &dst, &plugins_dir)?;

        println!("  Installed {} -> {} (claude mode)", dep.name, dst.display());
        Ok(())
    }

    /// Recursively copy directory contents, skipping .git
    fn copy_dir_all_excluding_git(&self, src: &Path, dst: &Path) -> Result<()> {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == ".git" {
                continue;
            }
            let src_path = entry.path();
            let dst_path = dst.join(&name);
            if entry.file_type()?.is_dir() {
                fs::create_dir_all(&dst_path)?;
                self.copy_dir_all_excluding_git(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    /// Write an empty .aspm marker file into a directory to mark it as aspm-managed
    fn write_marker(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        fs::write(dir.join(".aspm"), "")?;
        Ok(())
    }

    // ── Shared helpers ────────────────────────────────────────────────────────

    fn copy_subdirectories(&self, src: &Path, dst_type_dir: &Path, package_name: &str) -> Result<()> {
        fs::create_dir_all(dst_type_dir.join(package_name))?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let item_name = entry.file_name().to_string_lossy().to_string();
            let src_item = src.join(&item_name);
            let dst_item = dst_type_dir.join(package_name).join(&item_name);

            if src_item.is_dir() {
                self.copy_directory(&src_item, &dst_item)?;
            } else {
                fs::copy(&src_item, &dst_item)?;
            }
        }
        Ok(())
    }

    fn copy_directory(&self, src: &Path, dst: &Path) -> Result<()> {
        if !src.exists() {
            bail!("Source path does not exist: {}", src.display());
        }
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        fs::create_dir_all(dst)?;
        self.copy_dir_all(src, dst)?;
        Ok(())
    }

    fn copy_dir_all(&self, src: &Path, dst: &Path) -> Result<()> {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                fs::create_dir_all(&dst_path)?;
                self.copy_dir_all(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Remove packages that have a .aspm marker but are no longer in the dependency list
    /// Cleans both plain and claude mode files regardless of current mode
    fn prune(&self, keep: &std::collections::HashSet<String>) -> Result<()> {
        for target in &self.target_dirs {
            // Clean plain mode files (<type>/<pkg>/)
            let mut managed = std::collections::HashSet::new();
            for resource_type in RESOURCE_TYPES {
                let type_dir = target.path.join(resource_type);
                if !type_dir.is_dir() {
                    continue;
                }
                for entry in fs::read_dir(&type_dir)? {
                    let entry = entry?;
                    if entry.path().join(".aspm").exists() {
                        managed.insert(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
            for pkg in &managed {
                if !keep.contains(pkg) {
                    println!("  Pruning {}...", pkg);
                    self.remove(pkg)?;
                }
            }

            // Clean claude mode files (-plugins/<pkg>/)
            let plugins_dir = target.path.join("-plugins");
            if plugins_dir.is_dir() {
                for entry in fs::read_dir(&plugins_dir)? {
                    let entry = entry?;
                    if entry.path().join(".aspm").exists() {
                        let pkg = entry.file_name().to_string_lossy().to_string();
                        if !keep.contains(&pkg) && !managed.contains(&pkg) {
                            // Only print once if not already printed above
                            println!("  Pruning {}...", pkg);
                        }
                        self.remove(&pkg)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Install all dependencies, pruning any previously-managed packages no longer in the list
    pub fn install_all(&self, deps: &[ResolvedDependency]) -> Result<()> {
        let keep: std::collections::HashSet<String> = deps.iter().map(|d| d.name.clone()).collect();
        self.prune(&keep)?;
        println!("Installing {} dependencies...", deps.len());
        for dep in deps {
            self.install(dep)?;
        }
        Ok(())
    }

    /// Remove a package from all target directories (both plain and claude mode files)
    pub fn remove(&self, package_name: &str) -> Result<()> {
        for target in &self.target_dirs {
            // Remove plain mode files
            for resource_type in RESOURCE_TYPES {
                let package_dir = target.path.join(resource_type).join(package_name);
                if package_dir.exists() {
                    fs::remove_dir_all(&package_dir)?;
                    println!("  Removed {} from {}", package_name, package_dir.display());
                }
            }

            // Remove claude mode files
            let plugins_dir = target.path.join("-plugins");
            let package_dir = plugins_dir.join(package_name);
            if package_dir.exists() {
                // Try to unregister from settings.local.json (ignore errors if not a valid plugin)
                let settings_path = target.path.join("settings.local.json");
                if settings_path.exists() {
                    let _ = settings::unregister_plugin(&settings_path, &package_dir);
                }
                fs::remove_dir_all(&package_dir)?;
                println!("  Removed {} from {}", package_name, package_dir.display());
            }
        }
        Ok(())
    }
}
