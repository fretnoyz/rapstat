use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use git2::{Repository, Sort};
use std::collections::HashMap;
use std::path::Path;

pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
}

pub struct ContributorInfo {
    pub name: String,
    pub commit_count: usize,
    pub last_active: DateTime<Utc>,
}

pub struct RepoInfo {
    pub branch: String,
    pub last_commit: CommitInfo,
    pub commits_since_push: usize,
    pub last_push: Option<DateTime<Utc>>,
    pub days_since_last_commit: i64,
    pub days_since_last_push: Option<i64>,
    pub contributors: Vec<ContributorInfo>,
}

/// Collect all observable git facts from the repo at `repo_path`.
pub fn collect(repo_path: &Path) -> Result<RepoInfo> {
    let repo = Repository::discover(repo_path)
        .context("failed to open git repository")?;

    let head = repo.head().context("failed to get HEAD")?;
    let branch = head.shorthand().unwrap_or("HEAD").to_string();

    let head_commit = head
        .peel_to_commit()
        .context("failed to resolve HEAD to commit")?;

    let last_commit = CommitInfo {
        sha: head_commit.id().to_string(),
        message: head_commit.summary().unwrap_or("").to_string(),
        author: head_commit.author().name().unwrap_or("unknown").to_string(),
        timestamp: git_time_to_utc(head_commit.time().seconds()),
    };

    let now = Utc::now();
    let days_since_last_commit = (now - last_commit.timestamp).num_days();

    let remote_oid = find_remote_tip(&repo, &branch);

    let (commits_since_push, last_push) = match remote_oid {
        Some(remote_oid) => {
            let count = count_commits_ahead(&repo, head_commit.id(), remote_oid)?;
            let remote_commit = repo.find_commit(remote_oid)?;
            let push_time = git_time_to_utc(remote_commit.time().seconds());
            (count, Some(push_time))
        }
        None => {
            // No remote tracking branch — treat all commits as unpushed.
            let count = count_all_commits(&repo, head_commit.id())?;
            (count, None)
        }
    };

    let days_since_last_push = last_push.map(|t| (now - t).num_days());
    let contributors = collect_contributors(&repo, head_commit.id())?;

    Ok(RepoInfo {
        branch,
        last_commit,
        commits_since_push,
        last_push,
        days_since_last_commit,
        days_since_last_push,
        contributors,
    })
}

fn git_time_to_utc(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

/// Look for `refs/remotes/origin/<branch>` and return its tip OID.
fn find_remote_tip(repo: &Repository, branch: &str) -> Option<git2::Oid> {
    let ref_name = format!("refs/remotes/origin/{}", branch);
    repo.find_reference(&ref_name)
        .ok()
        .and_then(|r| r.target())
}

/// Count commits reachable from `head` but not from `remote`.
fn count_commits_ahead(repo: &Repository, head: git2::Oid, remote: git2::Oid) -> Result<usize> {
    if head == remote {
        return Ok(0);
    }
    let mut walk = repo.revwalk().context("failed to create revwalk")?;
    walk.push(head)?;
    walk.hide(remote)?;
    walk.set_sorting(Sort::TOPOLOGICAL)?;
    Ok(walk.count())
}

/// Count all commits reachable from `head`.
fn count_all_commits(repo: &Repository, head: git2::Oid) -> Result<usize> {
    let mut walk = repo.revwalk().context("failed to create revwalk")?;
    walk.push(head)?;
    walk.set_sorting(Sort::TOPOLOGICAL)?;
    Ok(walk.count())
}

/// Walk the full commit history and tally contributors by author name.
fn collect_contributors(repo: &Repository, head: git2::Oid) -> Result<Vec<ContributorInfo>> {
    let mut walk = repo.revwalk().context("failed to create revwalk")?;
    walk.push(head)?;
    walk.set_sorting(Sort::TIME)?;

    // (commit_count, last_active)
    let mut map: HashMap<String, (usize, DateTime<Utc>)> = HashMap::new();

    for oid_result in walk {
        let oid = oid_result?;
        let commit = repo.find_commit(oid)?;
        let name = commit.author().name().unwrap_or("unknown").to_string();
        let ts = git_time_to_utc(commit.time().seconds());
        let entry = map.entry(name).or_insert((0, ts));
        entry.0 += 1;
        if ts > entry.1 {
            entry.1 = ts;
        }
    }

    let mut contributors: Vec<ContributorInfo> = map
        .into_iter()
        .map(|(name, (commit_count, last_active))| ContributorInfo {
            name,
            commit_count,
            last_active,
        })
        .collect();

    // Most active contributor first.
    contributors.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
    Ok(contributors)
}
