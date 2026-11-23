# antik — backend architecture

Status: proposed · 21 November 2026 · M0–M3 implemented, M4–M5 planned
Companion documents: `.notes/backend.md` (the IPC contract), `PRODUCT.md` (product truth), `idea.md` §5.6 (build order), `DESIGN.md` (frontend system).

This document is the engineering plan for the Rust side of antik: crate layout, domain model, storage, IPC, concurrency, security, testing and delivery order. It assumes the contract in `.notes/backend.md` and does not renegotiate product behaviour.

---

## 1. Scope

### 1.1 What the backend is

A library of Rust services running in the Tauri process, owning all durable state on one Linux machine, reached from the frontend only through typed IPC commands. No HTTP server, no daemon, no port, no sync.

### 1.2 Invariants the backend must enforce

These are the contract's "rules the backend owns", restated as testable properties:

| # | Invariant | Enforced in |
| --- | --- | --- |
| I1 | No model output can set a gate to `passed` or `overridden`. | Gate service + scorer port |
| I2 | A handoff is not returned until the current session's recall is stored. | Session service + query layer |
| I3 | Check results are written only by the runner, never by a client payload. | Command surface excludes check results |
| I4 | Every mutation appends exactly one event with a backend timestamp, atomically with the state change. | `Store::write` transaction |
| I5 | Events are append-only at the database level. | SQLite triggers |
| I6 | Notebook and working notes never enter gate scoring or review figures. | Scorer inputs + metrics SQL |
| I7 | The model reads only allowlisted paths, confined to the project root, and cannot write. | `cap-std` + context builder |
| I8 | Phase transitions cannot skip or reverse. | Domain state machines |

### 1.3 Performance budgets

Measured on the owner's machine, release build, warm cache:

| Operation | Budget (p95) | Basis |
| --- | --- | --- |
| `bootstrap` (shell launch) | ≤ 25 ms | 20 projects, 10k events, ≤ 8 queries total |
| Read command (`get_task`, `get_project`) | ≤ 8 ms | prepared statements, no N+1 |
| Write command | ≤ 12 ms | one `BEGIN IMMEDIATE` + one file projection |
| Full markdown projection (one project) | ≤ 20 ms | 100 files, atomic writes |
| `review` (30/90 days) | ≤ 15 ms | indexed aggregates over `events` |
| Check run | process time + ≤ 5 ms overhead | no shell, capped output |

### 1.4 Non-goals for v1

- No HTTP/JSON server, no localhost port. The contract's routes become Tauri commands.
- No sync, multi-user, cloud or encryption at rest.
- No model write access and no agent loop. The model is a text-in/text-out advisor behind one port.
- No full event sourcing. State tables are the read model; the event log is the immutable audit trail and the source for metrics.
- No in-app git operations. Files are written commit-ready; the owner commits.
- No plugin system.

---

## 2. System shape

```
┌──────────────────────────── frontend (TS) ────────────────────────────┐
│  src/api.ts  ·  typed invoke wrapper + cache  ·  listen() for jobs    │
└───────────────────────────────┬───────────────────────────────────────┘
                                │ Tauri IPC (commands + events)
┌───────────────────────────────▼───────────────────────────────────────┐
│ src-tauri  (binary)                                                   │
│   commands/*  → validate DTO → spawn_blocking → service → DTO/error   │
│   AppState { App, Jobs, Model, Clock }  ·  capabilities  ·  tracing   │
└───────────────────────────────┬───────────────────────────────────────┘
┌───────────────────────────────▼───────────────────────────────────────┐
│ antik-app  (use cases, ports)                                         │
│   ProjectService · TaskService · GateService · SessionService          │
│   ConceptService · ReviewService · SettingsService · DrillService      │
└───────────────────────────────┬───────────────────────────────────────┘
┌───────────────────────────────▼───────────────────────────────────────┐
│ antik-domain  (pure)                                                  │
│   entities · ids · state machines · gate rubric · spacing · recall     │
└───────────────────────────────┬───────────────────────────────────────┘
                                │ ports implemented by
┌───────────────────────────────▼───────────────────────────────────────┐
│ antik-infra                                                           │
│   sqlite (rusqlite) · markdown (cap-std) · model (reqwest)             │
│   runner (tokio::process) · clock · filesystem index                  │
└───────────────────────────────────────────────────────────────────────┘
```

Dependency direction is strictly downward. `antik-domain` has no IO, no async, no `tokio`, no `tauri`. That is what makes it cheap to test exhaustively.

### 2.1 Workspace layout

Move to a Cargo workspace; the lockfile moves to the repository root.

```
Cargo.toml                     # [workspace] members, [workspace.dependencies]
rust-toolchain.toml            # pinned stable + rustfmt, clippy
crates/
  antik-domain/                # entities, policies, errors — no deps beyond serde/time
  antik-app/                   # use cases, ports, DTOs
  antik-infra/                 # sqlite, markdown, model, runner
crates/antik-infra/migrations/ # forward-only SQL, embedded via include_str!
src-tauri/                     # Tauri binary, commands, capabilities
```

`src-tauri` keeps package name `antik`; it depends on `antik-app` and `antik-infra`. Nothing else imports `antik-infra`.

Why four crates rather than one: the domain is the part that will be unit- and property-tested hardest, and it should not be able to reach a database or a Tauri type. If the split ever feels heavy, `antik-domain` and `antik-app` can merge without touching the adapters. Do not merge infra into the binary.

### 2.2 Dependencies

Pinned in `[workspace.dependencies]`, updated deliberately.

