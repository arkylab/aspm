//! Installation logic
//!
//! Supports three repository formats:
//! 1. aspm format: has aspub.yaml, installs based on publish config
//! 2. Claude plugin format: has skills/agents/commands/hooks/rules directories
//! 3. Single SKILL.md format: only has a SKILL.md file, auto-wraps in skills/ directory
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

use settings::PluginMeta;

/// Resource types that can be installed from Claude plugin format
const RESOURCE_TYPES: [&str; 5] = ["skills", "agents", "commands", "hooks", "rules"];

/// Directory name for Claude plugin mode installations
pub const PLUGINS_DIR: &str = "-plugins";

/// Repository format detected during installation
#[derive(Debug, Clone)]
pub enum RepoFormat {
    /// aspm format with aspub.yaml
    Aspm {
        config: AspubConfig,
        /// Whether .claude-plugin directory exists in the repo
        has_claude_plugin: bool,
    },
    /// Claude plugin format with resource directories
    Plugin {
        available_types: Vec<String>,
        /// Whether .claude-plugin directory exists in the repo
        has_claude_plugin: bool,
    },
    /// Single SKILL.md format: wrap all files in skills/{pkg}/
    SingleSkill {
        /// Whether .claude-plugin directory exists in the repo
        has_claude_plugin: bool,
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
    fn detect_format(repo_path: &Path, pkg_name: &str) -> Result<RepoFormat> {
        // Check for .claude-plugin directory (used by all formats)
        let has_claude_plugin = repo_path.join(".claude-plugin").is_dir();

        // Check for aspub.yaml first (priority)
        let aspub_path = repo_path.join("aspub.yaml");
        if aspub_path.exists() {
            let config = AspubConfig::load(aspub_path.to_str().unwrap())?;
            return Ok(RepoFormat::Aspm { config, has_claude_plugin });
        }

        // Check for Claude plugin format (resource directories at root)
        let available_types: Vec<String> = RESOURCE_TYPES.iter()
            .filter(|&dir| repo_path.join(dir).is_dir())
            .map(|s| s.to_string())
            .collect();

        if !available_types.is_empty() {
            return Ok(RepoFormat::Plugin { available_types, has_claude_plugin });
        }

        // Check for single SKILL.md format
        if Self::find_skill_md(repo_path).is_some() {
            eprintln!(
                "Warning: Package '{}' has no standard directory structure but contains SKILL.md.",
                pkg_name
            );
            eprintln!("  Auto-wrapping in skills/ directory for compatibility.");
            return Ok(RepoFormat::SingleSkill { has_claude_plugin });
        }

        bail!(
            "Package '{}' has unrecognized repository format. Missing aspub.yaml, resource directories, or SKILL.md file",
            pkg_name
        )
    }

    /// Find SKILL.md file in repository (root or subdirectories)
    fn find_skill_md(repo_path: &Path) -> Option<PathBuf> {
        // Check root level first
        let root_skill = repo_path.join("SKILL.md");
        if root_skill.exists() {
            return Some(root_skill);
        }

        // Search in subdirectories (one level deep)
        if let Ok(entries) = fs::read_dir(repo_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        return Some(skill_file);
                    }
                }
            }
        }

        None
    }

    /// Install a single dependency to all target directories
    pub fn install(&self, dep: &ResolvedDependency) -> Result<()> {
        let repo_path = self.get_repo_path(dep)?;
        let format = Self::detect_format(&repo_path, &dep.name)?;

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
            RepoFormat::Aspm { config, .. } => {
                self.install_aspm_plain(dep, repo_path, config, target_dir)
            }
            RepoFormat::Plugin { available_types, .. } => {
                self.install_plugin_plain(dep, repo_path, available_types, target_dir)
            }
            RepoFormat::SingleSkill { .. } => {
                self.install_single_skill_plain(dep, repo_path, target_dir)
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

    fn install_single_skill_plain(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        target_dir: &Path,
    ) -> Result<()> {
        // Wrap all files in skills/{package_name}/
        let dst_skill_dir = target_dir.join("skills").join(&dep.name);
        
        if dst_skill_dir.exists() {
            fs::remove_dir_all(&dst_skill_dir)?;
        }
        fs::create_dir_all(&dst_skill_dir)?;
        
        // Copy all files from repo root to skills/{package_name}/
        self.copy_dir_all_excluding_git(repo_path, &dst_skill_dir)?;
        self.write_marker(&dst_skill_dir)?;
        
        println!("  Installed {} -> {} (wrapped in skills/)", dep.name, dst_skill_dir.display());
        Ok(())
    }

    // ── Claude mode ───────────────────────────────────────────────────────────

    fn install_claude(
        &self,
        dep: &ResolvedDependency,
        repo_path: &Path,
        format: &RepoFormat,
        target_dir: &Path,
    ) -> Result<()> {
        let plugins_dir = target_dir.join(PLUGINS_DIR);
        let dst = plugins_dir.join(&dep.name);

        // Determine if we should copy entire repo or only publish items
        let (should_copy_all, has_claude_plugin, aspub_config) = self.analyze_claude_install_mode(format);

        if should_copy_all {
            // Situation 1 or 3: Copy entire repo
            self.install_claude_full(repo_path, &dst, dep, has_claude_plugin, target_dir)?;
        } else {
            // Situation 2: Copy only publish items
            self.install_claude_publish_only(repo_path, &dst, dep, aspub_config.as_ref().unwrap(), target_dir)?;
        }

        println!("  Installed {} -> {} (claude mode)", dep.name, dst.display());
        Ok(())
    }

    /// Analyze which Claude install mode to use
    /// Returns (should_copy_all, has_claude_plugin, aspub_config)
    fn analyze_claude_install_mode(&self, format: &RepoFormat) -> (bool, bool, Option<AspubConfig>) {
        match format {
            RepoFormat::Aspm { config, has_claude_plugin } => {
                // Situation 3: Has .claude-plugin -> copy all
                if *has_claude_plugin {
                    return (true, true, None);
                }
                // Situation 2: Has publish -> copy only publish
                if config.publish.is_some() && !config.publish.as_ref().unwrap().is_empty() {
                    return (false, false, Some(config.clone()));
                }
                // Situation 1: No publish -> copy all
                (true, false, None)
            }
            RepoFormat::Plugin { has_claude_plugin, .. } => {
                // Situation 3 or 1: Plugin format always copies all
                (true, *has_claude_plugin, None)
            }
            RepoFormat::SingleSkill { has_claude_plugin } => {
                // Situation 3 or 1: Single skill format always copies all
                (true, *has_claude_plugin, None)
            }
        }
    }

    /// Claude mode: Copy entire repository
    fn install_claude_full(
        &self,
        repo_path: &Path,
        dst: &Path,
        dep: &ResolvedDependency,
        has_claude_plugin: bool,
        target_dir: &Path,
    ) -> Result<()> {
        let plugins_dir = target_dir.join(PLUGINS_DIR);

        // Copy source_root/* -> dst/ (excluding .git)
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        fs::create_dir_all(dst)?;
        self.copy_dir_all_excluding_git(repo_path, dst)?;
        self.write_marker(dst)?;

        // For SingleSkill format, wrap files in skills/ directory
        if matches!(Self::detect_format(repo_path, &dep.name)?, RepoFormat::SingleSkill { .. }) {
            let skills_dir = dst.join("skills").join(&dep.name);
            fs::create_dir_all(&skills_dir)?;

            // Move all files (except .aspm, .claude-plugin and skills/) to skills/{package_name}/
            let entries: Vec<_> = fs::read_dir(dst)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name();
                    name != ".aspm" && name != "skills" && name != ".claude-plugin"
                })
                .collect();

            for entry in entries {
                let src = entry.path();
                let file_name = entry.file_name();
                let dst_path = skills_dir.join(&file_name);
                fs::rename(&src, &dst_path)?;
            }
        }

        // Prepare auto-generation metadata (only if no existing .claude-plugin)
        if !has_claude_plugin {
            let version = Self::extract_version(dep);
            let auto_meta = PluginMeta::new(dep.name.clone(), version, None, None);

            // Update settings.local.json with auto-generated metadata
            let settings_path = target_dir.join("settings.local.json");
            settings::register_plugin(&settings_path, dst, &plugins_dir, Some(&auto_meta))?;
        } else {
            // Use existing .claude-plugin, just register in settings.local.json
            let settings_path = target_dir.join("settings.local.json");
            settings::register_plugin(&settings_path, dst, &plugins_dir, None)?;
        }

        Ok(())
    }

    /// Claude mode: Copy only publish items
    fn install_claude_publish_only(
        &self,
        repo_path: &Path,
        dst: &Path,
        dep: &ResolvedDependency,
        config: &AspubConfig,
        target_dir: &Path,
    ) -> Result<()> {
        let plugins_dir = target_dir.join(PLUGINS_DIR);

        // Prepare destination
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        fs::create_dir_all(dst)?;
        self.write_marker(dst)?;

        let Some(publish) = &config.publish else {
            bail!("Expected publish config for publish-only install");
        };

        // Copy each published resource type
        for (resource_type, paths) in publish {
            let items = resolve_all_publish_paths(paths, repo_path, resource_type)?;
            for item in &items {
                let item_dst = dst.join(resource_type).join(&item.install_name);
                if item.is_dir {
                    self.copy_directory(&item.source_path, &item_dst)?;
                } else {
                    if let Some(parent) = item_dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&item.source_path, &item_dst)?;
                }
            }
        }

        // Auto-generate .claude-plugin
        let version = Self::extract_version(dep);
        let auto_meta = PluginMeta::new(dep.name.clone(), version, None, None);

        let settings_path = target_dir.join("settings.local.json");
        settings::register_plugin(&settings_path, dst, &plugins_dir, Some(&auto_meta))?;

        Ok(())
    }

    /// Extract version from resolved dependency
    fn extract_version(dep: &ResolvedDependency) -> String {
        if let Some(tag) = &dep.resolved_tag {
            // Strip 'v' prefix if present (e.g., "v1.2.3" -> "1.2.3")
            if tag.starts_with('v') && tag.len() > 1 {
                tag[1..].to_string()
            } else {
                tag.clone()
            }
        } else {
            "0.0.0".to_string()
        }
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

    /// Remove directories that have a .aspm marker
    /// Recursively scans target directories and removes any subdirectory containing .aspm file
    /// Also cleans up settings.local.json entries for non-existent paths
    /// project_root: the project root directory for resolving relative paths in settings
    pub fn prune(&self, _keep: &std::collections::HashSet<String>, project_root: &Path) -> Result<()> {
        for target in &self.target_dirs {
            self.prune_directory(&target.path)?;
            
            // Clean up settings.local.json after pruning directories
            let settings_path = target.path.join("settings.local.json");
            if settings_path.exists() {
                settings::cleanup_settings(&settings_path, project_root)?;
            }
        }
        Ok(())
    }

    /// Recursively prune a directory, removing any subdirectory with .aspm marker
    fn prune_directory(&self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Check if this directory itself has a .aspm marker
        let marker_path = dir.join(".aspm");
        if marker_path.exists() {
            println!("  Pruning {}...", dir.display());
            fs::remove_dir_all(dir)?;
            return Ok(()); // Directory removed, no need to scan further
        }

        // Recursively scan subdirectories
        let entries: Vec<_> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        for entry in entries {
            self.prune_directory(&entry.path())?;
        }

        Ok(())
    }

    /// Install all dependencies, pruning any previously-managed packages no longer in the list
    /// project_root: the project root directory for resolving relative paths in settings
    #[allow(dead_code)]
    pub fn install_all(&self, deps: &[ResolvedDependency], project_root: &Path) -> Result<()> {
        let keep: std::collections::HashSet<String> = deps.iter().map(|d| d.name.clone()).collect();
        self.prune(&keep, project_root)?;
        println!("Installing {} dependencies...", deps.len());
        for dep in deps {
            self.install(dep)?;
        }
        Ok(())
    }

    /// Remove a package from all target directories (both plain and claude mode files)
    #[allow(dead_code)]
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
            let plugins_dir = target.path.join(PLUGINS_DIR);
            let package_dir = plugins_dir.join(package_name);
            if package_dir.exists() {
                // Try to unregister from settings.local.json (ignore errors if not a valid plugin)
                let settings_path = target.path.join("settings.local.json");
                if settings_path.exists() {
                    let _ = settings::unregister_plugin(
                        &settings_path,
                        &package_dir,
                        package_name,
                    );
                }
                fs::remove_dir_all(&package_dir)?;
                println!("  Removed {} from {}", package_name, package_dir.display());
            }
        }
        Ok(())
    }
}
