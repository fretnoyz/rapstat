# Rapstat — Project Context

## What This Is

Rapstat is a lightweight, single-binary observability tool for Texas Quantitative's development environment. It solves one problem: project state is currently distributed across multiple machines, multiple Claude accounts, and agent-generated documentation that is non-deterministically maintained.

**One job:** observe project state and write STATUS.md. Nothing else.

Rapstat is not a reporting tool, not an orchestrator, and not part of Raptor's execution chain. It watches Raptor. An agent cannot influence its own observer.

---

## Architecture Decisions (Finalized)

- **No daemon.** Scheduled scanning is handled by cron invoking `rapstat scan`. Rapstat has no long-running process.
- **No central store.** Transport and aggregation belong to a downstream tool. Rapstat writes STATUS.md locally and stops.
- **One writer.** STATUS.md is written exclusively by Rapstat. No other process touches it.
- **Reads CONTEXT.md, never writes it.** Rapstat may flag drift but does not modify agent-owned files.
- **Non-blocking hooks.** A Rapstat failure never prevents a commit or push.
- **No LLM in the critical path.** Deterministic output only.

---

## Targets

| Machine | Platform |
|---|---|
| MacBook Pro (primary) | macOS aarch64 |
| MacBook Pro (getchkd) | macOS aarch64 |
| System76 Linux | Ubuntu x86_64 |
| MacBook Air (secondary) | macOS aarch64 |

Cross-compilation targets: `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`

---

## STATUS.md Two-Tier Model

| Trigger | Content | Destination |
|---|---|---|
| `commit` | Project state, file scan, CONTEXT.md validation. No SHA (not yet available). | Committed into the repo. |
| `push` | Full commit metadata — SHA, message, author, branch. Sprint-level activity. CONTEXT.md drift analysis. | Local filesystem only. Not committed. |

---

## Sprint 1 — MVP

**Goal:** working binary with hooks + CLI. All four commands functional. Installs cleanly on all target machines.

### Tranche 1 — Core scan engine
- [x] `config.rs` — load `.rapstat/config.toml`, fall back to `~/.rapstat/config.toml`
- [x] `git.rs` — repo introspection via git2: branch, last commit, commits since push, contributor activity
- [x] `status_model.rs` — STATUS.md data model, serialization (YAML frontmatter + Markdown body)
- [x] `scan` command — wire config + git + status writer, write STATUS.md with `trigger: scan`

### Tranche 2 — Hooks
- [x] `pre-commit` hook script — invoke `rapstat scan --trigger commit`, stage STATUS.md
- [x] `pre-push` hook script — invoke `rapstat scan --trigger push`, write locally (do not stage)
- [x] `init` command — symlink hooks from `.rapstat/hooks/` into `.git/hooks/`, create config if absent

### Tranche 3 — Remaining CLI
- [x] `status` command — read and pretty-print STATUS.md using `colored`
- [x] `check` command — compare CONTEXT.md mtime against last commit timestamp, print discrepancies
- [x] `context_check.rs` — shared inspection logic extracted as a module

### Tranche 4 — QA & Distribution
- [x] `cargo install --path .` — binary on PATH, hooks operational
- [x] `git2` vendored feature — self-contained binaries, no system libgit2 dependency
- [x] GitHub Actions cross-compile workflow: `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`
- [ ] Test on all four machines
- [ ] Tagged release with binary artifacts

---

## Phase 2 (Post-MVP)

Integration with downstream ingest/reporting tool that will consume STATUS.md files from registered machines.

**rapview** — built and pushed to github.com/fretnoyz/rapview. FastAPI local dashboard with SSH pull, AI classification (blocked/active/stale), three-lane layout, and CONTEXT.md expandable drawer.

**Workspace:** `rapstat/rapstat.code-workspace` — multi-root VS Code workspace containing both rapstat and rapview. Open with File → Open Workspace from File.

---

## Current State

- Spec: v1.2 (finalized)
- Sprint 1: **complete** — all four tranches shipped
- v0.1.0 tagged and released; GitHub Actions workflow live
- Binary installed: MacBook Pro (primary) ✓, MacBook Pro (getchkd) ✓
- Binary NOT YET installed: System76 Linux, redteam1
- rapview v0.1.0 built and pushed — not yet installed or tested end-to-end
- Multi-root workspace: `rapstat/rapstat.code-workspace` covers both repos

## Next Steps

1. **Install rapstat on remaining machines**
   - System76 Linux: `sudo apt install -y libssl-dev pkg-config cmake zlib1g-dev && cargo install --git https://github.com/fretnoyz/rapstat --force`
   - redteam1: same (use `apt update --fix-missing` if cmake 404s)
   - Run `rapstat init` in each active project on those machines after install

2. **Init rapstat in all active projects on all machines**
   - For each project: `cd /path/to/project && rapstat init`
   - Confirm hooks fire on next commit in each project

3. **Install and test rapview on primary MacBook**
   - `cd ~/Documents/repos/rapview && python -m venv .venv && source .venv/bin/activate && pip install -e .`
   - Run `rapview` once to get config template, create `~/.rapview/config.toml`
   - Test localhost-only first (mbp-primary as 127.0.0.1)
   - Known issues to fix before first run: body parser for git metrics, missing `origin` field (see rapview CONTEXT.md)

4. **Add remote machines to rapview config once local test passes**
   - Consumes STATUS.md files from registered projects/machines
   - Also reads CONTEXT.md for human context
   - Produces human-readable summary across all tracked projects
   - Design TBD — start with a spec session similar to this one