| Crate | Purpose | Note |
| --- | --- | --- |
| `tauri` 2 | shell, IPC, state | `withGlobalTauri: false` |
| `tokio` | runtime, process, time, sync | `rt-multi-thread`, `process`, `time`, `sync` |
| `rusqlite` | SQLite | `bundled` so the binary is self-contained |
| `r2d2` + `r2d2_sqlite` | sync read pool | matches the synchronous service layer |
| `rusqlite_migration` | forward-only migrations | embedded SQL |
| `cap-std` | capability-scoped filesystem | no path escapes, no symlink tricks |
| `serde`, `serde_json` | DTOs, event payloads | `serde_json` only for event payloads |
| `serde_yaml_ng` | frontmatter | strict parse, unknown keys rejected |
| `ulid` | record ids | sortable, stable across file renames |
| `time` | timestamps | `Rfc3339` + `serde`; store epoch millis |
| `thiserror` | typed errors | domain/app/infra layers |
| `tracing`, `tracing-subscriber`, `tracing-appender` | logs | rolling file + spans per command |
| `reqwest` | model transport | `rustls`, no default features, timeouts |
| `futures` | `BoxFuture` in object-safe ports | model and runner traits are `dyn` |
| `tokio-util` | `CancellationToken` | cancels runs and model calls |
| `specta` + `tauri-specta` 2 | generated TS bindings | replaces hand-written `src/data.ts` |
| `keyring` | API key storage | OS keyring, not SQLite |
| dev: `tempfile`, `proptest`, `insta`, `criterion` | tests, snapshots, benches | |

Rejected alternatives: `sqlx` (async SQLite is a thread pool underneath; compile-time macros require a live DB or offline metadata — cost without benefit here), `diesel` (schema/ORM weight), `notify` file watching in v1 (hash-on-open is enough and avoids feedback loops), `uuid` (ULID sorts).

---

## 3. Domain layer

### 3.1 Types

Newtype every identifier; never pass raw strings between layers.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ProjectId(Ulid);
pub struct TaskId(Ulid);
pub struct GateId(Ulid);
pub struct ConceptId(Arc<str>);   // "K-041": human-authored, stable
pub struct EventId(Ulid);

/// Milliseconds since the Unix epoch, UTC. Sorting and display are separate concerns.
pub struct Timestamp(i64);
```

Enums, not strings: `Phase { Plan, Implement, Verify, Reflect }`, `GateStatus { NotYet, Due, Passed, Failed, Overridden }`, `ConceptStatus { Passed, Weak, Due }`, `HelpLevel { Full, Hint, None }`, `CheckResult { Pass, Fail, Pending, Done }`.

`#[serde(rename_all = "snake_case")]` at the DTO boundary so the TS types stay flat strings.

### 3.2 State machines

Each aggregate exposes transitions that return the events they justify. No setter mutates a field directly.

```rust
impl Task {
    pub fn write_prediction(&mut self, text: PredictionText, at: Timestamp)
        -> Result<Vec<DomainEvent>, DomainError>;

    pub fn approve_plan(&mut self, at: Timestamp)
        -> Result<Vec<DomainEvent>, DomainError>;      // requires prediction, phase == Plan

    pub fn close_implement(&mut self, at: Timestamp)
        -> Result<Vec<DomainEvent>, DomainError>;      // Implement -> Verify

    pub fn close_for_reflect(&mut self, at: Timestamp, gates: &[Gate])
        -> Result<Vec<DomainEvent>, DomainError>;      // every gate resolved
}
```

Rules:

- `Plan → Implement` only with a non-empty prediction and an approved plan. The mock approves with a checkbox; the backend requires the prediction record.
- `Implement → Verify` is idempotent-safe: repeating it returns the current phase, not an error, so a double click is harmless.
- `Verify → Reflect` requires all gates attached to the task to be `Passed` or `Overridden`. The check happens here, not in a view.
- `Reflect` is terminal for the task; a closed task may still be read.
- Gate: `NotYet → Due` when the task enters verify; `Due|Failed → Passed` only via `GateAttempt` scoring; `Due|Failed → Overridden` only with a reason of at least one sentence; `Passed|Overridden` are terminal.
- Session: `Closed → RecallStored → Open → Closed` (closing happens with the handoff).

Every transition is a pure function over the aggregate; `proptest` drives random transition sequences and asserts no illegal jump is ever accepted.

### 3.3 Pure policies

- **Gate rubric.** A gate stores the criteria it is scored against at draft time. `crates/antik-domain/src/gate.rs` owns `GateRubric { min_words, criteria: Vec<Criterion { id, label, expect, min_hits }> }`: an answer passes when it meets the word floor and every criterion reaches its hit count, matching expected terms and their singular forms. The rubric is data stored as `rubric_json`, so scoring never needs a model at answer time. M3 drafts with an app-authored rubric (`GateDraft::from_task` or `GateDraft::from_concept`); M5 may add model drafting, but the scoring function stays deterministic and app-owned.
- **Concept-driven drafting.** `draft_gate` picks the first concept attached to the task that is `due`, `weak`, or past its `due_on`, and drafts against that concept's name and definition. A passing gate on a linked concept is also an explain-back: the concept's `seen`, `status`, `last_reviewed_at` and `due_on` move, and a `concept_reviews` row is written in the same transaction. A task with no due concept gets a task-based gate, so familiar work does not pay for a check.
- **Recall match.** Tokenise (lowercase, strip punctuation, drop tokens ≤ 3 chars, dedupe), then count a handoff line as matched when at least two of its tokens appear in the recall. Pure, snapshot-tested.
- **Spacing.** `next_due(now, outcome, seen) -> Timestamp`: a passing explanation moves 21 days out, a thin one stays due tomorrow. One function, one table-driven test.
- **Review figures.** Not domain code: SQL aggregates over `events`, because the figure must survive a task being edited or deleted.

