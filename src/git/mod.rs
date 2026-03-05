//! Git operations for cloning and checking out repositories

use anyhow::{Context, Result};
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::env;
use std::process::Command;

/// Git manager for handling repository operations
pub struct GitManager {
    cache_dir: PathBuf,
    /// In-memory cache of URLs that have been fetched in this session
    fetched_urls: RefCell<HashSet<String>>,
}

impl GitManager {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("aspm");
        
        std::fs::create_dir_all(&cache_dir)?;
        
        Ok(Self { 
            cache_dir,
            fetched_urls: RefCell::new(HashSet::new()),
        })
    }
    
    /// Get the cache directory for a repository
    fn get_repo_cache_path(&self, url: &str) -> PathBuf {
        let hash = hex::encode(Sha256::digest(url.as_bytes()));
        self.cache_dir.join("repos").join(&hash[..16])
    }
    
    /// Get the cache path for a given URL (public interface)
    pub fn get_cache_path(&self, url: &str) -> PathBuf {
        self.get_repo_cache_path(url)
    }
    
    /// Create authentication callbacks for git operations
    fn create_auth_callbacks<'a>() -> RemoteCallbacks<'a> {
        let mut callbacks = RemoteCallbacks::new();
        
        callbacks.credentials(move |url, username_from_url, allowed_types| {
            // Determine auth method based on URL scheme
            let is_https = url.starts_with("https://") || url.starts_with("http://");
            
            if is_https {
                // For HTTPS URLs, only try HTTPS auth
                if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                    // Check for token environment variables
                    if let Ok(token) = env::var("GIT_TOKEN")
                        .or_else(|_| env::var("GITHUB_TOKEN"))
                    {
                        if let Ok(cred) = Cred::userpass_plaintext(&token, "") {
                            return Ok(cred);
                        }
                    }
                }
            } else {
                // For SSH URLs, try SSH key authentication
                if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                    let username = username_from_url.unwrap_or("git");
                    
                    // Try default SSH key paths
                    if let Some(home) = dirs::home_dir() {
                        let ssh_dir = home.join(".ssh");
                        for key_name in &["id_ed25519", "id_rsa", "id_ecdsa"] {
                            let key_path = ssh_dir.join(key_name);
                            if key_path.exists() {
                                if let Ok(cred) = Cred::ssh_key(username, None, &key_path, None) {
                                    return Ok(cred);
                                }
                            }
                        }
                    }
                }
            }
            
            Err(git2::Error::from_str("No valid credentials available"))
        });
        
        callbacks
    }
    
    /// Create fetch options with authentication
    fn create_fetch_options<'a>() -> FetchOptions<'a> {
        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(Self::create_auth_callbacks());
        fetch_options
    }
    
    /// Clone or open a repository
    pub fn clone_or_open(&self, url: &str) -> Result<Repository> {
        let cache_path = self.get_repo_cache_path(url);
        
        // Check in-memory cache first - if already fetched successfully this session, just open
        if self.fetched_urls.borrow().contains(url) && cache_path.exists() {
            if let Ok(repo) = Repository::open(&cache_path) {
                return Ok(repo);
            }
        }
        
        if cache_path.exists() {
            // Try to open existing repository
            if let Ok(repo) = Repository::open(&cache_path) {
                // Reset any local changes before fetching (in case of accidental modifications)
                self.reset_to_head(&repo)?;
                
                // Try to fetch latest with git2 first, fallback to git command
                let fetch_ok = match Self::fetch_all(&repo) {
                    Ok(()) => true,
                    Err(_) => self.fetch_with_git_command(&repo).is_ok(),
                };
                
                if fetch_ok {
                    self.fetched_urls.borrow_mut().insert(url.to_string());
                }
                return Ok(repo);
            }
            // Only remove cache if opening failed (corrupted)
            std::fs::remove_dir_all(&cache_path)
                .context("Failed to remove corrupted cache directory")?;
        }
        
        // Try git2 first
        let fetch_options = Self::create_fetch_options();
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_options);
        
        if let Ok(repo) = builder.clone(url, &cache_path) {
            self.fetched_urls.borrow_mut().insert(url.to_string());
            return Ok(repo);
        }
        
        // Fallback to system git command (better SSH support on Windows)
        self.clone_with_git_command(url, &cache_path)?;
        
        let repo = Repository::open(&cache_path).context("Failed to open cloned repository")?;
        self.fetched_urls.borrow_mut().insert(url.to_string());
        Ok(repo)
    }
    
    /// Reset working directory to HEAD (discard all local changes)
    fn reset_to_head(&self, repo: &Repository) -> Result<()> {
        use git2::{ResetType, build::CheckoutBuilder};
        
        // Hard reset to HEAD - discards all staged and unstaged changes
        let head = repo.head()
            .context("Failed to get HEAD reference")?;
        let target = head.target()
            .context("HEAD has no target")?;
        let commit = repo.find_commit(target)?;
        
        let mut checkout = CheckoutBuilder::new();
        checkout.force(); // Allow overwriting modified files
        
        repo.reset(commit.as_object(), ResetType::Hard, Some(&mut checkout))
            .context("Failed to reset to HEAD")?;
        
        Ok(())
    }
    
    /// Fetch using system git command (fallback for SSH on Windows)
    fn fetch_with_git_command(&self, repo: &Repository) -> Result<()> {
        let path = repo.workdir()
            .context("Repository has no working directory")?;
        
        let status = Command::new("git")
            .args(["fetch", "--all"])
            .current_dir(path)
            .status()
            .context("Failed to execute git fetch command")?;
        
        if !status.success() {
            anyhow::bail!("git fetch failed with status: {}", status);
        }
        
        Ok(())
    }
    
    /// Clone using system git command (fallback for SSH on Windows)
    fn clone_with_git_command(&self, url: &str, path: &PathBuf) -> Result<()> {
        let status = Command::new("git")
            .args(["clone", url, path.to_str().unwrap()])
            .status()
            .context("Failed to execute git command. Make sure git is installed and in PATH")?;
        
        if !status.success() {
            anyhow::bail!("git clone failed with status: {}", status);
        }
        
        Ok(())
    }
    
    /// Fetch all remotes
    fn fetch_all(repo: &Repository) -> Result<()> {
        let mut fetch_options = Self::create_fetch_options();
        
        for remote in repo.remotes()?.iter().flatten() {
            repo.find_remote(remote)?
                .fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
        }
        Ok(())
    }
    
    /// Checkout to a specific tag
    pub fn checkout_tag(repo: &Repository, tag: &str) -> Result<()> {
        let tag_name = if tag.starts_with("refs/tags/") {
            tag.to_string()
        } else {
            format!("refs/tags/{}", tag)
        };
        
        let reference = repo.find_reference(&tag_name)
            .context(format!("Tag '{}' not found", tag))?;
        
        let commit = reference.peel_to_commit()?;
        repo.checkout_tree(commit.as_object(), None)?;
        repo.set_head_detached(commit.id())?;
        
        Ok(())
    }
    
    /// Checkout to a specific branch
    pub fn checkout_branch(repo: &Repository, branch: &str) -> Result<()> {
        let branch_name = if branch.starts_with("refs/heads/") {
            branch.to_string()
        } else {
            format!("refs/remotes/origin/{}", branch)
        };
        
        let reference = repo.find_reference(&branch_name)
            .context(format!("Branch '{}' not found", branch))?;
        
        let commit = reference.peel_to_commit()?;
        repo.checkout_tree(commit.as_object(), None)?;
        repo.set_head_detached(commit.id())?;
        
        Ok(())
    }
    
    /// Checkout to a specific commit
    pub fn checkout_commit(repo: &Repository, commit_hash: &str) -> Result<()> {
        let oid = git2::Oid::from_str(commit_hash)
            .context("Invalid commit hash")?;
        
        let commit = repo.find_commit(oid)
            .context(format!("Commit '{}' not found", commit_hash))?;
        
        repo.checkout_tree(commit.as_object(), None)?;
        repo.set_head_detached(oid)?;
        
        Ok(())
    }
    
    /// Get current HEAD commit hash
    pub fn get_head_commit(repo: &Repository) -> Result<String> {
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;
        Ok(commit.id().to_string())
    }
    
    /// List all tags in the repository
    pub fn list_tags(repo: &Repository) -> Result<Vec<String>> {
        let mut tags = Vec::new();
        
        // Use references to find tags
        for reference in repo.references()? {
            let reference = reference?;
            let name = reference.name();
            if let Some(name) = name {
                if let Some(tag_name) = name.strip_prefix("refs/tags/") {
                    tags.push(tag_name.to_string());
                }
            }
        }
        
        Ok(tags)
    }
    
    /// List all branches in the repository
    pub fn list_branches(repo: &Repository) -> Result<Vec<String>> {
        let mut branches = Vec::new();
        for branch in repo.branches(Some(git2::BranchType::Remote))? {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                // Remove "origin/" prefix
                if let Some(local_name) = name.strip_prefix("origin/") {
                    if local_name != "HEAD" {
                        branches.push(local_name.to_string());
                    }
                }
            }
        }
        Ok(branches)
    }
    
    /// Get all version-like refs (tags and branches)
    pub fn get_version_refs(repo: &Repository) -> Result<Vec<VersionRef>> {
        let mut refs = Vec::new();
        
        // Get tags
        for tag in Self::list_tags(repo)? {
            refs.push(VersionRef {
                name: tag.clone(),
                ref_type: RefType::Tag,
                version: crate::version::parse_version(&tag),
            });
        }
        
        // Get branches
        for branch in Self::list_branches(repo)? {
            refs.push(VersionRef {
                name: branch.clone(),
                ref_type: RefType::Branch,
                version: crate::version::parse_version(&branch),
            });
        }
        
        Ok(refs)
    }
    
    /// Create a git tag
    #[allow(dead_code)]
    pub fn create_tag(repo: &Repository, tag: &str, message: &str) -> Result<()> {
        let sig = repo.signature()?;
        let head = repo.head()?;
        let target = head.peel_to_commit()?;
        
        repo.tag(tag, target.as_object(), &sig, message, false)?;
        
        Ok(())
    }
    
    /// Push to remote
    #[allow(dead_code)]
    pub fn push(repo: &Repository, remote: &str) -> Result<()> {
        let mut remote = repo.find_remote(remote)?;
        
        let head = repo.head()?;
        let refname = head.name().context("HEAD has no name")?;
        
        remote.push(&[refname], None)?;
        
        // Push tags
        let tags = Self::list_tags(repo)?;
        for tag in tags {
            let ref_spec = format!("refs/tags/{}:refs/tags/{}", tag, tag);
            remote.push(&[&ref_spec], None)?;
        }
        
        Ok(())
    }
    
    /// Get remote URL
    #[allow(dead_code)]
    pub fn get_remote_url(repo: &Repository) -> Result<Option<String>> {
        let remote = repo.find_remote("origin")?;
        Ok(remote.url().map(|s| s.to_string()))
    }
}

impl Default for GitManager {
    fn default() -> Self {
        Self::new().expect("Failed to create GitManager")
    }
}

/// A version reference (tag or branch)
#[derive(Debug, Clone)]
pub struct VersionRef {
    pub name: String,
    pub ref_type: RefType,
    pub version: Option<crate::version::SemanticVersion>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RefType {
    Tag,
    Branch,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_git_manager_creation() {
        let manager = GitManager::new();
        assert!(manager.is_ok());
    }
}
