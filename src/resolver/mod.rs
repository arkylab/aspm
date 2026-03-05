//! Dependency resolution

use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::{AspubConfig, DependencySource};
use crate::git::{GitManager, RefType, VersionRef};

/// Dependency graph for managing resolved dependencies
struct DependencyGraph {
    /// Map from dependency name to resolved dependency
    nodes: HashMap<String, ResolvedDependency>,
    /// Adjacency list: dependency name -> list of dependencies it depends on
    edges: HashMap<String, Vec<String>>,
    /// Reverse adjacency list: dependency name -> list of nodes that depend on it
    reverse_edges: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }
    
    /// Add a node to the graph
    fn add_node(&mut self, dep: ResolvedDependency) {
        let name = dep.name.clone();
        
        // Get list of dependencies
        let deps: Vec<String> = dep.dependencies.keys().cloned().collect();
        
        // Add edges
        self.edges.insert(name.clone(), deps.clone());
        
        // Add reverse edges
        for d in &deps {
            self.reverse_edges.entry(d.clone()).or_default().push(name.clone());
        }
        
        // Add node
        self.nodes.insert(name, dep);
    }
    
    /// Remove a node and all its edges
    fn remove_node(&mut self, name: &str) {
        // Remove reverse edges from dependencies
        if let Some(deps) = self.edges.get(name) {
            for d in deps {
                if let Some(reverse) = self.reverse_edges.get_mut(d) {
                    reverse.retain(|n| n != name);
                }
            }
        }
        
        // Remove edges
        self.edges.remove(name);
        
        // Remove reverse edges pointing to this node
        self.reverse_edges.remove(name);
        
        // Remove node
        self.nodes.remove(name);
    }
    
    /// Find nodes that depend on the given node
    #[allow(dead_code)]
    fn find_dependents(&self, name: &str) -> Vec<String> {
        self.reverse_edges.get(name).cloned().unwrap_or_default()
    }
    
    /// Remove all edges from a node (but keep the node)
    fn remove_edges_from(&mut self, name: &str) {
        // Remove reverse edges from dependencies
        if let Some(deps) = self.edges.get(name) {
            for d in deps {
                if let Some(reverse) = self.reverse_edges.get_mut(d) {
                    reverse.retain(|n| n != name);
                }
            }
        }
        // Clear edges
        self.edges.remove(name);
    }
    
    /// Add a virtual root node that points to all nodes (to prevent deleting roots)
    fn add_virtual_root(&mut self) {
        let root_name = "__ROOT__".to_string();
        for name in self.nodes.keys() {
            self.reverse_edges.entry(name.clone()).or_default().push(root_name.clone());
        }
        self.edges.insert(root_name, self.nodes.keys().cloned().collect());
    }
    
    /// Check if any version has changed in the graph
    #[allow(dead_code)]
    fn has_version_change(&self, name: &str, new_source: &DependencySource) -> bool {
        if let Some(existing) = self.nodes.get(name) {
            return &existing.source != new_source;
        }
        false
    }
    
    /// Get all nodes with in-degree 0 (not depended upon by any other node)
    fn get_nodes_with_in_degree_zero(&self) -> Vec<String> {
        let mut result = Vec::new();
        for name in self.nodes.keys() {
            // Check if this node has any incoming edges (reverse_edges)
            // If reverse_edges doesn't contain this node, or the list is empty, it's in-degree 0
            let in_degree = self.reverse_edges.get(name).map(|v| v.len()).unwrap_or(0);
            if in_degree == 0 {
                result.push(name.clone());
            }
        }
        result
    }
    
    /// Prune nodes with in-degree 0 (not depended upon by any other node)
    fn prune_unreachable(&mut self) {
        loop {
            let in_degree_zero = self.get_nodes_with_in_degree_zero();
            if in_degree_zero.is_empty() {
                break;
            }
            for name in in_degree_zero {
                self.remove_node(&name);
            }
        }
    }
    
    /// Get all resolved dependencies
    fn get_resolved(&self) -> Vec<ResolvedDependency> {
        self.nodes.values().cloned().collect()
    }
}

