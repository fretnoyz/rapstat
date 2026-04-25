use anyhow::Result;
use chrono::Utc;
use clap::ValueEnum;
use std::io::BufWriter;

use crate::{
    config::Config,
    context_check,
    git,
    status_model::{StatusDoc, Trigger, WorkflowWipStatus},
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

    let context_md = context_check::inspect(&project_root, &repo_info)?;

    let doc = StatusDoc {
        project: config.project.name,
        machine: hostname(),
        trigger,
        updated_at: Utc::now(),
        repo: repo_info,
        context_md,
        workflow_wip: WorkflowWipStatus::load(&project_root),
    };

    let status_path = project_root.join("STATUS.md");
    let file = std::fs::File::create(&status_path)?;
    let mut writer = BufWriter::new(file);
    doc.write(&mut writer)?;

    Ok(())
}

fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
