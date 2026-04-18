use crate::git::RepoInfo;
use crate::status_model::{ContextMdInfo, ContextMdStatus};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

/// Probe order for CONTEXT.md:
/// 1. `raptor/CONTEXT.md` — Raptor-managed projects
/// 2. `CONTEXT.md` — standalone projects
/// Returns None if neither exists.
fn resolve_context_path(project_root: &Path) -> Option<(std::path::PathBuf, &'static str)> {
    let raptor_path = project_root.join("raptor").join("CONTEXT.md");
    if raptor_path.exists() {
        return Some((raptor_path, "raptor/CONTEXT.md"));
    }
    let root_path = project_root.join("CONTEXT.md");
    if root_path.exists() {
        return Some((root_path, "CONTEXT.md"));
    }
    None
}

/// Inspect CONTEXT.md relative to `project_root` and return structured findings.
/// Never modifies anything — read-only.
pub fn inspect(project_root: &Path, repo_info: &RepoInfo) -> Result<ContextMdInfo> {
    let Some((context_path, rel_path)) = resolve_context_path(project_root) else {
        return Ok(ContextMdInfo {
            status: ContextMdStatus::Missing,
            last_modified: None,
            discrepancies: vec!["CONTEXT.md not found (checked raptor/CONTEXT.md and CONTEXT.md)".to_string()],
            resolved_path: None,
        });
    };

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
