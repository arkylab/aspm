//! Publishing logic
//!
//! TODO: This is a Phase 2 feature. Full publishing workflow will be implemented
//! when the central registry service is available. Currently only dry-run validation
//! is supported.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

use crate::config::AspubConfig;

mod matcher;
pub use matcher::*;

/// Publisher for validating and previewing package publishing
#[allow(dead_code)]
pub struct Publisher {
    config: AspubConfig,
    /// Directory containing aspub.yaml (base for relative paths)
    base_dir: PathBuf,
}

#[allow(dead_code)]
impl Publisher {
    pub fn new(config: AspubConfig) -> Self {
        Self {
            config,
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Perform a dry run (validate without publishing)
    pub fn dry_run(&self) -> Result<()> {
        self.validate()?;

        println!("Dry run for {} v{}", self.config.name, self.config.version);
        println!();

        if let Some(publish) = &self.config.publish {
            for (resource_type, paths) in publish {
                let items = resolve_all_publish_paths(paths, &self.base_dir, resource_type)?;

                println!("  {}:", resource_type);
                for item in &items {
                    println!("    - {}", item.install_name);
                }
            }
        }

        println!();
        println!("Validation passed. Publishing will be available in Phase 2.");

        Ok(())
    }

    /// Publish the package
    ///
    /// TODO: Phase 2 - Implement full publishing workflow with central registry
    pub fn publish(&self) -> Result<()> {
        self.validate()?;

        bail!(
            "Publishing is not yet implemented. This feature will be available in Phase 2 \
            with the central registry service.\n\n\
            For now, you can manually create a git tag:\n  git tag v{}\n  git push origin v{}",
            self.config.version,
            self.config.version
        );
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if self.config.name.is_empty() {
            bail!("Package name is required in aspub.yaml");
        }

        if self.config.version.is_empty() {
            bail!("Version is required in aspub.yaml");
        }

        if let Some(publish) = &self.config.publish {
            for (resource_type, paths) in publish {
                resolve_all_publish_paths(paths, &self.base_dir, resource_type)
                    .with_context(|| format!("Invalid paths in publish.{}", resource_type))?;
            }
        }

        crate::version::parse_version(&self.config.version)
            .context("Invalid version format. Use semantic versioning (e.g., 1.0.0)")?;

        Ok(())
    }
}
