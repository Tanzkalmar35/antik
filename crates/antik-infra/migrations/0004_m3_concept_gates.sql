-- Gates can name the concept they check. A gate drafted from a task concept
-- stores that link; a task-based gate leaves it null.
ALTER TABLE gates ADD COLUMN concept_id TEXT REFERENCES concepts(id) ON DELETE SET NULL;

CREATE INDEX gates_by_concept ON gates(concept_id, status);
