use crate::git::RepoInfo;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::fmt;
use std::io::Write;

pub const RAPSTAT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trigger {
    /// Called from the pre-commit hook. The current commit does not exist yet.
    Commit,
    /// Called from the pre-push hook. Full commit metadata is available.
    Push,
    /// Manual invocation or cron. Equivalent to push-level data.
    Scan,
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Trigger::Commit => write!(f, "commit"),
            Trigger::Push => write!(f, "push"),
            Trigger::Scan => write!(f, "scan"),
        }
    }
}

#[derive(Debug)]
pub enum ContextMdStatus {
    Ok,
    DriftDetected,
    Missing,
}

impl fmt::Display for ContextMdStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextMdStatus::Ok => write!(f, "ok"),
            ContextMdStatus::DriftDetected => write!(f, "drift_detected"),
            ContextMdStatus::Missing => write!(f, "missing"),
        }
    }
}

pub struct ContextMdInfo {
    pub status: ContextMdStatus,
    pub last_modified: Option<DateTime<Utc>>,
    pub discrepancies: Vec<String>,
    /// Relative path used: "CONTEXT.md" or "raptor/CONTEXT.md"
    pub resolved_path: Option<String>,
}

pub struct StatusDoc {
    pub project: String,
    pub machine: String,
    pub trigger: Trigger,
    pub updated_at: DateTime<Utc>,
    pub repo: RepoInfo,
    pub context_md: ContextMdInfo,
}

impl StatusDoc {
    /// Serialize to the STATUS.md format: YAML frontmatter + Markdown body.
    pub fn write(&self, out: &mut impl Write) -> Result<()> {
        self.write_frontmatter(out)?;
        writeln!(out)?;
        self.write_body(out)?;
        Ok(())
    }

    fn write_frontmatter(&self, out: &mut impl Write) -> Result<()> {
        writeln!(out, "---")?;
        writeln!(out, "rapstat_version: {}", RAPSTAT_VERSION)?;
        writeln!(out, "project: {}", self.project)?;
        writeln!(out, "machine: {}", self.machine)?;
        writeln!(out, "trigger: {}", self.trigger)?;
        writeln!(out, "updated_at: {}", self.updated_at.to_rfc3339())?;
        writeln!(out, "branch: {}", self.repo.branch)?;
        writeln!(out, "---")?;
        Ok(())
    }

    fn write_body(&self, out: &mut impl Write) -> Result<()> {
        // Per spec: the commit trigger has no SHA — the commit doesn't exist yet.
        if self.trigger != Trigger::Commit {
            self.write_last_commit_section(out)?;
        }
        self.write_activity_section(out)?;
        self.write_context_md_section(out)?;
        if self.trigger != Trigger::Commit {
            self.write_contributors_section(out)?;
        }
        Ok(())
    }

    fn write_last_commit_section(&self, out: &mut impl Write) -> Result<()> {
        writeln!(out, "## Last Commit")?;
        writeln!(out, "- SHA: {}", self.repo.last_commit.sha)?;
        writeln!(out, "- Message: {}", self.repo.last_commit.message)?;
        writeln!(out, "- Author: {}", self.repo.last_commit.author)?;
        writeln!(
            out,
            "- Timestamp: {}",
            self.repo.last_commit.timestamp.to_rfc3339()
        )?;
        writeln!(out)?;
        Ok(())
    }

    fn write_activity_section(&self, out: &mut impl Write) -> Result<()> {
        writeln!(out, "## Activity")?;
        writeln!(
            out,
            "- Commits since last push: {}",
            self.repo.commits_since_push
        )?;
        match self.repo.last_push {
            Some(t) => writeln!(out, "- Last push: {}", t.to_rfc3339())?,
            None => writeln!(out, "- Last push: never")?,
        }
        match self.repo.days_since_last_push {
            Some(d) => writeln!(out, "- Days since last push: {}", d)?,
            None => writeln!(out, "- Days since last push: n/a")?,
        }
        writeln!(
            out,
            "- Days since last commit: {}",
            self.repo.days_since_last_commit
        )?;
        writeln!(out)?;
        Ok(())
    }

    fn write_context_md_section(&self, out: &mut impl Write) -> Result<()> {
        writeln!(out, "## CONTEXT.md")?;
        writeln!(out, "- Status: {}", self.context_md.status)?;
        if let Some(ref p) = self.context_md.resolved_path {
            writeln!(out, "- Path: {}", p)?;
        }
        match self.context_md.last_modified {
            Some(t) => writeln!(out, "- Last modified: {}", t.to_rfc3339())?,
            None => writeln!(out, "- Last modified: n/a")?,
        }
        if self.context_md.discrepancies.is_empty() {
            writeln!(out, "- Discrepancies: none")?;
        } else {
            writeln!(out, "- Discrepancies:")?;
            for d in &self.context_md.discrepancies {
                writeln!(out, "  - {}", d)?;
            }
        }
        writeln!(out)?;
        Ok(())
    }

    fn write_contributors_section(&self, out: &mut impl Write) -> Result<()> {
        writeln!(out, "## Contributors")?;
        for c in &self.repo.contributors {
            writeln!(
                out,
                "- {}: {} commits, last active {}",
                c.name,
                c.commit_count,
                c.last_active.to_rfc3339()
            )?;
        }
        writeln!(out)?;
        Ok(())
    }
}
