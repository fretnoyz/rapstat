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
- [ ] `config.rs` — load `.rapstat/config.toml`, fall back to `~/.rapstat/config.toml`
- [ ] `git.rs` — repo introspection via git2: branch, last commit, commits since push, contributor activity
- [ ] `status.rs` (lib) — STATUS.md data model, serialization (YAML frontmatter + Markdown body)
- [ ] `scan` command — wire config + git + status writer, write STATUS.md with `trigger: scan`

### Tranche 2 — Hooks
- [ ] `pre-commit` hook script — invoke `rapstat scan --trigger commit`, stage STATUS.md
- [ ] `pre-push` hook script — invoke `rapstat scan --trigger push`, write locally (do not stage)
- [ ] `init` command — symlink hooks from `.rapstat/hooks/` into `.git/hooks/`, create config if absent

### Tranche 3 — Remaining CLI
- [ ] `status` command — read and pretty-print STATUS.md using `colored`
- [ ] `check` command — compare CONTEXT.md mtime and content against recent commit log, print discrepancies

### Tranche 4 — QA & Distribution
- [ ] Test on all four machines
- [ ] GitHub Actions cross-compile workflow: `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`
- [ ] Tagged release with binary artifacts

---

## Phase 2 (Post-MVP)

Integration with downstream ingest/reporting tool that will consume STATUS.md files from registered machines.

---

## Current State

- Spec: v1.2 (finalized)
- Rust scaffold: in place — `cargo build` clean, all four CLI subcommands stub with `todo!()`
- Next: Tranche 1 — config loader and git introspection module
