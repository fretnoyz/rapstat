# rapview — Spec v1.1

**Date:** 2026-04-20  
**Status:** Draft  
**Companion to:** rapstat v0.1.0

---

## Purpose

`rapview` is a local web dashboard that aggregates STATUS.md files written by `rapstat` across multiple dev machines and presents a unified, live view. It runs on the operator's primary machine, SSHes into configured remotes on demand, and serves a browser-based card dashboard at `http://localhost:7474`.

---

## Design Axioms

1. **One job.** rapview reads STATUS.md files and displays them. It does not write anything, trigger scans, or modify remote state.
2. **SSH pull only.** No agent on the remote. No daemon. rapview opens a connection, reads the file, closes.
3. **Local-only server.** Binds to `127.0.0.1`. Not exposed to the network.
4. **No build step.** `pip install -r requirements.txt` + `rapview` — done.
5. **Graceful degradation.** If a machine is unreachable or a project has no STATUS.md, the card shows a clear error state rather than crashing.
6. **Project identity = git remote origin URL.** Same repo on two machines = one card with two rows.

---

## Architecture

```
rapview
├── rapview/
│   ├── __init__.py
│   ├── main.py          # FastAPI app, CLI entrypoint
│   ├── config.py        # Load ~/.rapview/config.toml
│   ├── ssh.py           # asyncssh connections, file reads
│   ├── parser.py        # STATUS.md YAML frontmatter + CONTEXT.md section extraction
│   ├── classifier.py    # AI classification: blocked / active / idle + pulse
│   ├── aggregator.py    # Group by origin URL, assign lanes, build card model
│   └── templates/
│       └── dashboard.html   # Jinja2 template (inline CSS + vanilla JS)
├── pyproject.toml
├── requirements.txt
└── README.md
```

---

## Configuration

Location: `~/.rapview/config.toml`

```toml
[server]
port = 7474
refresh_interval = 60       # seconds between auto-refreshes

[ssh]
default_user = "you"
key_path = "~/.ssh/id_ed25519"
connect_timeout = 5         # seconds

[[machine]]
name = "mbp-primary"
host = "127.0.0.1"          # localhost — reads directly, no SSH
user = "you"
project_roots = [
  "~/Documents/repos",
]

[[machine]]
name = "mbp-getchkd"
host = "100.x.x.x"          # Tailscale IP
user = "you"
project_roots = [
  "~/Documents/repos",
]

[[machine]]
name = "system76"
host = "100.x.x.x"
user = "you"
project_roots = [
  "~/repos",
  "~/work",
]

[[machine]]
name = "redteam1"
host = "100.x.x.x"
user = "kali"
project_roots = [
  "~/repos",
]
```

**Notes:**
- `project_roots` is a list of directories to walk for STATUS.md files. rapview looks one level deep (i.e., `~/repos/*/STATUS.md`).
- `localhost` machines skip SSH and read directly from the filesystem.
- `key_path` can be overridden per machine block.

---

## SSH Transport

- Library: `asyncssh`
- All machines are queried concurrently on each refresh (`asyncio.gather`)
- Each connection: SFTP read of `STATUS.md` (and optionally `CONTEXT.md`) for each project root discovered
- Discovery: list directory one level deep, filter for subdirs containing `STATUS.md`
- Connection errors are caught per-machine — one unreachable host does not block others
- Connections are not kept alive; open → read → close per refresh cycle

### Localhost shortcut

If `host == "127.0.0.1"` or `host == "localhost"`, skip SSH entirely. Read files directly using `pathlib`.

---

## Data Model

rapview parses the YAML frontmatter from STATUS.md. The fields it uses:

```
project.name
project.origin        # git remote origin — used as card identity key
git.branch
git.commits_since_push
git.days_since_last_commit
git.days_since_last_push
context_md.status     # ok | drift | missing
context_md.discrepancies  # list
rapstat.generated_at
rapstat.trigger
```

