use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::ValueEnum;
use std::io::BufWriter;
use std::path::Path;

use crate::{
    config::Config,
    git,
    status_model::{ContextMdInfo, ContextMdStatus, StatusDoc, Trigger},
};

#[derive(Debug, Clone, ValueEnum)]
pub enum TriggerArg {
    /// Pre-commit hook: commit SHA not yet available
    Commit,
    /// Pre-push hook: full commit metadata available
    Push,
    /// Manual invocation or cron
    Scan,
}

pub fn run(trigger: TriggerArg) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root)?;
    let repo_info = git::collect(&project_root)?;

    let trigger = match trigger {
        TriggerArg::Commit => Trigger::Commit,
        TriggerArg::Push => Trigger::Push,
        TriggerArg::Scan => Trigger::Scan,
    };

    let context_md = inspect_context_md(&project_root, &repo_info)?;

    let doc = StatusDoc {
        project: config.project.name,
        machine: hostname(),
        trigger,
        updated_at: Utc::now(),
        repo: repo_info,
        context_md,
    };

    let status_path = project_root.join("STATUS.md");
    let file = std::fs::File::create(&status_path)?;
    let mut writer = BufWriter::new(file);
    doc.write(&mut writer)?;

    Ok(())
}

fn inspect_context_md(project_root: &Path, repo_info: &git::RepoInfo) -> Result<ContextMdInfo> {
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

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