/// Resolved dependency
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedDependency {
    pub name: String,
    pub source: DependencySource,
    pub resolved_tag: Option<String>,
    pub resolved_branch: Option<String>,
    pub resolved_commit: Option<String>,
    pub git_url: Option<String>,
    /// Dependencies of this dependency (for recursive resolution)
    pub dependencies: HashMap<String, DependencySource>,
    /// Local cache path of the cloned repository
    pub repo_cache_path: Option<PathBuf>,
}

/// Dependency resolver
pub struct DependencyResolver {
    git_manager: GitManager,
}

impl DependencyResolver {
    pub fn new() -> Result<Self> {
        Ok(Self {
            git_manager: GitManager::new()?,
        })
    }
    
    /// Resolve a single dependency
    pub fn resolve(&self, name: &str, source: Option<&DependencySource>) -> Result<ResolvedDependency> {
        let source = source.cloned().unwrap_or_else(|| {
            DependencySource::Simple("0.0.0".to_string())
        });
        
        match &source {
            DependencySource::Simple(version) => {
                // Simple version string, need to find from index or default repo
                // For now, this is not supported in phase 1
                bail!("Simple version '{}' requires a git source. Use --git option.", version);
            }
            DependencySource::Detailed { git, version, tag, branch, commit, path } => {
                if let Some(p) = path {
                    // Local path dependency
                    return Ok(ResolvedDependency {
                        name: name.to_string(),
                        source: source.clone(),
                        resolved_tag: None,
                        resolved_branch: None,
                        resolved_commit: None,
                        git_url: None,
                        dependencies: HashMap::new(),
                        repo_cache_path: Some(PathBuf::from(p)),
                    });
                }
                
                let git_url = git.clone().unwrap_or_default();
                let repo_cache_path = self.git_manager.get_cache_path(&git_url);
                let repo = self.git_manager.clone_or_open(&git_url)?;
                
                // Determine which ref to use
                let (resolved_tag, resolved_branch, resolved_commit) = if let Some(t) = tag {
                    GitManager::checkout_tag(&repo, t)?;
                    let commit = GitManager::get_head_commit(&repo)?;
                    (Some(t.clone()), None, Some(commit))
                } else if let Some(b) = branch {
                    GitManager::checkout_branch(&repo, b)?;
                    let commit = GitManager::get_head_commit(&repo)?;
                    (None, Some(b.clone()), Some(commit))
                } else if let Some(c) = commit {
                    GitManager::checkout_commit(&repo, c)?;
                    (None, None, Some(c.clone()))
                } else if let Some(v) = version {
                    // Find best matching version
                    let best = self.find_best_version(&repo, v)?;
                    let commit = match best.ref_type {
                        RefType::Tag => {
                            GitManager::checkout_tag(&repo, &best.name)?;
                            GitManager::get_head_commit(&repo)?
                        }
                        RefType::Branch => {
                            GitManager::checkout_branch(&repo, &best.name)?;
                            GitManager::get_head_commit(&repo)?
                        }
                    };
                    match best.ref_type {
                        RefType::Tag => (Some(best.name), None, Some(commit)),
                        RefType::Branch => (None, Some(best.name), Some(commit)),
                    }
                } else {
                    // No specific ref, use default branch
                    let head = GitManager::get_head_commit(&repo)?;
                    (None, None, Some(head))
                };
                
                // Read aspub.yaml to get transitive dependencies
                let dependencies = self.get_transitive_dependencies(&repo)?;
                
                Ok(ResolvedDependency {
                    name: name.to_string(),
                    source: source.clone(),
                    resolved_tag,
                    resolved_branch,
                    resolved_commit,
                    git_url: Some(git_url),
                    dependencies,
                    repo_cache_path: Some(repo_cache_path),
                })
            }
        }
    }
    
    /// Get transitive dependencies from aspub.yaml
    fn get_transitive_dependencies(&self, repo: &git2::Repository) -> Result<HashMap<String, DependencySource>> {
        let repo_path = repo.workdir()
            .context("Repository has no working directory")?;
        
        let aspub_path = repo_path.join("aspub.yaml");
        if !aspub_path.exists() {
            return Ok(HashMap::new());
        }
        
        let config = AspubConfig::load(aspub_path.to_str().unwrap())?;
        Ok(config.dependencies)
    }
    
