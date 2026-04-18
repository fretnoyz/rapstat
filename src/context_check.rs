use crate::git::RepoInfo;
use crate::status_model::{ContextMdInfo, ContextMdStatus};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

const CONTEXT_MD_STUB: &str = "# Project Context\n\n## What This Is\n\n<!-- Describe the project purpose here -->\n\n## Current State\n\n<!-- Describe current state here -->\n";

/// Probe order for CONTEXT.md:
/// 1. `raptor/context/CONTEXT.md` — Raptor-managed projects
/// 2. `CONTEXT.md` — standalone projects
///
/// If neither exists, creates a stub in the appropriate location:
/// - `raptor/context/CONTEXT.md` if the `raptor/` directory is present
/// - `CONTEXT.md` otherwise
///
/// NOTE: creation of a missing file is the only case where rapstat writes
/// to a CONTEXT.md path. Existing files are never modified.
fn resolve_context_path(project_root: &Path) -> Result<(std::path::PathBuf, &'static str)> {
    let raptor_context = project_root.join("raptor").join("context").join("CONTEXT.md");
    if raptor_context.exists() {
        return Ok((raptor_context, "raptor/context/CONTEXT.md"));
    }

    let root_context = project_root.join("CONTEXT.md");
    if root_context.exists() {
        return Ok((root_context, "CONTEXT.md"));
    }

    // Neither found — create a stub in the appropriate location.
    let raptor_dir = project_root.join("raptor");
    if raptor_dir.is_dir() {
        let context_dir = raptor_dir.join("context");
        std::fs::create_dir_all(&context_dir)?;
        std::fs::write(&raptor_context, CONTEXT_MD_STUB)?;
        return Ok((raptor_context, "raptor/context/CONTEXT.md"));
    }

    std::fs::write(&root_context, CONTEXT_MD_STUB)?;
    Ok((root_context, "CONTEXT.md"))
}

/// Inspect CONTEXT.md relative to `project_root` and return structured findings.
/// Never modifies an existing CONTEXT.md — read-only except for stub creation.
pub fn inspect(project_root: &Path, repo_info: &RepoInfo) -> Result<ContextMdInfo> {
    let (context_path, rel_path) = resolve_context_path(project_root)?;

    let metadata = std::fs::metadata(&context_path)?;
    let last_modified: DateTime<Utc> = metadata.modified()?.into();

    // Drift: CONTEXT.md has not been touched since the last commit.
    let (status, discrepancies) = if last_modified < repo_info.last_commit.timestamp {
        let msg = format!(
            "CONTEXT.md last modified {} but last commit was {}",
            last_modified.to_rfc3339(),
            repo_info.last_commit.timestamp.to_rfc3339()
        );
        (ContextMdStatus::DriftDetected, vec![msg])
    } else {
        (ContextMdStatus::Ok, vec![])
    };

    Ok(ContextMdInfo {
        status,
        last_modified: Some(last_modified),
        discrepancies,
        resolved_path: Some(rel_path.to_string()),
    })
}
