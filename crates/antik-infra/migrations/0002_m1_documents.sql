-- M1 documents. Forward-only; never edit an applied migration.

ALTER TABLE projects ADD COLUMN brief TEXT NOT NULL DEFAULT '';

CREATE TABLE project_notes (
  id         TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  at         INTEGER NOT NULL,
  title      TEXT NOT NULL DEFAULT '',
  text       TEXT NOT NULL
);
CREATE INDEX project_notes_by_project ON project_notes(project_id, at DESC);