### 3.4 Events

```rust
pub enum EventKind {
    ProjectCreated, BriefEdited, NoteAdded, NoteDeleted,
    TaskPlanned, TaskSwitched, PredictionWritten, PlanApproved, AttemptRecorded,
    ImplementClosed, GateDrafted, GateAttempted, GateOverridden, HintUsed,
    CheckRun, CheckResult, HandoffWritten, RecallStored, SessionOpened, SessionClosed,
    DecisionRecorded, QuestionRecorded, QuestionAnswered,
    ConceptReviewed, HelpChanged, DrillRun, SettingChanged, ReadScopeChanged,
}
```

Payloads are `#[serde(tag = "type", content = "data")]` with a `v: u32` field. Payloads are versioned so old events stay readable when a shape changes. Notes and handoff *text* are not copied into event payloads (I6); events carry ids and lengths instead.

---

## 4. Application layer

### 4.1 Synchronous services

Services are synchronous. SQLite work is microseconds; process and HTTP are the only I/O that earns an await. Tauri commands call `spawn_blocking` once at the boundary, so the async runtime is never blocked and the service layer needs no `async-trait` for storage.

```rust
pub struct TaskService<S: Store, F: ProjectFiles, C: Clock, I: IdGen, N: Notifier> { ... }

impl TaskService<..> {
    pub fn record_attempt(&self, cmd: RecordAttempt) -> Result<TaskView, AppError> {
        let at = self.clock.now();
        let id = self.ids.next();
        let view = self.store.write(|tx| {
            let mut task = tx.task(&cmd.task_id)?;
            let events = task.record_attempt(AttemptText::parse(cmd.text)?, at, id)?;
            tx.save_task(&task)?;
            for e in events { tx.append(e); }
            tx.task_view(&cmd.task_id)          // same transaction, consistent read
        })?;
        self.files.project_task(&view.project_id, &view.task)?;   // projection after commit
        self.notifier.changed(&view);
        Ok(view)
    }
}
```

Ordering rule: **commit the database first, project to markdown second.** A failed projection is logged and retried; it never invalidates a committed mutation. The inverse (file first, database second) can lose events.

### 4.2 Ports

```rust
pub trait Store: Send + Sync {
    /// Borrows a pooled connection for one query batch.
    fn read<T>(&self, f: impl FnOnce(&ReadConn<'_>) -> Result<T, AppError>) -> Result<T, AppError>;
    /// Runs `f` in one `BEGIN IMMEDIATE`, appends queued events, bumps revision, commits.
    fn write<T>(&self, f: impl FnOnce(&mut WriteTx<'_>) -> Result<T, AppError>) -> Result<T, AppError>;
}

pub trait ProjectFiles: Send + Sync {
    fn ensure_root(&self, root: &ProjectRoot) -> Result<(), FileError>;
    fn project_task(&self, project: &ProjectId, task: &TaskView) -> Result<(), FileError>;
    fn project_brief(&self, project: &ProjectId, body: &str) -> Result<(), FileError>;
    fn project_note(&self, project: &ProjectId, note: &Note) -> Result<(), FileError>;
    fn import_changed(&self, project: &ProjectId) -> Result<Vec<ExternalEdit>, FileError>;
}

pub trait Clock: Send + Sync { fn now(&self) -> Timestamp; }
pub trait IdGen: Send + Sync { fn next(&self) -> Ulid; }
pub trait Notifier: Send + Sync { fn changed(&self, view: &ChangedView); fn emit(&self, e: UiEvent); }

pub trait Model: Send + Sync {
    fn draft_gate(&self, ctx: GateContext) -> BoxFuture<'_, Result<GateDraft, ModelError>>;
    fn critique_plan(&self, ctx: PlanContext) -> BoxFuture<'_, Result<Critique, ModelError>>;
    fn health(&self) -> BoxFuture<'_, ModelHealth>;
}

pub trait Runner: Send + Sync {
    fn run(&self, spec: CheckSpec, cancel: CancellationToken) -> BoxFuture<'_, CheckOutcome>;
}
```

`ReadConn` and `WriteTx` expose query methods (`bootstrap`, `project_view`, `task_view`, `events`, `review`, `concepts`). They are hand-written SQL returning assembled DTOs in one round trip each; no repository returns objects that then trigger lazy loads. `StoreError` is converted into `AppError` at the transaction boundary.

### 4.3 Use cases

One method per contract endpoint, grouped by service:

`ProjectService` (bootstrap, list, create, brief, notes) · `TaskService` (plan, prediction, approve, notes, attempts, close) · `GateService` (draft, answer, hint, override) · `SessionService` (start, recall, handoff) · `ConceptService` (list, review, help) · `ReviewService` · `SettingsService` (model, allowlist) · `DrillService`.

The IPC command is a thin adapter: parse DTO → call service → map error. No SQL and no business rule in `src-tauri`.

---

## 5. Storage

### 5.1 Why SQLite, and how

One file per installation: `app_data_dir()/antik.db`. `rusqlite` with `bundled`; read pool via `r2d2_sqlite` (4 connections); a single writer connection behind a `std::sync::Mutex` — writes are synchronous, so the mutex is never held across an await.

