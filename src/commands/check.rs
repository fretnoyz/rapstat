use anyhow::Result;
use colored::Colorize;

use crate::{context_check, git, status_model::ContextMdStatus};

/// Validate CONTEXT.md against the current git state. Read-only — no writes.
pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_info = git::collect(&cwd)?;
    let info = context_check::inspect(&cwd, &repo_info)?;

    match info.status {
        ContextMdStatus::Ok => {
            println!("{}", "CONTEXT.md ok".green());
            if let Some(t) = info.last_modified {
                println!("  Last modified: {}", t.to_rfc3339());
            }
        }
        ContextMdStatus::DriftDetected => {
            println!("{}", "CONTEXT.md drift detected".yellow().bold());
            for d in &info.discrepancies {
                println!("  {}", d.red());
            }
        }
        ContextMdStatus::Missing => {
            println!("{}", "CONTEXT.md missing and could not be created".red().bold());
        }
    }

    Ok(())
}
