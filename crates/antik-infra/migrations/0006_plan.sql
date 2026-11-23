-- The plan document. SQLite is authoritative; the task markdown file is a
-- projection of these rows. A plan is two levels deep: a section owns
-- subtasks, and definition-of-done items stand on their own.

ALTER TABLE tasks ADD COLUMN plan_updated_at INTEGER;

CREATE TABLE plan_items (
  id          TEXT PRIMARY KEY,
  task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  parent_id   TEXT REFERENCES plan_items(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL CHECK (kind IN ('section','subtask','dod')),
  title       TEXT NOT NULL,
  body        TEXT NOT NULL DEFAULT '',
  position    INTEGER NOT NULL DEFAULT 0,
  status      TEXT NOT NULL CHECK (status IN ('open','done')) DEFAULT 'open',
  source      TEXT NOT NULL CHECK (source IN ('user','model')) DEFAULT 'user',
  check_id    TEXT REFERENCES checks(id) ON DELETE SET NULL,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX plan_items_by_task ON plan_items(task_id, position, created_at);

CREATE TABLE plan_messages (
  id         TEXT PRIMARY KEY,
  task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  at         INTEGER NOT NULL,
  role       TEXT NOT NULL CHECK (role IN ('user','assistant')),
  content    TEXT NOT NULL,
  model      TEXT NOT NULL DEFAULT ''
);
CREATE INDEX plan_messages_by_task ON plan_messages(task_id, at, id);