The parsed result is a `MachineProject`:

```python
@dataclass
class MachineProject:
    machine: str          # machine.name from config
    project_name: str
    origin: str           # canonical identity key
    branch: str
    commits_ahead: int
    days_since_commit: float
    days_since_push: float
    context_status: str   # ok | drift | missing
    discrepancies: list[str]
    generated_at: str     # ISO timestamp
    trigger: str
    last_commit_sha: str | None
    last_push_at: str | None
    context_sections: list[ContextSection]  # parsed from CONTEXT.md
    context_md_hash: str | None             # SHA-256 of raw CONTEXT.md content
    error: str | None     # set if STATUS.md missing or parse failed

@dataclass
class ContextSection:
    heading: str          # e.g. "Current State", "Next Steps"
    body: str             # raw Markdown text of the section
    auto_expand: bool     # True if heading matches "current state" or "next steps" (case-insensitive)
```

---

## AI Classification

`classifier.py` reads the CONTEXT.md sections for each `MachineProject` and returns a classification. This runs at refresh time and results are cached with the rest of the data.

**Input:** concatenation of `Current State` and `Next Steps` section bodies (if present), plus the project name and days_since_commit as context.

**Output:**
```python
@dataclass
class Classification:
    lane: str        # "blocked" | "active" | "stale"
    pulse: str       # one sentence, ≤ 100 chars
```

**Prompt (sent to LLM):**
```
You are summarizing a software project's current status for a developer dashboard.

Project: {project_name}
Days since last commit: {days_since_commit}

Context:
{current_state_text}
{next_steps_text}

Classify the project as exactly one of:
- blocked: explicitly waiting on something external, or a blocker is described
- active: work is underway with no blocker mentioned
- stale: no meaningful context or nothing in progress

Then write a single sentence (≤ 100 chars) describing what is happening or why it is blocked.

Respond in JSON: {"lane": "...", "pulse": "..."}
```

**Caching — AI pulse is NOT re-run if nothing has changed:**

Before calling the LLM, `classifier.py` checks whether any of the project's machine instances have changed since the last classification. A re-run is only triggered if, for at least one instance, either:
- `last_commit_sha` differs from the cached value, OR
- `last_push_at` differs from the cached value, OR
- `context_md_hash` differs from the cached value

If none of these have changed, the cached `Classification` is returned immediately. This means a full refresh cycle (SSH pull of fresh data) still runs on every timer tick, but the LLM is only called when there is actually new information.

The classification cache is stored in memory (not persisted to disk). It is cleared when rapview restarts.

**Rules override AI:**
- If `days_since_commit >= 14` → lane is forced to `stale` regardless of AI output
- If CONTEXT.md is missing on all instances → lane is `stale`, pulse is "No context available"
- If AI call fails or times out → lane is `stale`, pulse is "Classification unavailable"

**Providers (configured in `~/.rapview/config.toml`):**
```toml
[ai]
enabled = true
provider = "openai"          # or "ollama"
model = "gpt-4o-mini"        # or e.g. "llama3"
api_key_env = "OPENAI_API_KEY"   # env var name; only used for openai
ollama_base_url = "http://localhost:11434"  # only used for ollama
timeout = 10                 # seconds; classification skipped on timeout
```

If `ai.enabled = false`, all projects are classified as `active` with pulse from project name only (no LLM call).

---

## Aggregation

`aggregator.py` groups `MachineProject` instances by `origin` URL into `ProjectCard` objects, then assigns each card to a lane based on the classification of its instances.

```python
@dataclass
class ProjectCard:
    origin: str
    display_name: str               # last path component of origin
    instances: list[MachineProject] # one per machine where it exists
    lane: str                       # "blocked" | "active" | "stale"
    pulse: str                      # one-line summary from AI
    alert_level: str                # ok | warn | alert — from metrics rules
```

**Lane assignment:** if any instance is `blocked` → card is `blocked`. Else if any instance is `active` → `active`. Else `stale`.

