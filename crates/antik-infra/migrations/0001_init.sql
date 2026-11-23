-- Schema v1. Forward-only; never edit an applied migration.
-- Enum columns are TEXT with CHECK constraints so invalid state cannot be stored.

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT INTO meta (key, value) VALUES ('revision', '0');

CREATE TABLE projects (
  id             TEXT PRIMARY KEY,
  slug           TEXT NOT NULL UNIQUE,
  name           TEXT NOT NULL,
  root_path      TEXT NOT NULL,
  brief_excerpt  TEXT NOT NULL DEFAULT '',
  current_task_id TEXT,
  created_at     INTEGER NOT NULL,
  archived_at    INTEGER,
  revision       INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE tasks (
  id             TEXT PRIMARY KEY,
  project_id     TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title          TEXT NOT NULL,
  lane           TEXT NOT NULL CHECK (lane IN ('learn','verify')),
  phase          TEXT NOT NULL CHECK (phase IN ('plan','implement','verify','reflect')),
  batch          TEXT NOT NULL DEFAULT '',
  scope          TEXT NOT NULL DEFAULT '',
  prediction     TEXT NOT NULL DEFAULT '',
  prediction_at  INTEGER,
  plan_approved_at INTEGER,
  started_at     INTEGER NOT NULL,
  finished_at    INTEGER,
  minutes        INTEGER NOT NULL DEFAULT 0,
  created_at     INTEGER NOT NULL DEFAULT 0,
  revision       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX tasks_by_project ON tasks(project_id, phase, started_at DESC);

CREATE TABLE task_notes (
  id      TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  at      INTEGER NOT NULL,
  text    TEXT NOT NULL
);
CREATE INDEX task_notes_by_task ON task_notes(task_id, at DESC);

CREATE TABLE attempts (
  id      TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  at      INTEGER NOT NULL,
  text    TEXT NOT NULL
);
CREATE INDEX attempts_by_task ON attempts(task_id, at DESC);

CREATE TABLE gates (
  id              TEXT PRIMARY KEY,
  task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  n               INTEGER NOT NULL,
  question        TEXT NOT NULL,
  hint            TEXT NOT NULL DEFAULT '',
  rubric_json     TEXT NOT NULL,
  status          TEXT NOT NULL CHECK (status IN
                    ('not_yet','due','passed','failed','overridden')),
  answer          TEXT,
  override_reason TEXT,
  hint_used       INTEGER NOT NULL DEFAULT 0,
  evaluated_at    INTEGER,
  drafted_by      TEXT NOT NULL CHECK (drafted_by IN ('app','model')),
  UNIQUE (task_id, n)
);
CREATE INDEX gates_by_task ON gates(task_id, status);

CREATE TABLE gate_attempts (
  id      TEXT PRIMARY KEY,
  gate_id TEXT NOT NULL REFERENCES gates(id) ON DELETE CASCADE,
  at      INTEGER NOT NULL,
  answer  TEXT NOT NULL,
  passed  INTEGER NOT NULL,
  note    TEXT NOT NULL
);
CREATE INDEX gate_attempts_by_gate ON gate_attempts(gate_id, at DESC);

CREATE TABLE checks (
  id            TEXT PRIMARY KEY,
  task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  label         TEXT NOT NULL,
  detail        TEXT NOT NULL DEFAULT '',
  spec_json     TEXT NOT NULL DEFAULT '{}',
  result        TEXT NOT NULL CHECK (result IN ('pass','fail','pending','done')),
  evaluated_at  INTEGER,
  last_run_id   TEXT
);
CREATE INDEX checks_by_task ON checks(task_id);

CREATE TABLE check_runs (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL,
  started_at  INTEGER NOT NULL,
  finished_at INTEGER,
  status      TEXT NOT NULL CHECK (status IN ('running','done','failed','cancelled'))
);

CREATE TABLE check_results (
  run_id      TEXT NOT NULL REFERENCES check_runs(id) ON DELETE CASCADE,
  check_id    TEXT NOT NULL,
  exit_code   INTEGER,
  result      TEXT NOT NULL CHECK (result IN ('pass','fail','pending','done')),
  duration_ms INTEGER NOT NULL,
  stdout_tail TEXT NOT NULL DEFAULT '',
  stderr_tail TEXT NOT NULL DEFAULT '',
  truncated   INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (run_id, check_id)
);

CREATE TABLE concepts (
  id               TEXT PRIMARY KEY,
  name             TEXT NOT NULL,
  area             TEXT NOT NULL,
  definition       TEXT NOT NULL DEFAULT '',
  seen             INTEGER NOT NULL DEFAULT 0,
  help             TEXT NOT NULL CHECK (help IN ('full','hint','none')) DEFAULT 'full',
  status           TEXT NOT NULL CHECK (status IN ('passed','weak','due')) DEFAULT 'due',
  last_reviewed_at INTEGER,
  due_on           INTEGER
);
CREATE INDEX concepts_due ON concepts(status, due_on);

CREATE TABLE task_concepts (
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  concept_id TEXT NOT NULL REFERENCES concepts(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, concept_id)
);

CREATE TABLE concept_reviews (
  id         TEXT PRIMARY KEY,
  concept_id TEXT NOT NULL REFERENCES concepts(id) ON DELETE CASCADE,
  at         INTEGER NOT NULL,
  answer     TEXT NOT NULL,
  verdict    TEXT NOT NULL CHECK (verdict IN ('pass','thin')),
  next_due   INTEGER NOT NULL
);

CREATE TABLE decisions (
  id         TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id    TEXT,
  at         INTEGER NOT NULL,
  title      TEXT NOT NULL,
  why        TEXT NOT NULL,
  cost       TEXT NOT NULL
);
CREATE INDEX decisions_by_project ON decisions(project_id, at DESC);

CREATE TABLE questions (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  at          INTEGER NOT NULL,
  text        TEXT NOT NULL,
  answer      TEXT,
  answered_at INTEGER
);
CREATE INDEX questions_by_project ON questions(project_id, at DESC);

CREATE TABLE handoffs (
  id         TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id    TEXT,
  at         INTEGER NOT NULL,
  next       TEXT NOT NULL,
  blocking   TEXT NOT NULL,
  thinking   TEXT NOT NULL
);
CREATE INDEX handoffs_by_project ON handoffs(project_id, at DESC);

CREATE TABLE sessions (
  id           TEXT PRIMARY KEY,
  project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  started_at   INTEGER NOT NULL,
  closed_at    INTEGER,
  recall_text  TEXT,
  matched_lines INTEGER
);
CREATE INDEX sessions_by_project ON sessions(project_id, started_at DESC);

CREATE TABLE events (
  seq            INTEGER PRIMARY KEY AUTOINCREMENT,
  id             TEXT NOT NULL UNIQUE,
  at             INTEGER NOT NULL,
  kind           TEXT NOT NULL,
  actor          TEXT NOT NULL CHECK (actor IN ('user','app','runner','model')),
  project_id     TEXT,
  task_id        TEXT,
  payload        TEXT NOT NULL DEFAULT '{}',
  summary        TEXT NOT NULL,
  schema_version INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX events_by_project ON events(project_id, seq DESC);
CREATE INDEX events_by_kind ON events(kind, seq DESC);

CREATE TRIGGER events_no_update BEFORE UPDATE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;

CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;

CREATE TABLE file_index (
  path        TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL,
  kind        TEXT NOT NULL,
  record_id   TEXT,
  content_hash TEXT NOT NULL,
  exported_at INTEGER NOT NULL
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT INTO settings (key, value) VALUES ('model', 'local'), ('write_access', '0');

CREATE TABLE allowlist (
  path     TEXT PRIMARY KEY,
  added_at INTEGER NOT NULL
);

CREATE TABLE drills (
  id          TEXT PRIMARY KEY,
  skill       TEXT NOT NULL,
  name        TEXT NOT NULL,
  command     TEXT NOT NULL,
  last_run_at INTEGER,
  attempts    INTEGER NOT NULL DEFAULT 0,
  passed      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE drill_runs (
  id          TEXT PRIMARY KEY,
  drill_id    TEXT NOT NULL REFERENCES drills(id) ON DELETE CASCADE,
  at          INTEGER NOT NULL,
  exit_code   INTEGER,
  duration_ms INTEGER NOT NULL,
  passed      INTEGER NOT NULL
);

CREATE TABLE jobs (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('checks','drill','model')),
  status       TEXT NOT NULL CHECK (status IN ('running','done','failed','cancelled')),
  started_at   INTEGER NOT NULL,
  finished_at  INTEGER,
  payload_json TEXT NOT NULL DEFAULT '{}'
);
