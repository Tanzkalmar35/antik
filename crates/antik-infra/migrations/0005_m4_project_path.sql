-- A project may be attached to a local repository. When it is, checks run in
-- that directory and antik's documents live in its .antik folder. The column
-- stays null for projects that live entirely in app data.
ALTER TABLE projects ADD COLUMN repo_path TEXT;