**Alert level rules (independent of lane):**
- `alert` if any instance has `context_status == "missing"` or `commits_ahead >= 10` or `days_since_push >= 7`
- `warn` if any instance has `context_status == "drift"` or `commits_ahead >= 3` or `days_since_push >= 3`
- `ok` otherwise

Cards are sorted within each lane alphabetically by `display_name`.

---

## Dashboard UI

Served as a single Jinja2-rendered HTML page at `GET /`. Data refreshed via `GET /api/status` (returns JSON).

### Main layout — three lanes

The page is divided into three vertical sections, always in this order: **BLOCKED**, **ACTIVE**, **STALE**. Empty lanes are hidden.

```
┌─ BLOCKED ──────────────────────────────────────────────────────────────────┐
│ raptor-toolkit   Blocked: waiting on LLM API key rotation          ● warn  │
│ adh              Blocked: SSH auth failing on system76             ● alert │
└────────────────────────────────────────────────────────────────────────────┘

┌─ ACTIVE ───────────────────────────────────────────────────────────────────┐
│ rapstat          Designing rapview reporter spec                   ● ok    │
│ rapview          Spec in progress, implementation not yet started  ● ok    │
└────────────────────────────────────────────────────────────────────────────┘

┌─ STALE ────────────────────────────────────────────────────────────────────┐
│ llama.cpp        No commits in 14d                                 ● ok    │
│ crev             No commits in 31d                                 ● ok    │
└────────────────────────────────────────────────────────────────────────────┘
```

Each row is a summary line. Clicking a row expands the full card inline.

---

### Expanded card

Clicking a summary row expands to show the full card below it:

```
▼ rapstat   Designing rapview reporter spec                          ● ok

  ├──────────────┬──────────────┬──────────────┬────────┤
  │ machine      │ branch       │ ahead / push │ ctx    │
  ├──────────────┼──────────────┼──────────────┼────────┤
  │ mbp-primary  │ main         │ 0 / 0d       │ ✓      │  ▶ context
  │ system76     │ main         │ 2 / 1d       │ drift  │  ▶ context
  │ redteam1     │ feature/x    │ 5 / 3d       │ ✓      │  ▶ context
  └──────────────┴──────────────┴──────────────┴────────┘
  Updated: 2026-04-20 14:32 via scan
```

The `▶ context` button on each machine row expands the CONTEXT.md drawer for that machine.

---

### CONTEXT.md drawer (per machine row)

Expands below the machine row. Sections from CONTEXT.md are rendered as a collapsible tree:

```
  ▼ mbp-primary  main  0 ahead  ✓

      ▶ What This Is
      ▶ Architecture Decisions
      ▼ Current State                ← auto-expanded
          Sprint 1: complete
          v0.1.0 tagged and released...
      ▼ Next Steps                   ← auto-expanded
          1. Install on remaining machines
          2. Init in all active projects
          3. Spec reporter
      ▶ Targets
```

- Sections whose heading matches `current state` or `next steps` (case-insensitive substring) are expanded by default
- All other sections are collapsed by default
- If neither key section is found, all sections are expanded
- Section body rendered as HTML via `markdown-it` (client-side JS library, no server dep)
- Drawer height capped at 320px with internal scroll if content overflows
- If CONTEXT.md was not found on the machine, drawer shows: *"CONTEXT.md not present"*

---

### Detail window

Clicking the project name (not the expand arrow) opens the full detail view in a **new browser tab** via `window.open()`. This allows multiple project detail views to be open simultaneously and arranged across the screen.

The detail page is served at `GET /project/{project_id}` where `project_id` is a URL-safe slug derived from the origin URL (e.g. `fretnoyz-rapstat`).

The detail page contains:
- Project name + origin URL
- AI pulse and lane badge
- Full metrics table (all machine instances)
- CONTEXT.md section tree for each machine, all sections expanded by default
- A "Refresh" button that re-fetches just this project's data via `GET /api/project/{project_id}`
- No countdown timer (detail pages do not auto-refresh; they are snapshot views opened on demand)

