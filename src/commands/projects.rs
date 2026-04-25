use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::registry::Registry;

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

/// `rapstat projects list`
pub fn list() -> Result<()> {
    let registry = Registry::load()?;

    if registry.projects.is_empty() {
        println!("{}", "No projects registered. Use `rapstat projects add` to register one.".yellow());
        return Ok(());
    }

    // Column widths.
    let name_w = registry.projects.iter().map(|e| e.name.len()).max().unwrap_or(4).max(4);
    let path_w = registry.projects.iter().map(|e| e.path.display().to_string().len()).max().unwrap_or(4).max(4);

    println!(
        "{:name_w$}  {:path_w$}  {}  {}",
        "NAME".bold(),
        "PATH".bold(),
        "STATUS.md".bold(),
        "ADDED".bold(),
        name_w = name_w,
        path_w = path_w,
    );
    println!("{}", "─".repeat(name_w + path_w + 32).dimmed());

    for entry in &registry.projects {
        let status_age = status_age(&entry.path);
        let added = entry.added_at.format("%Y-%m-%d").to_string();
        println!(
            "{:name_w$}  {:path_w$}  {:9}  {}",
            entry.name.green(),
            entry.path.display().to_string().dimmed(),
            status_age,
            added.dimmed(),
            name_w = name_w,
            path_w = path_w,
        );
    }

    Ok(())
}

/// `rapstat projects add [PATH] [--name NAME]`
pub fn add(path: Option<PathBuf>, name: Option<String>) -> Result<()> {
    let project_root = match path {
        Some(p) => p,
        None => std::env::current_dir()?,
    };

    let resolved_name = match name {
        Some(n) => n,
        None => Registry::infer_name(&project_root),
    };

    let mut registry = Registry::load()?;
    registry.add(resolved_name.clone(), project_root.clone())?;
    registry.save()?;

    println!(
        "{} '{}' → {}",
        "Registered".green().bold(),
        resolved_name,
        project_root.display()
    );
    Ok(())
}

/// `rapstat projects remove NAME`
pub fn remove(name: &str) -> Result<()> {
    let mut registry = Registry::load()?;

    if registry.remove(name) {
        registry.save()?;
        println!("{} '{}'", "Removed".yellow().bold(), name);
    } else {
        anyhow::bail!("no project named '{}' is registered", name);
    }
    Ok(())
}

/// `rapstat projects scan`
///
/// Runs a daemon-level scan across every registered project.
pub fn scan_all() -> Result<()> {
    use crate::{
        config::Config,
        context_check,
        git,
        status_model::{StatusDoc, Trigger, WorkflowWipStatus},
    };
    use std::io::BufWriter;

    let registry = Registry::load()?;

    if registry.projects.is_empty() {
        println!("{}", "No projects registered.".yellow());
        return Ok(());
    }

    let mut ok = 0usize;
    let mut failed = 0usize;

    for entry in &registry.projects {
        print!("  scanning {} … ", entry.name.cyan());

        let result: Result<()> = (|| {
            let root = &entry.path;
            let config = Config::load(root)?;
            let repo_info = git::collect(root)?;
            let context_md = context_check::inspect(root, &repo_info)?;

            let doc = StatusDoc {
                project: config.project.name,
                machine: hostname(),
                trigger: Trigger::Scan,
                updated_at: Utc::now(),
                repo: repo_info,
                context_md,
                workflow_wip: WorkflowWipStatus::load(root),
            };

            let status_path = root.join("STATUS.md");
            let file = std::fs::File::create(&status_path)
                .with_context(|| format!("cannot create {}", status_path.display()))?;
            let mut writer = BufWriter::new(file);
            doc.write(&mut writer)?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                println!("{}", "ok".green());
                ok += 1;
            }
            Err(e) => {
                println!("{} — {}", "FAILED".red(), e);
                failed += 1;
            }
        }
    }

    println!(
        "\n{} scanned, {} ok, {} failed",
        registry.projects.len(),
        ok.to_string().green(),
        if failed > 0 { failed.to_string().red() } else { failed.to_string().normal() },
    );
    Ok(())
}

/// `rapstat projects import <RAPTOR_YAML>`
///
/// Reads the `projects:` block from a raptor.yaml and registers every entry
/// that has a valid `repo_path`. Skips already-registered projects.
pub fn import(raptor_yaml: PathBuf) -> Result<()> {
    #[derive(Deserialize)]
    struct RaptorYaml {
        projects: HashMap<String, RaptorProject>,
    }
    #[derive(Deserialize)]
    struct RaptorProject {
        name: String,
        repo_path: PathBuf,
    }

    let raw = std::fs::read_to_string(&raptor_yaml)
        .with_context(|| format!("cannot read {}", raptor_yaml.display()))?;
    let parsed: RaptorYaml = serde_yaml::from_str(&raw)
        .with_context(|| format!("cannot parse {}", raptor_yaml.display()))?;

    let mut registry = Registry::load()?;
    let mut added = 0usize;
    let mut skipped = 0usize;

    // Sort by key for deterministic output.
    let mut entries: Vec<_> = parsed.projects.into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    for (_id, proj) in entries {
        match registry.add(proj.name.clone(), proj.repo_path.clone()) {
            Ok(()) => {
                println!(
                    "{} '{}' → {}",
                    "Registered".green().bold(),
                    proj.name,
                    proj.repo_path.display()
                );
                added += 1;
            }
            Err(e) => {
                println!("{} '{}' — {}", "Skipped".dimmed(), proj.name, e);
                skipped += 1;
            }
        }
    }

    registry.save()?;
    println!(
        "\n{} imported, {} skipped",
        added.to_string().green(),
        skipped.to_string().dimmed()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn status_age(project_root: &std::path::Path) -> String {
    let path = project_root.join("STATUS.md");
    if !path.exists() {
        return "missing".yellow().to_string();
    }
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                let secs = elapsed.as_secs();
                return if secs < 3600 {
                    format!("{:3}m ago", secs / 60).green().to_string()
                } else if secs < 86400 {
                    format!("{:3}h ago", secs / 3600).normal().to_string()
                } else {
                    format!("{:3}d ago", secs / 86400).yellow().to_string()
                };
            }
        }
    }
    "unknown".dimmed().to_string()
}

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
