# Rapstat
## Project Observability Tool — Requirements Specification v1.2

**Texas Quantitative**

| Field | Value |
|---|---|
| Version | 1.2 |
| Date | April 2026 |
| Author | Dowell Stackpole |
| Status | Draft — Pending Sprint Assignment |
| Language | Rust |
| Targets | aarch64-apple-darwin, x86_64-unknown-linux-gnu |

---

## 1. Purpose & Problem Statement

Rapstat is a lightweight, single-binary observability tool for Texas Quantitative's development environment. It solves a specific problem: project state is currently distributed across multiple machines, multiple Claude accounts, and agent-generated documentation that is non-deterministically maintained.

Rapstat provides an independent, objective record of what is actually happening in each project — derived from git history, file system state, and project structure — rather than relying on agent self-reporting or manual human capture.

**It does one job well: observe and record project state.**

---

## 2. Design Axioms

- **One job.** Rapstat observes and records. It does not orchestrate, execute, report, or decide.
- **Independent observer.** Rapstat is not part of Raptor's execution chain. It watches Raptor. An agent cannot influence its own observer.
- **Never blocks work.** All hooks are non-blocking. A Rapstat failure never prevents a commit or push.
- **CONTEXT.md stub creation only.** Rapstat will not modify an existing CONTEXT.md. If no CONTEXT.md is found, rapstat creates a minimal stub in the correct location: `raptor/context/CONTEXT.md` for Raptor-managed projects (detected by the presence of a `raptor/` directory), `CONTEXT.md` at the project root otherwise.
- **STATUS.md is Rapstat's sole write target.** One writer, no conflicts.
- **Deterministic output.** Given the same repo state, Rapstat produces the same STATUS.md. No LLM calls in the critical path.
- **Facts only.** Rapstat records observable facts. Interpretation is the responsibility of the reporting layer.

---

## 3. Target Machines

| Machine | Platform |
|---|---|
| MacBook Pro (primary) | macOS, aarch64 — primary dev machine, most projects |
| MacBook Pro (getchkd) | macOS, aarch64 — dedicated client machine |
| System76 Linux | Ubuntu, x86_64 — NVIDIA 5090, inference work |
| MacBook Air (secondary) | macOS, aarch64 — occasional direct dev, usually remote |

Non-dev machines (Mac Mini / studio MacBook Air) are out of scope for git hooks but may be reached by the daemon for filesystem observation.

---

## 4. File Contracts

### 4.1 Read/Write Matrix

| File | Daemon | Raptor Agents | Human |
|---|---|---|---|
| CONTEXT.md | Read | Read / Write | Read / Write |
| STATUS.md | Write | Read | Read |

### 4.2 STATUS.md Schema

Each project receives a STATUS.md at the project root. Rapstat owns this file exclusively. Format is human-readable Markdown with machine-parseable YAML frontmatter. All fields are observable facts — no inference, no LLM-generated content.

The frontmatter is machine-queryable. The Markdown body is human-readable. Together they serve both audiences without compromise.

#### Frontmatter — machine-parseable

```yaml
---
rapstat_version: 1.0
project: <project name>
machine: <hostname>
trigger: commit | push | daemon
updated_at: <ISO 8601>
branch: <branch name>
---
```

#### Body — human-readable

```markdown
## Last Commit
- SHA: <hash>
- Message: <message>
- Author: <name>
- Timestamp: <ISO 8601>

## Activity
- Commits since last push: <n>
- Last push: <ISO 8601 | never>
- Days since last push: <n>
- Days since last commit: <n>

## CONTEXT.md
- Status: ok | drift_detected | missing
- Last modified: <ISO 8601>
- Discrepancies: <none | bulleted list of specific flags>

## Contributors
- <name>: <n> commits, last active <ISO 8601>
```

> There is no State or summary field. Rapstat records observable facts only. Interpretation is the responsibility of the reporting layer.

#### Two-Tier Content Model

The content of STATUS.md differs by trigger. This is by design.

| Trigger | Content |
|---|---|
| commit | Project state, file scan, CONTEXT.md validation. No commit SHA — the commit does not exist yet at pre-commit time. This version is committed into the repo and visible on the remote. |
| push | Full commit metadata — SHA, message, author, branch. Sprint-level activity. CONTEXT.md drift analysis. Contributors. Written to local filesystem and synced to central store. Not committed into the repo. |
| daemon | Same as push-level scan. Written to local filesystem and synced to central store. Catches projects where hooks were not triggered. |

A reader of the remote repo sees `trigger: commit` — accurate project state as of last commit, without full git metadata. A reader of the central store sees `trigger: push` or `trigger: daemon` — the richer record with full metadata and sprint context. Both are correct and complementary.

---

## 5. Components

### 5.1 Git Hook — pre-commit

Fires on every git commit, before the commit is finalized. Lightweight. No LLM calls. Non-blocking. This is the version of STATUS.md that gets committed into the repo and is visible on the remote.

**What is available at pre-commit time:**
- File system state — directory structure, file counts, recent changes
- Staged file list
- CONTEXT.md content and last-modified timestamp
- Git log up to the previous commit — branch, prior SHAs, author history

**What is NOT available at pre-commit time:**
- The current commit SHA — it does not exist yet
- The current commit message

**Responsibilities:**
- Scan project state and write STATUS.md with `trigger: commit`
- Validate CONTEXT.md exists and flag if missing
- Stage STATUS.md so it is included in the commit
- Elapsed time target: < 200ms

