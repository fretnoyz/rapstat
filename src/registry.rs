use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A single entry in the global project registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub name: String,
    pub path: PathBuf,
    pub added_at: DateTime<Utc>,
}

/// The full registry, stored at `~/.rapstat/projects.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub projects: Vec<RegistryEntry>,
}

impl Registry {
    /// Path to the registry file: `~/.rapstat/projects.toml`.
    pub fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".rapstat").join("projects.toml"))
    }

    /// Load the registry from disk. Returns an empty registry if the file does
    /// not exist yet.
    pub fn load() -> Result<Self> {
        let path = Self::path().context("cannot determine home directory")?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Persist the registry to disk, creating the directory if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("cannot determine home directory")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
        }
        let contents = toml::to_string_pretty(self)
            .context("failed to serialise registry")?;
        std::fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))
    }

    /// Add a project. Returns an error if the path is already registered.
    pub fn add(&mut self, name: String, path: PathBuf) -> Result<()> {
        let canonical = path.canonicalize()
            .with_context(|| format!("path does not exist: {}", path.display()))?;

        // Reject duplicate path or name.
        if let Some(existing) = self.projects.iter().find(|e| {
            e.path == canonical || e.name == name
        }) {
            anyhow::bail!(
                "project '{}' at {} is already registered",
                existing.name,
                existing.path.display()
            );
        }

        self.projects.push(RegistryEntry {
            name,
            path: canonical,
            added_at: Utc::now(),
        });
        Ok(())
    }

    /// Remove a project by name. Returns `true` if it was present.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.projects.len();
        self.projects.retain(|e| e.name != name);
        self.projects.len() < before
    }

    /// Resolve a display name for a project root by checking `.rapstat/config.toml`
    /// then falling back to the directory name.
    pub fn infer_name(project_root: &Path) -> String {
        let config_path = project_root.join(".rapstat").join("config.toml");
        if config_path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&config_path) {
                #[derive(Deserialize)]
                struct Minimal {
                    project: ProjectName,
                }
                #[derive(Deserialize)]
                struct ProjectName {
                    name: String,
                }
                if let Ok(m) = toml::from_str::<Minimal>(&raw) {
                    return m.project.name;
                }
            }
        }
        project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }
}