The detail page is a standalone HTML document — it does not embed inside the main dashboard.

---

### Color coding

- Lane header: red (BLOCKED) / blue (ACTIVE) / grey (STALE)
- Alert dot on summary row: green (ok) / yellow (warn) / red (alert)
- `ctx` column: green ✓ / yellow "drift" / red "missing"
- `ahead` count: yellow if ≥ 3, red if ≥ 10
- `days since push`: yellow if ≥ 3d, red if ≥ 7d

### Header bar

```
rapview                    Last refresh: 14:32:01   [Refresh Now]   Next in: 00:47
```

- Countdown timer in JS; hits zero → auto-refresh
- Countdown pauses while a refresh is in flight; resumes when complete
- "Refresh Now" calls `POST /api/refresh` → awaits full SSH + AI cycle → returns updated JSON → page re-renders without full reload
- If a refresh is already in progress, button shows "Refreshing…" and is disabled

### Error state (unreachable machine or missing STATUS.md)

The machine row shows with a grey background and an inline message:

```
  │ system76     │ unreachable (timeout)                              │
```

The project card still appears in the lane it was last classified in (stale cache), or `stale` if never seen.

---

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Serve main dashboard HTML |
| GET | `/project/{project_id}` | Serve standalone detail page for one project |
| GET | `/api/status` | Return current cached card data as JSON |
| POST | `/api/refresh` | Trigger re-query of all machines + AI (if changed); return updated JSON |
| GET | `/api/project/{project_id}` | Return cached data for one project as JSON |

The server caches the last successful pull. `/api/status` returns the cache immediately. `/api/refresh` awaits the full SSH cycle before returning; AI is only called for projects whose commit SHA, push timestamp, or CONTEXT.md hash has changed. If a refresh is already in progress, `/api/refresh` waits for the in-flight one rather than launching a second (async lock).

---

## CLI

```sh
rapview                          # start server, open browser
rapview --port 8080              # override port
rapview --config ~/custom.toml   # override config path
rapview --no-open                # start server without opening browser
```

Entrypoint installed via `pyproject.toml` `[project.scripts]`.

---

## Dependencies

```
fastapi
uvicorn[standard]
asyncssh
jinja2
toml
openai          # for openai provider; not required if using ollama only
httpx           # for ollama provider (async HTTP)
```

`markdown-it-py` is **not** a server dependency — the dashboard uses the `markdown-it` JS library loaded from a CDN link in the HTML template, keeping the server install minimal.

Python ≥ 3.11.

---

## Project Structure (new repo: rapview)

```
rapview/              # repo root
├── rapview/          # package
│   ├── __init__.py
│   ├── main.py
│   ├── config.py
│   ├── ssh.py
│   ├── parser.py        # STATUS.md + CONTEXT.md parsing
│   ├── classifier.py    # AI classification (blocked / active / stale)
│   ├── aggregator.py
│   └── templates/
│       └── dashboard.html
├── pyproject.toml
├── requirements.txt
├── .rapstat/         # rapview is itself a rapstat-tracked project
└── README.md
```

---

## Out of Scope (v1.0)

- Authentication / HTTPS (local-only server)
- History / trend graphs
- Writing to remotes or triggering `rapstat scan` remotely
- Mobile layout
- Notifications / alerts beyond the dashboard
- Windows support
- AI-generated cross-machine divergence detection (same project, different CONTEXT.md on two machines)

---

## First-Run Behaviour

If `~/.rapview/config.toml` does not exist, rapview prints a message and a commented template to stdout, then exits with a non-zero code:

```
No config found at ~/.rapview/config.toml
Create it with the following template and edit to match your machines:

  [server]
  port = 7474
  ...
```

No interactive wizard. The user creates the file manually, which keeps the config explicit and auditable.