Connection setup, applied to every connection:

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA temp_store = MEMORY;
PRAGMA cache_size = -16000;   -- 16 MiB
```

Rules:

- `prepare_cached` for every statement; statements are reused per connection.
- Explicit column lists. No `SELECT *`.
- Reads never open a transaction longer than a single query batch; metrics queries are read-only.
- Writes use `BEGIN IMMEDIATE`, so a second writer fails fast instead of deadlocking mid-transaction.
- All ids are text ULIDs; all timestamps are `INTEGER` epoch millis; all enums are text with a `CHECK` constraint. A schema that can only hold valid values is cheaper than validation code.

### 5.2 Schema (core)

```sql
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
-- schema_version, revision, installation_id, last_projection_at

CREATE TABLE projects (
  id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL,
  root_path TEXT NOT NULL, brief_excerpt TEXT NOT NULL DEFAULT '',
  current_task_id TEXT, created_at INTEGER NOT NULL, archived_at INTEGER,
  revision INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE tasks (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL, lane TEXT NOT NULL CHECK (lane IN ('learn','verify')),
  phase TEXT NOT NULL CHECK (phase IN ('plan','implement','verify','reflect')),
  batch TEXT NOT NULL DEFAULT '', scope TEXT NOT NULL DEFAULT '',
  prediction TEXT NOT NULL DEFAULT '', prediction_at INTEGER,
  plan_approved_at INTEGER, started_at INTEGER NOT NULL, finished_at INTEGER,
  minutes INTEGER NOT NULL DEFAULT 0, revision INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX tasks_by_project ON tasks(project_id, phase, started_at DESC);

CREATE TABLE attempts (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  at INTEGER NOT NULL, text TEXT NOT NULL
);

CREATE TABLE gates (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  concept_id TEXT REFERENCES concepts(id) ON DELETE SET NULL,
  n INTEGER NOT NULL, question TEXT NOT NULL, hint TEXT NOT NULL DEFAULT '',
  rubric_json TEXT NOT NULL, status TEXT NOT NULL CHECK (status IN
    ('not_yet','due','passed','failed','overridden')),
  answer TEXT, override_reason TEXT, hint_used INTEGER NOT NULL DEFAULT 0,
  evaluated_at INTEGER, drafted_by TEXT NOT NULL
);

CREATE TABLE gate_attempts (
  id TEXT PRIMARY KEY, gate_id TEXT NOT NULL REFERENCES gates(id) ON DELETE CASCADE,
  at INTEGER NOT NULL, answer TEXT NOT NULL, passed INTEGER NOT NULL, note TEXT NOT NULL
);

CREATE TABLE checks (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label TEXT NOT NULL, detail TEXT NOT NULL, spec_json TEXT NOT NULL,
  result TEXT NOT NULL CHECK (result IN ('pass','fail','pending','done')),
  evaluated_at INTEGER, last_run_id TEXT
);

CREATE TABLE check_runs (
  id TEXT PRIMARY KEY, task_id TEXT NOT NULL, started_at INTEGER NOT NULL,
  finished_at INTEGER, status TEXT NOT NULL CHECK (status IN ('running','done','failed','cancelled'))
);
CREATE TABLE check_results (
  run_id TEXT NOT NULL REFERENCES check_runs(id) ON DELETE CASCADE,
  check_id TEXT NOT NULL, exit_code INTEGER, result TEXT NOT NULL,
  duration_ms INTEGER NOT NULL, stdout_tail TEXT NOT NULL DEFAULT '',
  stderr_tail TEXT NOT NULL DEFAULT '', truncated INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (run_id, check_id)
);

CREATE TABLE concepts (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, area TEXT NOT NULL,
  definition TEXT NOT NULL DEFAULT '', seen INTEGER NOT NULL DEFAULT 0,
  help TEXT NOT NULL CHECK (help IN ('full','hint','none')) DEFAULT 'full',
  status TEXT NOT NULL CHECK (status IN ('passed','weak','due')) DEFAULT 'due',
  last_reviewed_at INTEGER, due_on INTEGER
);
CREATE INDEX concepts_due ON concepts(status, due_on);
CREATE TABLE task_concepts (
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  concept_id TEXT NOT NULL REFERENCES concepts(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, concept_id)
);
CREATE TABLE concept_reviews (
  id TEXT PRIMARY KEY, concept_id TEXT NOT NULL REFERENCES concepts(id) ON DELETE CASCADE,
  at INTEGER NOT NULL, answer TEXT NOT NULL, verdict TEXT NOT NULL CHECK (verdict IN ('pass','thin')),
  next_due INTEGER NOT NULL
);

CREATE TABLE decisions (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, task_id TEXT,
  at INTEGER NOT NULL, title TEXT NOT NULL, why TEXT NOT NULL, cost TEXT NOT NULL
);
CREATE TABLE questions (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, at INTEGER NOT NULL,
  text TEXT NOT NULL, answer TEXT, answered_at INTEGER
);

CREATE TABLE handoffs (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL, task_id TEXT,
  at INTEGER NOT NULL, next TEXT NOT NULL, blocking TEXT NOT NULL, thinking TEXT NOT NULL
);
CREATE TABLE sessions (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL,
  started_at INTEGER NOT NULL, closed_at INTEGER,
  recall_text TEXT, matched_lines INTEGER
);

CREATE TABLE events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  id TEXT NOT NULL UNIQUE,
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  actor TEXT NOT NULL CHECK (actor IN ('user','app','runner','model')),
  project_id TEXT, task_id TEXT,
  payload TEXT NOT NULL,
  schema_version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX events_by_project ON events(project_id, seq DESC);
CREATE INDEX events_by_kind ON events(kind, seq DESC);

CREATE TRIGGER events_no_update BEFORE UPDATE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;

CREATE TABLE file_index (
  path TEXT PRIMARY KEY, project_id TEXT NOT NULL,
  kind TEXT NOT NULL, record_id TEXT, content_hash TEXT NOT NULL,
  exported_at INTEGER NOT NULL
);

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE allowlist (path TEXT PRIMARY KEY, added_at INTEGER NOT NULL);

CREATE TABLE drills (
  id TEXT PRIMARY KEY, skill TEXT NOT NULL, name TEXT NOT NULL,
  command TEXT NOT NULL, last_run_at INTEGER, attempts INTEGER NOT NULL DEFAULT 0,
  passed INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE drill_runs (
  id TEXT PRIMARY KEY, drill_id TEXT NOT NULL REFERENCES drills(id) ON DELETE CASCADE,
  at INTEGER NOT NULL, exit_code INTEGER, duration_ms INTEGER NOT NULL, passed INTEGER NOT NULL
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('checks','drill','model')),
  status TEXT NOT NULL CHECK (status IN ('running','done','failed','cancelled')),
  started_at INTEGER NOT NULL, finished_at INTEGER, payload_json TEXT NOT NULL
);
```

Notes on the schema:

- `events` has no foreign keys on purpose: the audit trail outlives the records it describes.
- `handoffs` and `sessions` are separate so the withholding rule (I2) is a join, not an inference.
- `file_index` is the only link between the database and the filesystem; it makes "was this file changed outside the app?" a hash comparison.

### 5.3 Review metrics

Computed from `events`, never from mutable task rows, so a reopened or edited task cannot rewrite history.

```sql
-- lead time: first task_planned to handoff_written, per task
SELECT task_id,
       (MAX(CASE WHEN kind = 'handoff_written' THEN at END) -
        MAX(CASE WHEN kind = 'task_planned'   THEN at END)) AS lead_ms
FROM events
WHERE project_id = ?1 AND at >= ?2
GROUP BY task_id;
```

Rework counts `attempt_recorded`; gate pass rate counts `gate_attempted` and `gate_overridden`; bypasses are overrides with a reason. Each figure ships with its basis string, as the UI already expects.

### 5.4 Migrations, backup, recovery

- Forward-only SQL in `migrations/0001_init.sql`, `0002_…`; embedded and applied at startup inside one transaction using `rusqlite_migration`. `meta.schema_version` must equal the binary's expected version; a newer database is refused with a clear error rather than guessed at.
- Before any migration, `VACUUM INTO backups/antik-<version>-<UTC>.db`; keep the last three.
- On startup, any `jobs` row left `running` is marked `failed` with reason `interrupted`, and its task's matters are flagged in the UI. This is the only automatic correction the backend performs.
- On clean shutdown: `PRAGMA wal_checkpoint(TRUNCATE)`.

### 5.5 Markdown projection and the import lane

SQLite is the system of record. Markdown is a live projection kept current by `ProjectFiles`, so the folder is always readable and commit-ready.

The projects root is chosen once on first run (default `~/antik`), stored in `settings`, and never embedded in a document. Each project is a directory under it; renaming the directory is safe because every document carries its id.

Directory layout per project:

```
<project>/
  brief.md
  notes/<ulid>.md
  decisions/<ulid>.md
  questions/<ulid>.md
  tasks/<ulid>.md
  glossary.md            # projection of the concept map
```

Frontmatter, strict and versioned:

```yaml
---
antik: 1
id: 01J8Z6Q2V3
kind: note
project: 01J8Z5...
created: 2026-11-21T10:04:11Z
title: Timing note
---
```

Ownership matrix:

| Content | Read in app | Written by app | External edits |
| --- | --- | --- | --- |
| `brief.md` body | yes | yes | **imported** |
| `notes/*.md` body + title | yes | yes | **imported** |
| `decisions/*.md` | yes | append | **imported** (new files) |
| `questions/*.md` | yes | yes | **imported** |
| `tasks/*.md` | yes | yes | ignored, replaced on next projection |
| `glossary.md` | yes | yes | ignored, replaced |

The import lane is deliberate: those four document kinds are what "open it in Obsidian" means. Before any write to one of them, `import_changed` compares `file_index.content_hash` with the file on disk; a mismatch is parsed and applied as a domain edit event (`brief_edited`, `note_added`, …) with `actor = user`, timestamped at import, then re-projected. Parsing is strict: unknown `kind`, unknown schema version or a missing id is reported as `invalid_document` with the path, never guessed at.

Conflict policy: a write command that replaces text takes an optional `expected_revision`. If an external import landed since the UI read the record, the command fails with `conflict` and the UI reloads. Appends (notes, decisions, questions, attempts) never conflict.

### 5.6 File safety

- All filesystem work goes through `cap_std::fs::Dir` opened once per project root. Paths are relative and validated; `..`, absolute paths, symlinks that resolve above the root and non-UTF-8 names are rejected by the capability layer (`openat2` + `RESOLVE_BENEATH` on Linux). This is the same primitive the model's read scope uses, so there is one confinement implementation, not two.
- Pin a patched `cap-std` and forbid its `manually::*` APIs: advisory GHSA-hp8f-xmx4-4qrg (August 2026) covers a trailing-slash symlink escape in the non-`openat2` fallback path. Add a regression test that plants a symlink with a trailing slash and asserts the open fails.
- Writes are atomic: write `.<name>.tmp` in the same directory, `fsync` the file, `rename`, `fsync` the directory. Leftover `.tmp` files are ignored on scan and removed on startup.
- File operations run inside `spawn_blocking` from the command layer; `ProjectFiles` stays synchronous.
- Text is normalised to LF on write; CRLF and BOM are accepted on import and normalised. Notes are capped at 64 KiB and rejected beyond that with a clear error.

---

## 6. IPC surface

Commands replace the contract's routes one-to-one. Each command is `#[tauri::command]`, `#[specta::specta]`, returns `Result<Dto, AppErrorDto>` and is registered in one `commands::collect()` so bindings cannot drift.

| Group | Commands |
| --- | --- |
| Workspace | `bootstrap()`, `export_store()` |
| Projects | `list_projects()`, `create_project(input)`, `get_project(id)`, `update_brief(id, body, expected_revision?)`, `open_project_folder(id)` |
| Notebook | `list_notes(project_id)`, `add_note(project_id, title?, text)`, `delete_note(project_id, note_id)` |
| Tasks | `get_task(id)`, `create_task(project_id, input)`, `write_prediction(task_id, text)`, `approve_plan(task_id)`, `add_task_note(task_id, text)`, `record_attempt(task_id, text)`, `close_implement(task_id)`, `write_handoff(task_id, three_lines)` |
| Checks | `add_check(task_id, input)`, `remove_check(check_id)`, `run_checks(task_id) -> run_id`, `get_run(run_id)`, `cancel_run(run_id) -> bool` |
| Gates | `list_gates(task_id)`, `draft_gate(task_id)`, `answer_gate(gate_id, answer)`, `use_hint(gate_id)`, `override_gate(gate_id, reason)` |
| Session | `start_session(project_id)`, `submit_recall(project_id, text)`, `get_handoff(project_id)`, `close_session(project_id)` |
| Concepts | `list_concepts(status)`, `create_concept(input)`, `attach_concept(task_id, concept_id)`, `detach_concept(task_id, concept_id)`, `review_concept(concept_id, answer)`, `set_help(concept_id, level)` |
| Decisions / questions | `list_decisions(project_id)`, `add_decision(...)`, `list_questions(project_id)`, `add_question(...)`, `answer_question(id, text)` |
| Review / log | `review(days)`, `list_events(filter)` |
| Drills | `list_drills()` (bootstrap), `add_drill(input)`, `run_drill(drill_id) -> DrillOutcome` |
| Settings | `get_settings()`, `set_model(choice)`, `set_model_endpoint(url, name)`, `add_allowlist_path(path)`, `remove_allowlist_path(path)`, `model_health()`, `critique_plan(task_id)` |

Rules for every command:

- Payloads are typed structs; free-form strings are length-checked before they reach a service (note 64 KiB, answer 8 KiB, reason 2 KiB, title 200 chars, path 4096 bytes, ≤ 64 allowlist entries).
- Errors are one shape: `{ code, message, detail?, retryable }` with codes `not_found`, `invalid`, `conflict`, `locked`, `model_unavailable`, `runner_timeout`, `io`, `internal`. Internals (SQL text, absolute paths, stack) never cross IPC.
- A mutation returns the changed aggregate and the new `revision`, so the UI patches without a refetch.
- Job commands return ids immediately; progress and completion arrive as events: `antik://job-started`, `antik://job-finished`, `antik://external-change`, `antik://revision`.
- The DTOs live in `antik-app::dto`; `src/data.ts` is deleted and `src/bindings.ts` is generated.

The Tauri state is a single `AppState`:

```rust
// As built: the services are the state, and the model is one of their ports.
type Services = antik_app::App<Db>;  // store, clock, ids, files, runner, model
tauri::State<Services>;              // managed state for every command
tauri::State<Arc<JobManager>>;       // two process slots, one cancel token per run
```

---

## 7. Concurrency and jobs

- **One writer.** `Store::write` serialises on a mutex around the writer connection. For a single-user desktop app this is contention-free and makes "state + event atomically" trivially true.
- **Reads in parallel.** The read pool is sized at 4; `bootstrap` and screen reads may run concurrently with a write but see a consistent snapshot thanks to WAL.
- **Blocking work off the runtime.** Every sync service call is wrapped in `tauri::async_runtime::spawn_blocking`. No `std::sync` guard is held across an await.
- **Jobs.** `JobManager` owns a `Semaphore(2)` (at most two child processes), a `HashSet<TaskId>` for dedupe (one check run per task), a `CancellationToken` per job, and a default timeout of 120 s. It records the job row before spawning, updates it on completion, appends the event, and emits `job-finished`.
- **Process execution.** `tokio::process::Command` with an argv array — never a shell string. `kill_on_drop(true)`; on Unix the child is placed in its own process group and the group is killed on timeout; on Windows a job object is used in a later cross-platform pass. `stdout`/`stderr` are captured through a capped reader (64 KiB tail, `truncated` flag), because an unbounded pipe is a memory bug.
- **Program allowlist.** A check's `spec_json` names a program from a fixed set (`cargo`, `bun`, `npm`, `node`, `tsc`, `pytest`, `go`). The runner resolves the binary once, refuses anything outside the set, and runs with `cwd` confined to the project root.
- **Cancellation.** Closing the window or leaving the task cancels the token; the job row is marked `cancelled`, and a cancelled run never writes results (I3).

---

## 8. Model integration

```rust
pub enum ModelChoice { Local, Api, None }       // exactly the three UI states

pub struct GateContext {
    pub title: String,
    pub scope: String,
    pub prediction: String,
    pub concept: Option<ConceptRecord>,         // name and definition, never the notes
    pub files: Vec<ScopedFile>,                 // allowlisted, confined, size-capped
}
```

- **One lane, on this machine** (M5 as built). Only `Ollama` is attached; `Api` is a stored choice that reports `model_unavailable` instead of opening an egress. `set_model_endpoint` accepts `http://` on `127.0.0.1`, `localhost` or `[::1]` and refuses every other host, so a typo cannot send a project elsewhere. The target (base URL + model name) crosses the port per call, so a change applies to the next request rather than the next launch.
- **Allowlist enforcement in two layers.** `antik_domain::scope::Pattern` compiles an entry (`*` and `?` inside a segment, `**` across segments, a literal entry covers its subtree) and refuses absolute paths, `..`, backslashes and control characters. `MarkdownFiles::read_scope` walks the project, canonicalises every candidate and drops anything that does not resolve under the project root, so a symlink is an escape that opens nothing. Files are capped at 64 KiB, the block at 40k characters and 40 files, NUL bytes are skipped, and an empty allowlist sends nothing at all. The budget is sized against the context window the request asks for (`num_ctx` 16384): at about four characters per token the prompt is roughly 10k tokens, leaving room for the system prompt and the reply. Ask for a window larger than the model was loaded with and Ollama truncates the prompt rather than refusing it, so the two numbers are one policy, not two.
- **Untrusted output.** Model text is normalised to one line per field (`one_line` strips fences, quotes and control characters) and capped (question 400 characters, hint 300, each advice point 600, four points). It is display copy only: a gate question, a hint, or critique paragraphs. It is never parsed as a command, never stored as a gate outcome (I1), and never allowed to pick a file.
- **Failure is fail-open, with the reason logged.** `draft_gate` builds the app's question and rubric first. A model success replaces the wording and sets `gates.drafted_by = 'model'`; a failure keeps the app's question and appends `model unavailable: …` to the gate event, so the log says what happened. Critique is refused without a prediction and writes no event at all: advice is not a mutation.
- **Local models are slow, so the clock is split.** 180 s for a completion, 5 s for health (`antik_domain::model`), and `think: false` is sent because a 14B reasoning model would otherwise spend the whole deadline on a trace nobody reads. One retry on 429/5xx, none on a refused connection.
- **Secrets.** None exist in this build: transport is loopback HTTP with no TLS stack compiled in, and the keyring is not wired. When the API lane arrives, its key belongs in the OS keyring and nowhere else.
- **Prompt injection is assumed.** File contents are wrapped in delimiters and the system prompt is fixed; even so, the only effect of a successful injection is misleading advice or a bad question — never a state change. That is the security boundary, and it is why write access is off in code rather than a setting.
- **The opener plugin is scoped, not removed.** `open_project_folder` is the only thing it may do: the command canonicalises the stored project root, refuses anything outside it, and `opener:default` is replaced with `opener:allow-open-path` whose `allow` list is the projects root only. `withGlobalTauri` is set to `false` and a real CSP is set (see §12).

---

## 9. Errors and observability

### 9.1 Error taxonomy

```rust
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("illegal transition {from} -> {to}")]
    IllegalTransition { from: Phase, to: Phase },
    #[error("a prediction is required before approval")]
    PredictionRequired,
    #[error("gate {0} is unresolved")]
    GateUnresolved(GateId),
    #[error("handoff is locked until recall is stored")]
    HandoffLocked,
    #[error("document is invalid: {0}")]
    InvalidDocument(String),
    // …
}
```

- `DomainError`: what the user did wrong. Exhaustive, no IO.
- `StoreError`: SQL, migration, corruption. Wraps `rusqlite::Error`, carries context.
- `FileError`: confinement violations, parse failures, IO. Always carries the relative path.
- `AppError`: the union services return, mapped once to `AppErrorDto`.

`anyhow` is not used outside the binary's startup path. Errors that a user can act on must be typed.

### 9.2 Tracing

- `tracing_subscriber` with `EnvFilter` (`RUST_LOG` overrides), JSON file layer via `tracing-appender` writing to `logs/antik-YYYY-MM-DD.jsonl`, daily rotation, 14 days retained.
- One span per command: `command = "answer_gate"`, `task_id`, `gate_id`, `duration_ms`, `outcome`. Spans for jobs and model calls nest under it.
- Redaction rule: never log note text, answers, handoff lines, prompts or file contents at `info` or above. Log lengths and ids. This is enforced by review, and by keeping those strings out of span fields.

---

## 10. Testing and CI

### 10.1 Layers

| Layer | What | Tooling |
| --- | --- | --- |
| Domain | transition tables, gate rubric, spacing, recall matching, event derivation | `#[test]`, `proptest`, `insta` |
| Application | use cases against in-memory fakes for every port | fakes in `antik-app::testing` |
| Infra: SQLite | migrations, append-only triggers, query plans, concurrent writes, interrupted-job recovery | temp file DB, `EXPLAIN QUERY PLAN` assertions |
| Infra: files | frontmatter round-trip, atomic write, hash import, path escapes, CRLF/BOM, 64 KiB cap | `tempfile`, `cap-std` |
| Infra: runner | exit codes, timeouts, output caps, cancellation, allowlist refusal | real `/bin/sh -c` via fixture programs |
| Infra: model | prompt bounds, JSON recovery, health parsing, model-drafted vs app-drafted gates | fake `Model` in `tests/m5_model.rs`, one `#[ignore]` test against live Ollama |
| Contract | DTO snapshots + generated bindings compile | `insta`, `tsc --noEmit` |
| Performance | `bootstrap`/`review` at 10k and 100k events | `criterion`, soft budget in CI |

### 10.2 CI gates

`cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo nextest run` · `cargo deny check` (licenses, advisories, duplicates) · workspace build on Linux, macOS and Windows (compile only) · `specta` export then `git diff --exit-code src/bindings.ts` · frontend `tsc` + `vite build` · `cargo audit` weekly.

No `#[allow]` without a comment stating why. No `unwrap`/`expect` outside tests, startup and invariant-proving constructors.

---

## 11. Delivery plan

Ordered so that each milestone leaves the app usable and the riskiest assumptions are tested early.

| Milestone | Ships | Exit criteria |
| --- | --- | --- |
| **M0 — Foundation** (2–3 days) | workspace, errors, tracing, SQLite + migrations, capabilities/CSP hardening, `bootstrap` over an empty DB, specta bindings, remove `greet` | app builds on all three platforms; bindings generated; a smoke test boots an empty store |
| **M1 — The no-AI core** | projects, tasks, phases, prediction, notes, attempts, brief, handoff, session/recall, event log, markdown projection, import lane for brief/notes/decisions/questions | the Phase 1 flow from `idea.md` runs end-to-end against real files; I2, I4, I5 hold under tests |
| **M2 — Review** | metrics from events, export, backups | six figures match hand-computed fixtures; 90-day review under 15 ms at 100k events |
| **M3 — Gates and concepts** | gate drafting (local fallback), deterministic rubric scoring, overrides, concept map, spaced reviews, help levels | I1 and I6 hold; a gate answer cannot be forged through any command; property tests over transitions |
| **M4 — Sensors** (built) | check runner, drill runner, check/drill authoring, job events, cancellation | exit status is the only result; timeouts kill the process group; a cancelled run writes nothing |
| **M5 — Model** (built, local lane) | Ollama adapter, read-scope context builder, model-drafted gate questions, live critique, health, fail-open | allowlist escape tests fail closed; model off still runs gates; a dead model still drafts one |

M4 specifics as built: one `ProcessRunner` in `antik-infra::runner` spawns an argv array (never a shell) from the domain allowlist, in the project folder, in its own process group, with a two-minute deadline and a 64 KiB tail per stream. `App::start_checks` reserves the run row before any process starts; `execute_checks` writes results only if every check produced an exit status, so a timeout, a spawn failure or a cancel leaves every check untouched. `JobManager` allows two concurrent processes and holds one cancel token per run; completion is announced on `antik://job-finished` and pollable through `get_run`.

M5 specifics as built: one `Ollama` adapter in `antik-infra::model` sends a non-streaming `/api/chat` request with `format: json` and `think: false` to a loopback endpoint. Gate drafting keeps the app's rubric and only accepts the model's wording; `drafted_by` records which one phrased the question. The read scope is a pure matcher in `antik-domain` plus a canonicalising walk in `antik-infra`, and the only project contents a prompt ever carries. Health reads `/api/tags` and says whether the configured model is actually pulled. Tests use a fake `Model`; one `#[ignore]` test calls a live Ollama.

Explicitly deferred: the hosted API lane and its keyring, delegate lane, git checkpoints, sync, multi-machine, file watching, two-way task documents.

---

## 12. Frontend changes this implies

1. Delete the mock seed from production code; keep `seedState()` behind a `--demo`/`ANTIK_DEMO=1` flag as a fixture generator for tests and screenshots.
2. Replace `src/store.ts` with `src/api.ts`: typed `invoke` wrapper, generated types, a `listen` subscription for job events, and error-code → copy mapping.
3. Keep the current screens; they already render every field the contract returns. Check authoring shipped with M4 (the verify screen adds and removes checks and shows the command each one runs). Still open: **surfacing import conflicts** (`conflict` → reload prompt).
4. Model screen (M5, done): Settings carries the loopback endpoint, the model name and a **Test connection** button that renders health as "reachable / unreachable" with the reason; the plan screen's critique is a button that shows the live advice or the failure, never fixture prose; a gate phrased by the model is marked as such next to the question.
5. Self-hosted type (done). Geist and Geist Mono ship in the bundle as latin-subset `woff2` files, loaded by `@font-face` in `src/styles.css`; the release CSP names no font host, and `style-src 'self'` means markup carries no `style` attribute — layout tweaks are classes. `devCsp` keeps `'unsafe-inline'` because Vite injects the stylesheet through JavaScript in development.

---

## 13. Risks and open decisions

1. **Markdown ownership.** This plan makes SQLite authoritative and markdown a projection, with a four-document import lane. That is the simplest correct model, but it means task documents are read-only outside the app. If editing tasks in Obsidian matters, the import lane has to grow a strict section parser and a conflict UI. Decide before M1 ships, not after.
2. **Deterministic gate scoring quality.** Keyword rubrics are honest but shallow. The interface (`GateScorer`) allows a second scorer later, but v1 will feel mechanical on answers that are correct but phrased differently. Watch it in the first month; overrides are the escape hatch.
3. **Check authoring UX.** The runner is only useful if writing a definition of done is quick. That is a frontend milestone, not backend.
4. **Cross-platform process control.** Process-group kill is Unix-shaped; Windows needs a job object. If Windows is not a target, say so and drop the claim rather than ship a half version.
5. **`panic = "abort"` in release** is already configured. It is fine for a desktop app, but it means a panic in a job task takes the process down. Jobs must be written as total functions returning `Result`; review that in M4.
6. **Migration reversibility.** None. Backups before migration are the mitigation; a downgrade requires restoring a backup, which the UI should explain rather than attempt automatically.
