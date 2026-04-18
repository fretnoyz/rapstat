use crate::git::RepoInfo;
use crate::status_model::{ContextMdInfo, ContextMdStatus};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::path::Path;

/// Inspect CONTEXT.md relative to `project_root` and return structured findings.
/// Never modifies anything — read-only.
pub fn inspect(project_root: &Path, repo_info: &RepoInfo) -> Result<ContextMdInfo> {
    let context_path = project_root.join("CONTEXT.md");

    if !context_path.exists() {
        return Ok(ContextMdInfo {
            status: ContextMdStatus::Missing,
            last_modified: None,
            discrepancies: vec!["CONTEXT.md not found".to_string()],
        });
    }

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
    })
}
