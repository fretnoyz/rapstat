use crate::git::RepoInfo;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::Path;

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
    pub workflow_wip: Option<WorkflowWipStatus>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowWipStatus {
    pub schema_version: Option<String>,
    pub project_id: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub current_sprint: Option<SprintWipSummary>,
    #[serde(default)]
    pub active_sprint_ids: Vec<String>,
    #[serde(default)]
    pub counts: WorkflowWipCounts,
    #[serde(default)]
    pub sprints: Vec<SprintWipSummary>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkflowWipCounts {
    #[serde(default)]
    pub total_sprints: usize,
    #[serde(default)]
    pub active_sprints: usize,
    #[serde(default)]
    pub closed_sprints: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SprintWipSummary {
    pub sprint_id: String,
    pub sprint_state: Option<String>,
    pub latest_event_type: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub dispatchable: Vec<String>,
    #[serde(default)]
    pub wip_tranches: Vec<TrancheWipSummary>,
    #[serde(default)]
    pub counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrancheWipSummary {
    pub id: String,
    pub state: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub last_event: Option<String>,
}

impl WorkflowWipStatus {
    pub fn load(project_root: &Path) -> Option<Self> {
        if let Some(snapshot) = Self::load_snapshot(project_root) {
            return Some(snapshot);
        }
        Self::load_from_workflow_states(project_root)
    }

    fn load_snapshot(project_root: &Path) -> Option<Self> {
        let path = project_root.join("raptor").join("artifacts").join("data").join("wip_status.yaml");
        let content = std::fs::read_to_string(path).ok()?;
        serde_yaml::from_str(&content).ok()
    }

    fn load_from_workflow_states(project_root: &Path) -> Option<Self> {
        let tranches_dir = project_root.join("raptor").join("tranches");
        let entries = std::fs::read_dir(tranches_dir).ok()?;
        let mut sprints: Vec<SprintWipSummary> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let sprint_dir = entry.path();
                if !sprint_dir.is_dir() {
                    return None;
                }
                let state_path = sprint_dir.join("_workflow_state.yaml");
                let content = std::fs::read_to_string(state_path).ok()?;
                let workflow_state: WorkflowStateFile = serde_yaml::from_str(&content).ok()?;
                Some(SprintWipSummary::from_workflow_state(&sprint_dir, workflow_state))
            })
            .collect();
        if sprints.is_empty() {
            return None;
        }

        sprints.sort_by(|a, b| a.sprint_id.cmp(&b.sprint_id));
        let active_sprint_ids: Vec<String> = sprints
            .iter()
            .filter(|sprint| sprint.sprint_state.as_deref() != Some("CLOSED"))
            .map(|sprint| sprint.sprint_id.clone())
            .collect();
        let current_sprint = sprints
            .iter()
            .filter(|sprint| sprint.sprint_state.as_deref() != Some("CLOSED"))
            .max_by_key(|sprint| sprint.updated_at)
            .cloned()
            .or_else(|| sprints.first().cloned());
        let updated_at = sprints.iter().filter_map(|sprint| sprint.updated_at).max();
        let project_id = current_sprint.as_ref().map(|_| project_root.file_name().map(|name| name.to_string_lossy().to_string())).flatten();

        Some(Self {
            schema_version: Some("2".to_string()),
            project_id,
            updated_at,
            current_sprint,
            active_sprint_ids: active_sprint_ids.clone(),
            counts: WorkflowWipCounts {
                total_sprints: sprints.len(),
                active_sprints: active_sprint_ids.len(),
                closed_sprints: sprints.len().saturating_sub(active_sprint_ids.len()),
            },
            sprints,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowStateFile {
    sprint_id: Option<String>,
    sprint_state: Option<String>,
    updated_at: Option<DateTime<Utc>>,
    event_log: Option<Vec<WorkflowEventRecord>>,
    tranches: Option<Vec<WorkflowStateTranche>>,
}

#[derive(Debug, Deserialize)]
struct WorkflowEventRecord {
    #[serde(rename = "type")]
    event_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowStateTranche {
    id: Option<String>,
    state: Option<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    last_event: Option<String>,
}

impl SprintWipSummary {
    fn from_workflow_state(sprint_dir: &Path, workflow_state: WorkflowStateFile) -> Self {
        let tranches = workflow_state.tranches.unwrap_or_default();
        Self {
            sprint_id: workflow_state.sprint_id.unwrap_or_else(|| sprint_dir.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| "unknown".to_string())),
            sprint_state: workflow_state.sprint_state,
            latest_event_type: workflow_state
                .event_log
                .as_ref()
                .and_then(|event_log| event_log.last())
                .and_then(|event| event.event_type.clone()),
            updated_at: workflow_state.updated_at,
            dispatchable: tranches
                .iter()
                .filter(|tranche| tranche.state.as_deref() == Some("READY"))
                .filter_map(|tranche| tranche.id.clone())
                .collect(),
            wip_tranches: tranches
                .iter()
                .filter(|tranche| !matches!(tranche.state.as_deref(), Some("DONE" | "ARCHIVED" | "CANCELLED")))
                .filter_map(|tranche| {
                    Some(TrancheWipSummary {
                        id: tranche.id.clone()?,
                        state: tranche.state.clone().unwrap_or_else(|| "unknown".to_string()),
                        depends_on: tranche.depends_on.clone(),
                        last_event: tranche.last_event.clone(),
                    })
                })
                .collect(),
            counts: collect_tranche_counts(&tranches),
        }
    }
}

fn collect_tranche_counts(tranches: &[WorkflowStateTranche]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::from([
        ("total".to_string(), 0usize),
        ("wip".to_string(), 0usize),
        ("done".to_string(), 0usize),
    ]);
    counts.insert("total".to_string(), tranches.len());
    for tranche in tranches {
        let state = tranche.state.as_deref().unwrap_or("unknown").to_ascii_lowercase();
        match state.as_str() {
            "done" | "archived" | "cancelled" => {
                *counts.entry("done".to_string()).or_insert(0) += 1;
                if state != "done" {
                    *counts.entry(state).or_insert(0) += 1;
                }
            }
            _ => {
                *counts.entry("wip".to_string()).or_insert(0) += 1;
                *counts.entry(state).or_insert(0) += 1;
            }
        }
    }
    counts
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
        self.write_workflow_wip_section(out)?;
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

    fn write_workflow_wip_section(&self, out: &mut impl Write) -> Result<()> {
        let Some(workflow_wip) = &self.workflow_wip else {
            return Ok(());
        };

        writeln!(out, "## Workflow WIP")?;
        // One-line project summary
        let updated_str = workflow_wip
            .updated_at
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        writeln!(
            out,
            "- {} active / {} total sprints  |  updated {}",
            workflow_wip.counts.active_sprints,
            workflow_wip.counts.total_sprints,
            updated_str
        )?;

        // Active sprint table: one row per sprint
        let active_sprints: Vec<&SprintWipSummary> = workflow_wip
            .sprints
            .iter()
            .filter(|s| s.sprint_state.as_deref() != Some("CLOSED"))
            .collect();

        if active_sprints.is_empty() {
            writeln!(out, "- No active sprints.")?;
            writeln!(out)?;
            return Ok(());
        }

        writeln!(out, "")?;
        writeln!(out, "| Sprint | State | Tranches (wip/done/total) | Dispatchable | Latest event |")?;
        writeln!(out, "|--------|-------|---------------------------|--------------|--------------|")?;
        for sprint in &active_sprints {
            let state = sprint.sprint_state.as_deref().unwrap_or("?");
            let wip   = sprint.counts.get("wip").copied().unwrap_or(0);
            let done  = sprint.counts.get("done").copied().unwrap_or(0);
            let total = sprint.counts.get("total").copied().unwrap_or(0);
            let dispatchable = if sprint.dispatchable.is_empty() {
                "—".to_string()
            } else {
                sprint.dispatchable.join(", ")
            };
            let latest = sprint.latest_event_type.as_deref().unwrap_or("—");
            writeln!(
                out,
                "| {} | {} | {}/{}/{} | {} | {} |",
                sprint.sprint_id, state, wip, done, total, dispatchable, latest
            )?;
        }
        writeln!(out)?;

        // WIP tranche detail — only when there are blocked/in-flight tranches
        let mut any_wip = false;
        for sprint in &active_sprints {
            if !sprint.wip_tranches.is_empty() {
                if !any_wip {
                    writeln!(out, "WIP tranches:")?;
                    any_wip = true;
                }
                for tranche in &sprint.wip_tranches {
                    let deps = if tranche.depends_on.is_empty() {
                        String::new()
                    } else {
                        format!(" ← {}", tranche.depends_on.join(", "))
                    };
                    writeln!(
                        out,
                        "  {}/{} ({}){}",
                        sprint.sprint_id, tranche.id, tranche.state, deps
                    )?;
                }
            }
        }
        if any_wip {
            writeln!(out)?;
        }
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

#[cfg(test)]
mod tests {
    use super::{ContextMdInfo, ContextMdStatus, SprintWipSummary, StatusDoc, TrancheWipSummary, Trigger, WorkflowWipCounts, WorkflowWipStatus};
    use crate::git::{CommitInfo, ContributorInfo, RepoInfo};
    use chrono::{TimeZone, Utc};
    use std::collections::BTreeMap;

    #[test]
    fn writes_workflow_wip_section_when_snapshot_present() {
        let repo = RepoInfo {
            branch: "main".to_string(),
            last_commit: CommitInfo {
                sha: "abc123".to_string(),
                message: "Test commit".to_string(),
                author: "Tester".to_string(),
                timestamp: Utc.with_ymd_and_hms(2026, 4, 24, 12, 0, 0).unwrap(),
            },
            commits_since_push: 1,
            last_push: Some(Utc.with_ymd_and_hms(2026, 4, 24, 10, 0, 0).unwrap()),
            days_since_last_commit: 0,
            days_since_last_push: Some(0),
            contributors: vec![ContributorInfo {
                name: "Tester".to_string(),
                commit_count: 3,
                last_active: Utc.with_ymd_and_hms(2026, 4, 24, 12, 0, 0).unwrap(),
            }],
        };
        let workflow_wip = WorkflowWipStatus {
            schema_version: Some("2".to_string()),
            project_id: Some("demo-project".to_string()),
            updated_at: Some(Utc.with_ymd_and_hms(2026, 4, 24, 12, 30, 0).unwrap()),
            current_sprint: Some(SprintWipSummary {
                sprint_id: "Sprint54".to_string(),
                sprint_state: Some("EXECUTING".to_string()),
                latest_event_type: Some("validation_requested".to_string()),
                updated_at: Some(Utc.with_ymd_and_hms(2026, 4, 24, 12, 30, 0).unwrap()),
                dispatchable: vec!["S54T2".to_string()],
                wip_tranches: vec![TrancheWipSummary {
                    id: "S54T1".to_string(),
                    state: "ACTIVE".to_string(),
                    depends_on: vec!["S54T0".to_string()],
                    last_event: Some("status_active".to_string()),
                }],
                counts: BTreeMap::new(),
            }),
            active_sprint_ids: vec!["Sprint54".to_string()],
            counts: WorkflowWipCounts {
                total_sprints: 2,
                active_sprints: 1,
                closed_sprints: 1,
            },
            sprints: vec![
                SprintWipSummary {
                    sprint_id: "Sprint53".to_string(),
                    sprint_state: Some("CLOSED".to_string()),
                    latest_event_type: Some("retrospective_recorded".to_string()),
                    updated_at: Some(Utc.with_ymd_and_hms(2026, 4, 24, 11, 30, 0).unwrap()),
                    dispatchable: vec![],
                    wip_tranches: vec![],
                    counts: BTreeMap::new(),
                },
                SprintWipSummary {
                    sprint_id: "Sprint54".to_string(),
                    sprint_state: Some("EXECUTING".to_string()),
                    latest_event_type: Some("validation_requested".to_string()),
                    updated_at: Some(Utc.with_ymd_and_hms(2026, 4, 24, 12, 30, 0).unwrap()),
                    dispatchable: vec!["S54T2".to_string()],
                    wip_tranches: vec![TrancheWipSummary {
                        id: "S54T1".to_string(),
                        state: "ACTIVE".to_string(),
                        depends_on: vec!["S54T0".to_string()],
                        last_event: Some("status_active".to_string()),
                    }],
                    counts: BTreeMap::new(),
                },
            ],
        };

        let doc = StatusDoc {
            project: "demo-project".to_string(),
            machine: "test-host".to_string(),
            trigger: Trigger::Scan,
            updated_at: Utc.with_ymd_and_hms(2026, 4, 24, 13, 0, 0).unwrap(),
            repo,
            context_md: ContextMdInfo {
                status: ContextMdStatus::Ok,
                last_modified: Some(Utc.with_ymd_and_hms(2026, 4, 24, 11, 0, 0).unwrap()),
                discrepancies: vec![],
                resolved_path: Some("raptor/context/CONTEXT.md".to_string()),
            },
            workflow_wip: Some(workflow_wip),
        };

        let mut output = Vec::new();
        doc.write(&mut output).unwrap();
        let rendered = String::from_utf8(output).unwrap();

        assert!(rendered.contains("## Workflow WIP"));
        // One-line project summary
        assert!(rendered.contains("1 active / 2 total sprints"));
        // Compact table row for active sprint only
        assert!(rendered.contains("| Sprint54 | EXECUTING |"));
        assert!(rendered.contains("| S54T2 |")); // dispatchable column
        // WIP tranche section
        assert!(rendered.contains("Sprint54/S54T1 (ACTIVE) ← S54T0"));
        // Closed sprint must NOT appear in the table
        assert!(!rendered.contains("| Sprint53 | CLOSED |"));
    }
}
