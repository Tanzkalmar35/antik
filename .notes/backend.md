# Backend contract

The desktop client is local-first, so this service may be Tauri IPC or a small HTTP server on the same machine. The shapes below are HTTP-shaped. Every mutation returns the changed record and appends one event; the client renders state but never decides it.

## Bootstrap

| Method | Path | Does |
| --- | --- | --- |
| GET | `/bootstrap` | Everything the shell needs on launch: projects with current task and handoff state, concepts, drills, settings, current session, newest 50 events. |
| GET | `/export` | Whole store as JSON, for the export button. |

## Projects

| Method | Path | Does |
| --- | --- | --- |
| GET | `/projects` | List with current task, pending gate, handoff/recall state and last-touched time. |
| POST | `/projects` | Create a project and its first task (plan phase, unapproved). Body: `name`, `brief`. |
| GET | `/projects/:id` | Brief, facts, current task id, handoff state. |
| PATCH | `/projects/:id/brief` | Replace the brief body; records a decision event. |

## Notebook

Project notes are ungraded. They must never appear in gate scoring, gate payloads or review figures.

| Method | Path | Does |
| --- | --- | --- |
| GET | `/projects/:id/notes` | Notes, newest first. |
| POST | `/projects/:id/notes` | Add a note. Body: `title` (optional), `text`. |
| DELETE | `/projects/:id/notes/:noteId` | Remove one note; logged. |

## Tasks

| Method | Path | Does |
| --- | --- | --- |
| GET | `/tasks/:id` | Task, checks, gates, attempts, working notes, concepts, phase. |
| POST | `/projects/:id/tasks` | Plan a batch. Body: `title`, `scope`, `prediction` (may be empty). |
| POST | `/tasks/:id/prediction` | Write or replace the prediction. Critique is unavailable before this exists. |
| POST | `/tasks/:id/plan/approve` | Plan → implement. Refuses without a prediction. |
| POST | `/tasks/:id/notes` | Add a working note attached to this batch. |
| POST | `/tasks/:id/attempts` | Record an attempt. Append-only. |
| POST | `/tasks/:id/implement/close` | Implement → verify. |
| POST | `/tasks/:id/handoff` | Write the three-line handoff; closes the task and the session. |

## Checks

The server, not the client, decides a check result.

| Method | Path | Does |
| --- | --- | --- |
| GET | `/tasks/:id/checks` | Current checks with last evaluation time. |
| POST | `/tasks/:id/checks/run` | Execute the definition of done, return `runId`. Async. |
| GET | `/runs/:runId` | Run status and per-check `pass`/`fail`, written by the runner only. |

## Gates

| Method | Path | Does |
| --- | --- | --- |
| GET | `/tasks/:id/gates` | Gates for the task, newest attempt log included. |
| POST | `/tasks/:id/gates/draft` | Drafts the next question from the finished work. Prefers a due concept the task touches; falls back to the task. Stored as `due`; it cannot pass itself. |
| POST | `/gates/:id/attempts` | Score one answer. Body: `answer`. Returns `status` (`passed`/`failed`), a short note, and the appended attempt. |
| POST | `/gates/:id/hint` | Mark the hint used; it stays attached to the gate. |
| POST | `/gates/:id/override` | Body: `reason`. Sets `overridden`, never `passed`; the reason is logged. |

## Handoff and recall

The handoff must not be readable before the session's recall is stored. Enforce this on the server, not in the view.

| Method | Path | Does |
| --- | --- | --- |
| GET | `/projects/:id/handoff` | The last handoff. `409 locked` until a recall exists for the open session. |
| POST | `/projects/:id/recall` | Body: `text`. Stores the recall, starts the session, returns `matchedLines` and — only now — the handoff. |
| POST | `/sessions/current/close` | Close the session. Normally happens through the handoff. |

## Concepts

| Method | Path | Does |
| --- | --- | --- |
| GET | `/concepts?status=due\|weak\|all` | Ledger with help level, last reviewed, next due. |
| POST | `/concepts` | Body: `id`, `name`, `area`, `definition`. The key is human-authored and stable. |
| POST | `/tasks/:id/concepts` | Body: `conceptId`. Attach a concept to the batch it touches. |
| DELETE | `/tasks/:id/concepts/:conceptId` | Detach a concept; logged. |
| POST | `/concepts/:id/reviews` | Body: `answer`. Server judges pass or thin, sets the next due date, records the attempt. |
| PATCH | `/concepts/:id/help` | Set `full`, `hint` or `none`. |

## Decisions and questions

| Method | Path | Does |
| --- | --- | --- |
| GET | `/projects/:id/decisions` | Decisions with why and cost. |
| POST | `/projects/:id/decisions` | Body: `title`, `why`, `cost`, optional `taskId`. Append-only. |
| GET | `/projects/:id/questions` | Open and answered questions. |
| POST | `/projects/:id/questions` | Record an open question. |
| POST | `/questions/:id/answer` | Attach an answer; the question stays in history. |

## Review

| Method | Path | Does |
| --- | --- | --- |
| GET | `/review?days=30` | Six figures (tasks, lead time, rework, gate pass rate, bypasses, failing checks) plus task rows. Each figure carries its basis string; compute from the event log. |

## Drills

| Method | Path | Does |
| --- | --- | --- |
| GET | `/drills` | Drills grouped by skill with attempt count and state. |
| POST | `/drills/:id/run` | Run the command, record the attempt; the exit status is the result. Failures are kept. |

## Settings and model

| Method | Path | Does |
| --- | --- | --- |
| GET | `/settings` | Model choice and read allowlist. |
| PATCH | `/settings/model` | Set `local`, `api` or `none`. |
| PATCH | `/settings/model/target` | Set the local endpoint and model name. Loopback only; any other host is refused. |
| POST | `/settings/allowlist` | Add one path. |
| DELETE | `/settings/allowlist` | Remove one path; body: `path`. |
| GET | `/model/health` | Configured and reachable, without exposing credentials. |
| POST | `/model/critique` | Critiques the plan from the prediction and read scope. Returns advice only; cannot approve, score or write. |

Model requests receive allowlisted paths only. There is no write endpoint. This build speaks to a local endpoint only: `api` is stored as a choice and reports `model_unavailable`.

## Log

| Method | Path | Does |
| --- | --- | --- |
| GET | `/events?project=&kind=&before=&limit=` | Read the append-only log, newest first, filtered by project and kind. |

## Rules the backend owns

- A gate outcome is computed by the app. No model response can set `passed` or `overridden`.
- A handoff is withheld until that session's recall is stored.
- Check results are written only by the check runner, never submitted by the client.
- One event per mutation, server timestamp, append-only. Notes are the only deletable records.
- Project and batch notes are never sent to gate scoring or into the review figures.
- Read scope is enforced per request, and there is no path by which the model can write.