### 5.2 Git Hook — pre-push

Fires when a push is initiated, after all commits are finalized. Heavier scan. This is the sprint completion moment — the richest STATUS.md Rapstat produces. This version is written to the local filesystem and synced to the central store but is NOT committed into the repo.

**What becomes available at pre-push time that was not at pre-commit:**
- Full commit metadata for all commits in the push — SHA, message, author, timestamp
- Commits since last push — the complete sprint commit log
- Contributor activity across the sprint

**Responsibilities:**
- Write STATUS.md with `trigger: push` including full commit metadata
- Compare CONTEXT.md against commit history since last push — flag specific discrepancies
- Record contributor activity for the sprint period
- Sync STATUS.md payload to central store
- Exit 0 always — never block the push
- Elapsed time target: < 2s

### 5.3 Scheduled Scanning

Periodic scanning is handled by the OS scheduler — cron, launchd, or systemd timers — invoking `rapstat scan`. Rapstat has no daemon component. The human controls the schedule; Rapstat does the work when called.

**Example crontab entry:**
```
0 2 * * * /usr/local/bin/rapstat scan
```

This covers projects where hooks were not triggered and provides a nightly safety net without Rapstat owning a long-running process.

---

## 6. CLI Interface

| Command | Description |
|---|---|
| `rapstat init` | Install git hooks into .git/hooks/ for the current repo. Creates .rapstat/config.toml if absent. |
| `rapstat scan` | Manual scan of current project. Updates STATUS.md immediately. Useful after pulling on a new machine or as a cron target. |
| `rapstat status` | Print current STATUS.md to stdout in human-readable format. |
| `rapstat check` | Validate CONTEXT.md against git log and print discrepancies. Read-only, no writes. |

---

## 7. Rust Crate Dependencies

| Crate | Purpose |
|---|---|
| clap | CLI argument parsing and subcommand routing |
| git2 | libgit2 bindings — repo introspection, log, branch, commit metadata |
| serde | Serialization framework |
| serde_json | STATUS.md frontmatter and central store JSON payloads |
| toml | Config file parsing (.rapstat/config.toml) |
| walkdir | Recursive project directory scanning |
| chrono | Timestamps, duration calculations, ISO 8601 formatting |
| dirs | Cross-platform home directory and config path resolution |
| anyhow | Ergonomic error handling |
| colored | Terminal output coloring for rapstat status |

---

## 8. Configuration

Each project root contains a `.rapstat/config.toml`. Machine-level config lives at `~/.rapstat/config.toml`. Project config takes precedence.

```toml
[project]
name = "raptor-v2"

[hooks]
pre_commit = true
pre_push = true

[validation]
check_context_md = true
flag_only = true   # never modify CONTEXT.md
```

---

## 9. Build & Distribution

### 9.1 Cross-Compilation Targets

| Target | Machines |
|---|---|
| aarch64-apple-darwin | MacBook Pro (primary + getchkd), MacBook Air |
| x86_64-unknown-linux-gnu | System76 Linux inference machine |

### 9.2 Release Process

- GitHub Actions builds on tag push
- Produces binary artifacts for both targets
- GitHub Release attaches binaries
- Install script: `curl -fsSL https://rapstat.tq.internal/install.sh | sh`

### 9.3 Hook Installation

Hooks are stored in `.rapstat/hooks/` and tracked by git. `rapstat init` symlinks them into `.git/hooks/`. This means hook logic is version-controlled alongside the project and propagates automatically when other machines pull.

---

## 10. Out of Scope — v1.0

- Reporting, interpretation, or summarization — Rapstat writes facts, a separate tool reads them
- Central store aggregation or transport — a downstream tool owns that
- LLM-generated content of any kind in the critical path
- Writing or modifying an existing CONTEXT.md under any condition
- Inter-agent coordination or orchestration
- Daemon/long-running process — use cron or equivalent
- Any UI — Rapstat has no display layer
- Windows support
- Authentication or encryption of STATUS.md (internal tool, private repos)

---

## 11. Success Criteria

- `rapstat init` completes without error on all four dev machines
- pre-commit hook fires and updates STATUS.md in < 200ms on all machines
- pre-push hook fires, validates CONTEXT.md, and completes in < 2s
- `rapstat scan` runs cleanly when invoked from cron
- STATUS.md written by pre-push hook contains full commit metadata including SHA and message
- STATUS.md is never written by any process other than Rapstat
- CONTEXT.md is never modified by Rapstat under any condition
- A git push is never blocked by a Rapstat failure

---

## 12. Raptor Integration Notes

Rapstat is a Raptor-adjacent tool and should be built using the Raptor governance framework.

| Field | Value |
|---|---|
| Project type | Raptor-managed sprint project |
| QA | Joe — tranche-level QA per Raptor standard |
| Sprint size | Single sprint for MVP (hooks + CLI) |
| Phase 2 | Integration with downstream ingest/reporting tool |
| CONTEXT.md | Required at project root before first sprint |

Rapstat is itself a project that Rapstat will eventually observe. The first `rapstat init` on the Rapstat repo is a meaningful milestone.

---

*This document was produced via collaborative specification with Claude (Anthropic). It represents design intent as of April 2026 and should be treated as a living document until sprint assignment.*

---

### Revision History

| Version | Change |
|---|---|
| 1.1 | Initial draft |
| 1.2 | Removed daemon component (replaced by cron); removed central store (out of scope, handled by downstream tool); trimmed CLI commands accordingly |