    /// Resolve all dependencies (non-recursive)
    #[allow(dead_code)]
    pub fn resolve_all(&self, dependencies: &HashMap<String, DependencySource>) -> Result<Vec<ResolvedDependency>> {
        let mut resolved = Vec::new();
        
        for (name, source) in dependencies {
            let dep = self.resolve(name, Some(source))?;
            resolved.push(dep);
        }
        
        Ok(resolved)
    }
    
    /// Resolve all dependencies recursively with conflict resolution
    pub fn resolve_all_recursive(&self, dependencies: &HashMap<String, DependencySource>) -> Result<Vec<ResolvedDependency>> {
        // Step 1: Collect all dependencies with their sources (collect conflicts)
        let mut all_sources: HashMap<String, Vec<DependencySource>> = HashMap::new();
        self.collect_all_sources(dependencies, &mut all_sources, &mut HashSet::new())?;
        
        // Step 2: Resolve conflicts - keep only one source per dependency
        let resolved_sources = self.resolve_all_conflicts(all_sources)?;
        
        // Step 3: Build dependency graph
        let mut graph = DependencyGraph::new();
        
        for (name, source) in &resolved_sources {
            // Resolve each dependency and add to graph
            let dep = self.resolve(name, Some(source))?;
            graph.add_node(dep);
        }
        
        // Step 4: Iterative upgrade - handle version changes
        loop {
            let mut changed = false;
            
            // Check each node if its dependencies changed
            let nodes: Vec<String> = graph.nodes.keys().cloned().collect();
            for name in nodes {
                let dep = graph.nodes.get(&name).cloned().unwrap();
                
                // Re-resolve to get fresh dependencies
                let fresh_dep = self.resolve(&name, Some(&dep.source))?;
                
                // If dependencies changed, update edges
                if fresh_dep.dependencies != dep.dependencies {
                    // Remove old edges from this node
                    graph.remove_edges_from(&name);
                    
                    // Update node with new dependencies
                    graph.nodes.insert(name.clone(), fresh_dep.clone());
                    
                    // Add new edges
                    let deps: Vec<String> = fresh_dep.dependencies.keys().cloned().collect();
                    for d in &deps {
                        graph.reverse_edges.entry(d.clone()).or_default().push(name.clone());
                    }
                    graph.edges.insert(name.clone(), deps);
                    
                    changed = true;
                }
            }
            
            if !changed {
                break;
            }
        }
        
        // Step 5: Prune nodes with in-degree 0 (using virtual root)
        graph.add_virtual_root();
        graph.prune_unreachable();
        
        Ok(graph.get_resolved())
    }
    
    /// Collect all dependency sources recursively (collecting conflicts)
    /// This only collects sources without resolving - use git cache to read aspub.yaml directly
    fn collect_all_sources(
        &self,
        dependencies: &HashMap<String, DependencySource>,
        all_sources: &mut HashMap<String, Vec<DependencySource>>,
        visiting: &mut HashSet<String>,
    ) -> Result<()> {
        for (name, source) in dependencies {
            // Skip if already being processed (circular dependency)
            if visiting.contains(name) {
                continue;
            }
            
            // Add source to the list
            all_sources.entry(name.clone()).or_default().push(source.clone());
            
            // Skip path dependencies (they don't have transitive deps)
            if let DependencySource::Detailed { path: Some(_), .. } = source {
                continue;
            }
            
            // Get transitive dependencies from git cache without full resolve
            if let Ok(transitive) = self.get_transitive_from_cache(name, source) {
                if !transitive.is_empty() {
                    visiting.insert(name.clone());
                    self.collect_all_sources(&transitive, all_sources, visiting)?;
                    visiting.remove(name);
                }
            }
        }
        
        Ok(())
    }
    
