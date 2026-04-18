use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub project: ProjectSection,
    #[serde(default)]
    pub hooks: HooksSection,
    #[serde(default)]
    pub validation: ValidationSection,
}

#[derive(Debug, Deserialize)]
pub struct ProjectSection {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct HooksSection {
    #[serde(default = "default_true")]
    pub pre_commit: bool,
    #[serde(default = "default_true")]
    pub pre_push: bool,
}

impl Default for HooksSection {
    fn default() -> Self {
        Self {
            pre_commit: true,
            pre_push: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ValidationSection {
    #[serde(default = "default_true")]
    pub check_context_md: bool,
    #[serde(default = "default_true")]
    pub flag_only: bool,
}

impl Default for ValidationSection {
    fn default() -> Self {
        Self {
            check_context_md: true,
            flag_only: true,
        }
    }
}

impl Config {
    /// Load config from `.rapstat/config.toml` in the project root, falling back
    /// to `~/.rapstat/config.toml`, then a minimal default derived from the
    /// directory name.
    pub fn load(project_root: &Path) -> Result<Self> {
        let project_config = project_root.join(".rapstat").join("config.toml");
        if project_config.exists() {
            return load_file(&project_config);
        }

        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".rapstat").join("config.toml");
            if user_config.exists() {
                return load_file(&user_config);
            }
        }

        // No config file found — derive project name from the directory name.
        let name = project_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Config {
            project: ProjectSection { name },
            hooks: HooksSection::default(),
            validation: ValidationSection::default(),
        })
    }
}

fn load_file(path: &PathBuf) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))
}
