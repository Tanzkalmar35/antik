-- Schema v3. Indexes for the review window and its per-task aggregates.
-- Append-only event history already exists; this only makes the read model cheap.

CREATE INDEX IF NOT EXISTS events_by_at ON events(at DESC);
CREATE INDEX IF NOT EXISTS events_by_kind_at ON events(kind, at DESC);
CREATE INDEX IF NOT EXISTS events_by_task_kind ON events(task_id, kind, at);

-- Only scored gate attempts carry a `pass` payload. Keeping passed attempts in
-- their own partial index avoids parsing JSON for every attempt in a review.
CREATE INDEX IF NOT EXISTS events_gate_passed ON events(at)
  WHERE kind = 'gate_attempted' AND json_extract(payload, '$.pass') = 1;