    /// Get transitive dependencies from git cache (without full resolve)
    fn get_transitive_from_cache(&self, _name: &str, source: &DependencySource) -> Result<HashMap<String, DependencySource>> {
        use DependencySource::*;
        
        let git_url = match source {
            Detailed { git: Some(url), .. } => url.clone(),
            _ => return Ok(HashMap::new()),
        };
        
        // Clone or open repo
        let repo = self.git_manager.clone_or_open(&git_url)?;
        
        // Get the ref
        let (tag, branch, commit) = match source {
            Detailed { tag: Some(t), .. } => (Some(t.as_str()), None, None),
            Detailed { branch: Some(b), .. } => (None, Some(b.as_str()), None),
            Detailed { commit: Some(c), .. } => (None, None, Some(c.as_str())),
            _ => (None, None, None),
        };
        
        // Checkout the ref
        if let Some(t) = tag {
            GitManager::checkout_tag(&repo, t)?;
        } else if let Some(b) = branch {
            GitManager::checkout_branch(&repo, b)?;
        } else if let Some(c) = commit {
            GitManager::checkout_commit(&repo, c)?;
        }
        
        // Read aspub.yaml
        self.get_transitive_dependencies(&repo)
    }
    
    /// Resolve all conflicts - keep highest version for each dependency
    fn resolve_all_conflicts(&self, all_sources: HashMap<String, Vec<DependencySource>>) -> Result<HashMap<String, DependencySource>> {
        let mut resolved = HashMap::new();
        
        for (name, sources) in all_sources {
            let resolved_source = self.resolve_conflicts(&sources)?;
            resolved.insert(name, resolved_source);
        }
        
        Ok(resolved)
    }

    /// Find the best version that matches the requirement (exact match only for now)
    /// Note: Current implementation only supports exact tag/branch/commit matching
    fn find_best_version(&self, repo: &git2::Repository, requirement: &str) -> Result<VersionRef> {
        let refs = GitManager::get_version_refs(repo)?;
        
        // Try to find exact match first (tag or branch with matching name)
        for r in &refs {
            if r.name == requirement || r.name == format!("v{}", requirement) {
                return Ok(r.clone());
            }
        }
        
        // If no exact match, try to find version-like refs and pick the highest
        let version_refs: Vec<_> = refs
            .iter()
            .filter(|r| r.version.is_some())
            .collect();
        
        if version_refs.is_empty() {
            // No version-like refs, pick any ref and warn
            if let Some(first) = refs.first() {
                eprintln!("Warning: No version '{}' found in repository. Using '{}'.", requirement, first.name);
                return Ok(first.clone());
            }
            bail!("No refs found in repository");
        }
        
        // Sort by version descending (highest first) and pick the highest
        let mut sorted = version_refs.clone();
        sorted.sort_by(|a, b| {
            b.version.as_ref().unwrap().cmp(a.version.as_ref().unwrap())
        });
        
        Ok(sorted.remove(0).clone())
    }
    
    /// Resolve version conflicts by selecting the highest version
    /// This is used when multiple dependencies reference the same package with different sources
    pub fn resolve_conflicts(&self, sources: &[DependencySource]) -> Result<DependencySource> {
        use DependencySource::*;
        
        if sources.is_empty() {
            bail!("No sources to resolve");
        }
        
        // No conflict - only one source, return directly
        if sources.len() == 1 {
            return Ok(sources[0].clone());
        }
        
        // Check for path type - path has highest priority
        for source in sources {
            if let Detailed { path: Some(_), .. } = source {
                eprintln!("Warning: Multiple sources for dependency, using path.");
                return Ok(source.clone());
            }
        }
        
        // Try to parse versions from tag/branch fields
        let mut versioned: Vec<(&DependencySource, crate::version::SemanticVersion)> = Vec::new();
        
        for source in sources {
            match source {
                Detailed { tag: Some(ref t), .. } => {
                    if let Some(v) = crate::version::parse_version(t) {
                        versioned.push((source, v));
                    }
                }
                Detailed { branch: Some(ref b), .. } => {
                    if let Some(v) = crate::version::parse_version(b) {
                        versioned.push((source, v));
                    }
                }
                _ => {}
            }
        }
        
        // If not all sources have parseable versions, report error
        if versioned.len() < sources.len() {
            bail!(
                "Dependency conflict: {} sources provided but only {} have valid version numbers. \
                 All sources must have parseable versions (tag or branch with semver format).",
                sources.len(),
                versioned.len()
            );
        }
        
        // Sort by version descending and pick the highest
        versioned.sort_by(|a, b| b.1.cmp(&a.1));
        
        Ok(versioned[0].0.clone())
    }
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new().expect("Failed to create DependencyResolver")
    }
}
