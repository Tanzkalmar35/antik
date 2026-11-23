use antik_app::dto::{
    Attempt, Bootstrap, CheckOutcome, CheckSummary, ConceptStatus, ConceptSummary,
    CurrentTaskSummary, DecisionSummary, DrillOutcome, DrillSummary, EventSummary, GateAttempt,
    GateDetail, GateStatus, HandoffDetail, HandoffSummary, HelpLevel, ModelChoice, PlanItem,
    PlanItemKind, PlanItemStatus, PlanMessage, ProjectDetail, ProjectNote, ProjectSummary,
    QuestionSummary, RecallOutcome, Review, ReviewBasis, ReviewGate, ReviewRow, RunDetail, Session,
    SessionView, Settings, TaskDetail, TaskNote,
};
use antik_app::error::AppError;
use antik_app::ports::{
    CheckTarget, ConceptRecord, DrillRecord, GateAnswer, GateRecord, NewAttempt, NewCheck,
    NewCheckResult, NewCheckRun, NewConcept, NewConceptReview, NewDecision, NewDrill, NewDrillRun,
    NewEvent, NewGate, NewHandoff, NewNote, NewPlanItem, NewPlanMessage, NewProject, NewQuestion,
    NewTask, NewTaskNote, Store,
};
use antik_domain::concept::ConceptVerdict;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::db::{Db, StoreError};

impl Store for Db {
    fn bootstrap(&self) -> Result<Bootstrap, AppError> {
        self.with_conn(bootstrap_from)
            .map_err(|error| AppError::Internal(format!("bootstrap: {error}")))
    }

    fn create_project(
        &self,
        project: NewProject,
        task: NewTask,
        events: &[NewEvent],
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            conn.execute(
                "INSERT INTO projects
                   (id, slug, name, root_path, brief, brief_excerpt, current_task_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &project.id,
                    &project.slug,
                    &project.name,
                    &project.slug,
                    &project.brief,
                    excerpt(&project.brief),
                    &task.id,
                    project.at
                ],
            )?;
            insert_task(conn, &task)?;
            append_events(conn, events)?;
            bump_revision(conn)?;
            project_detail(conn, &project.id)
        })
        .map_err(map_error)
    }

    fn project(&self, id: &str) -> Result<ProjectDetail, AppError> {
        self.with_conn(|conn| project_detail(conn, id))
            .map_err(map_error)
    }

    fn set_project_path(
        &self,
        project_id: &str,
        repo_path: Option<&str>,
        _at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let current: Option<Option<String>> = conn
                .query_row(
                    "SELECT repo_path FROM projects WHERE id = ?1 AND archived_at IS NULL",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()?;
            let current = match current {
                Some(value) => value,
                None => return Err(not_found("project")),
            };
            // Attaching the same directory twice is not a mutation.
            if current.as_deref() == repo_path {
                return project_detail(conn, project_id);
            }
            conn.execute(
                "UPDATE projects SET repo_path = ?2, revision = revision + 1 WHERE id = ?1",
                params![project_id, repo_path],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn set_current_task(
        &self,
        project_id: &str,
        task_id: &str,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let owner: Option<String> = conn
                .query_row(
                    "SELECT project_id FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;
            match owner.as_deref() {
                Some(owner) if owner == project_id => {}
                // A task from another project is not found here, not invalid:
                // ids must not be usable across projects.
                _ => return Err(not_found("task")),
            }
            let current: Option<Option<String>> = conn
                .query_row(
                    "SELECT current_task_id FROM projects WHERE id = ?1 AND archived_at IS NULL",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()?;
            match current {
                // Switching to the batch already current is not a mutation.
                Some(value) if value.as_deref() == Some(task_id) => {
                    return project_detail(conn, project_id);
                }
                Some(_) => {}
                None => return Err(not_found("project")),
            }
            conn.execute(
                "UPDATE projects SET current_task_id = ?2, revision = revision + 1 WHERE id = ?1",
                params![project_id, task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn update_brief(
        &self,
        project_id: &str,
        body: &str,
        expected_revision: Option<u32>,
        _at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let revision: Option<u32> = conn
                .query_row(
                    "SELECT revision FROM projects WHERE id = ?1 AND archived_at IS NULL",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()?;
            let revision = revision.ok_or_else(|| not_found("project"))?;
            if let Some(expected) = expected_revision {
                if expected != revision {
                    return Err(StoreError::Conflict(
                        "the brief changed since you loaded it".to_owned(),
                    ));
                }
            }
            conn.execute(
                "UPDATE projects
                 SET brief = ?2, brief_excerpt = ?3, revision = revision + 1
                 WHERE id = ?1",
                params![project_id, body, excerpt(body)],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn update_note(
        &self,
        project_id: &str,
        note_id: &str,
        title: &str,
        text: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let changed = conn.execute(
                "UPDATE project_notes SET title = ?3, text = ?4, at = ?5
                 WHERE id = ?1 AND project_id = ?2",
                params![note_id, project_id, title, text, at],
            )?;
            if changed == 0 {
                return Err(not_found("note"));
            }
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn add_note(
        &self,
        project_id: &str,
        note: NewNote,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            ensure_project(conn, project_id)?;
            conn.execute(
                "INSERT INTO project_notes (id, project_id, at, title, text)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![note.id, project_id, note.at, note.title, note.text],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn delete_note(
        &self,
        project_id: &str,
        note_id: &str,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let removed = conn.execute(
                "DELETE FROM project_notes WHERE id = ?1 AND project_id = ?2",
                params![note_id, project_id],
            )?;
            if removed == 0 {
                return Err(not_found("note"));
            }
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn create_task(&self, task: NewTask, event: &NewEvent) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_project(conn, &task.project_id)?;
            insert_task(conn, &task)?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task.id)
        })
        .map_err(map_error)
    }

    fn delete_task(&self, task_id: &str, event: &NewEvent) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let project_id: Option<String> = conn
                .query_row(
                    "SELECT project_id FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;
            let project_id = project_id.ok_or_else(|| not_found("task"))?;
            // Run history is operational data, not the append-only log, so it
            // goes with the task. Child rows (notes, attempts, gates, checks,
            // task_concepts) cascade from the task itself.
            conn.execute(
                "DELETE FROM check_results
                 WHERE run_id IN (SELECT id FROM check_runs WHERE task_id = ?1)",
                params![task_id],
            )?;
            conn.execute(
                "DELETE FROM check_runs WHERE task_id = ?1",
                params![task_id],
            )?;
            conn.execute("DELETE FROM tasks WHERE id = ?1", params![task_id])?;
            // The current pointer cannot outlive the task it names: fall back
            // to the most recently started remaining task, or to no task.
            let next: Option<String> = conn
                .query_row(
                    "SELECT id FROM tasks WHERE project_id = ?1
                     ORDER BY started_at DESC, id DESC LIMIT 1",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()?;
            conn.execute(
                "UPDATE projects SET current_task_id = ?2, revision = revision + 1
                 WHERE id = ?1 AND current_task_id = ?3",
                params![project_id, next, task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, &project_id)
        })
        .map_err(map_error)
    }

    fn task(&self, id: &str) -> Result<TaskDetail, AppError> {
        self.with_conn(|conn| task_detail(conn, id))
            .map_err(map_error)
    }

    fn write_prediction(
        &self,
        task_id: &str,
        text: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let phase: Option<String> = conn
                .query_row(
                    "SELECT phase FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;
            match phase.as_deref() {
                None => Err(not_found("task")),
                Some("plan") => {
                    conn.execute(
                        "UPDATE tasks
                         SET prediction = ?2, prediction_at = ?3, revision = revision + 1
                         WHERE id = ?1",
                        params![task_id, text, at],
                    )?;
                    append_event(conn, event)?;
                    bump_revision(conn)?;
                    task_detail(conn, task_id)
                }
                Some(_) => Err(StoreError::Invalid(
                    "a prediction can only be written during the plan phase".to_owned(),
                )),
            }
        })
        .map_err(map_error)
    }

    fn approve_plan(
        &self,
        task_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT phase, prediction FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (phase, prediction) = row.ok_or_else(|| not_found("task"))?;
            match phase.as_str() {
                "plan" => {
                    if prediction.trim().is_empty() {
                        return Err(StoreError::Invalid(
                            "write a prediction before approving the plan".to_owned(),
                        ));
                    }
                    let dod: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM plan_items WHERE task_id = ?1 AND kind = 'dod'",
                        params![task_id],
                        |row| row.get(0),
                    )?;
                    if dod == 0 {
                        return Err(StoreError::Invalid(
                            "add at least one definition of done item before approving the plan"
                                .to_owned(),
                        ));
                    }
                    conn.execute(
                        "UPDATE tasks
                         SET phase = 'implement', plan_approved_at = ?2, revision = revision + 1
                         WHERE id = ?1 AND phase = 'plan'",
                        params![task_id, at],
                    )?;
                    append_event(conn, event)?;
                    bump_revision(conn)?;
                    task_detail(conn, task_id)
                }
                // A double click is harmless.
                _ => task_detail(conn, task_id),
            }
        })
        .map_err(map_error)
    }

    fn add_task_note(
        &self,
        task_id: &str,
        note: NewTaskNote,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_task(conn, task_id)?;
            conn.execute(
                "INSERT INTO task_notes (id, task_id, at, text) VALUES (?1, ?2, ?3, ?4)",
                params![note.id, task_id, note.at, note.text],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn add_plan_items(
        &self,
        task_id: &str,
        items: Vec<NewPlanItem>,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_task(conn, task_id)?;
            if items.is_empty() {
                return Err(StoreError::Invalid(
                    "a plan change needs at least one item".to_owned(),
                ));
            }
            for item in &items {
                match item.kind {
                    PlanItemKind::Subtask => {
                        let parent_id = item.parent_id.as_deref().ok_or_else(|| {
                            StoreError::Invalid("a subtask needs a section".to_owned())
                        })?;
                        let parent: Option<(String, String)> = conn
                            .query_row(
                                "SELECT task_id, kind FROM plan_items WHERE id = ?1",
                                params![parent_id],
                                |row| Ok((row.get(0)?, row.get(1)?)),
                            )
                            .optional()?;
                        match parent {
                            Some((owner, kind)) if owner == task_id && kind == "section" => {}
                            _ => {
                                return Err(StoreError::Invalid(
                                    "a subtask must belong to a section on the same task"
                                        .to_owned(),
                                ));
                            }
                        }
                    }
                    PlanItemKind::Section | PlanItemKind::Dod => {
                        if item.parent_id.is_some() {
                            return Err(StoreError::Invalid(
                                "a section and a definition-of-done item live at the top level"
                                    .to_owned(),
                            ));
                        }
                    }
                }
                if let Some(check_id) = item.check_id.as_deref() {
                    if item.kind != PlanItemKind::Dod {
                        return Err(StoreError::Invalid(
                            "only a definition-of-done item can link a check".to_owned(),
                        ));
                    }
                    let owner: Option<String> = conn
                        .query_row(
                            "SELECT task_id FROM checks WHERE id = ?1",
                            params![check_id],
                            |row| row.get(0),
                        )
                        .optional()?;
                    if owner.as_deref() != Some(task_id) {
                        return Err(StoreError::Invalid(
                            "that check belongs to another task".to_owned(),
                        ));
                    }
                }
                conn.execute(
                    "INSERT INTO plan_items
                       (id, task_id, parent_id, kind, title, body, position, status,
                        source, check_id, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                    params![
                        &item.id,
                        task_id,
                        &item.parent_id,
                        item.kind.as_str(),
                        &item.title,
                        &item.body,
                        item.position,
                        item.status.as_str(),
                        item.source.as_str(),
                        &item.check_id,
                        item.at
                    ],
                )?;
            }
            conn.execute(
                "UPDATE tasks SET plan_updated_at = ?2, revision = revision + 1 WHERE id = ?1",
                params![task_id, at],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn plan_item_task(&self, item_id: &str) -> Result<String, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT task_id FROM plan_items WHERE id = ?1",
                params![item_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| not_found("plan item"))
        })
        .map_err(map_error)
    }

    fn update_plan_item(
        &self,
        item_id: &str,
        title: &str,
        body: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let task_id: Option<String> = conn
                .query_row(
                    "SELECT task_id FROM plan_items WHERE id = ?1",
                    params![item_id],
                    |row| row.get(0),
                )
                .optional()?;
            let task_id = task_id.ok_or_else(|| not_found("plan item"))?;
            conn.execute(
                "UPDATE plan_items SET title = ?2, body = ?3, updated_at = ?4 WHERE id = ?1",
                params![item_id, title, body, at],
            )?;
            conn.execute(
                "UPDATE tasks SET plan_updated_at = ?2, revision = revision + 1 WHERE id = ?1",
                params![&task_id, at],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn set_plan_item_status(
        &self,
        item_id: &str,
        done: bool,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT task_id, kind FROM plan_items WHERE id = ?1",
                    params![item_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (task_id, kind) = row.ok_or_else(|| not_found("plan item"))?;
            if kind == "section" {
                return Err(StoreError::Invalid(
                    "a section is a heading; complete its subtasks instead".to_owned(),
                ));
            }
            let status = if done {
                PlanItemStatus::Done
            } else {
                PlanItemStatus::Open
            };
            conn.execute(
                "UPDATE plan_items SET status = ?2, updated_at = ?3 WHERE id = ?1",
                params![item_id, status.as_str(), at],
            )?;
            conn.execute(
                "UPDATE tasks SET plan_updated_at = ?2, revision = revision + 1 WHERE id = ?1",
                params![&task_id, at],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn move_plan_item(
        &self,
        item_id: &str,
        direction: i32,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, Option<String>, String)> = conn
                .query_row(
                    "SELECT task_id, parent_id, kind FROM plan_items WHERE id = ?1",
                    params![item_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let (task_id, parent_id, kind) = row.ok_or_else(|| not_found("plan item"))?;
            if direction == 0 {
                return task_detail(conn, &task_id);
            }
            let mut siblings: Vec<String> = {
                let mut statement = conn.prepare_cached(
                    "SELECT id FROM plan_items
                     WHERE task_id = ?1 AND parent_id IS ?2 AND kind = ?3
                     ORDER BY position, created_at, id",
                )?;
                let rows =
                    statement.query_map(params![&task_id, &parent_id, &kind], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            let Some(index) = siblings.iter().position(|id| id == item_id) else {
                return Err(not_found("plan item"));
            };
            let target = index as i32 + direction;
            if target < 0 || target as usize >= siblings.len() {
                return task_detail(conn, &task_id);
            }
            siblings.swap(index, target as usize);
            for (position, id) in siblings.iter().enumerate() {
                conn.execute(
                    "UPDATE plan_items SET position = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, position as i32 + 1, at],
                )?;
            }
            conn.execute(
                "UPDATE tasks SET plan_updated_at = ?2, revision = revision + 1 WHERE id = ?1",
                params![&task_id, at],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn delete_plan_item(
        &self,
        item_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let task_id: Option<String> = conn
                .query_row(
                    "SELECT task_id FROM plan_items WHERE id = ?1",
                    params![item_id],
                    |row| row.get(0),
                )
                .optional()?;
            let task_id = task_id.ok_or_else(|| not_found("plan item"))?;
            conn.execute("DELETE FROM plan_items WHERE id = ?1", params![item_id])?;
            conn.execute(
                "UPDATE tasks SET plan_updated_at = ?2, revision = revision + 1 WHERE id = ?1",
                params![&task_id, at],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn add_plan_message(
        &self,
        task_id: &str,
        message: NewPlanMessage,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_task(conn, task_id)?;
            conn.execute(
                "INSERT INTO plan_messages (id, task_id, at, role, content, model)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &message.id,
                    task_id,
                    message.at,
                    message.role.as_str(),
                    &message.content,
                    &message.model
                ],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn record_attempt(
        &self,
        task_id: &str,
        attempt: NewAttempt,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_task(conn, task_id)?;
            conn.execute(
                "INSERT INTO attempts (id, task_id, at, text) VALUES (?1, ?2, ?3, ?4)",
                params![attempt.id, task_id, attempt.at, attempt.text],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn close_implement(
        &self,
        task_id: &str,
        _at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let phase: Option<String> = conn
                .query_row(
                    "SELECT phase FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;
            match phase.as_deref() {
                None => Err(not_found("task")),
                Some("implement") => {
                    conn.execute(
                        "UPDATE tasks
                         SET phase = 'verify', revision = revision + 1
                         WHERE id = ?1 AND phase = 'implement'",
                        params![task_id],
                    )?;
                    append_event(conn, event)?;
                    bump_revision(conn)?;
                    task_detail(conn, task_id)
                }
                // Idempotent: a repeated close returns the current task.
                Some(_) => task_detail(conn, task_id),
            }
        })
        .map_err(map_error)
    }

    fn write_handoff(
        &self,
        task_id: &str,
        handoff: NewHandoff,
        events: &[NewEvent],
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, String, i64)> = conn
                .query_row(
                    "SELECT phase, project_id, started_at FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let (phase, project_id, started_at) = row.ok_or_else(|| not_found("task"))?;
            if phase != "verify" {
                return Err(StoreError::Invalid(
                    "the handoff is written when the task is in verify".to_owned(),
                ));
            }
            let unresolved: i64 = conn.query_row(
                "SELECT COUNT(*) FROM gates
                 WHERE task_id = ?1 AND status NOT IN ('passed', 'overridden')",
                params![task_id],
                |row| row.get(0),
            )?;
            if unresolved > 0 {
                return Err(StoreError::Invalid(
                    "resolve every gate before writing the handoff".to_owned(),
                ));
            }
            conn.execute(
                "INSERT INTO handoffs (id, project_id, task_id, at, next, blocking, thinking)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    handoff.id,
                    project_id,
                    task_id,
                    handoff.at,
                    handoff.next_line,
                    handoff.blocking,
                    handoff.thinking
                ],
            )?;
            let minutes = ((handoff.at - started_at) / 60_000).max(0);
            conn.execute(
                "UPDATE tasks
                 SET phase = 'reflect', finished_at = ?2, minutes = ?3, revision = revision + 1
                 WHERE id = ?1",
                params![task_id, handoff.at, minutes],
            )?;
            conn.execute(
                "UPDATE sessions SET closed_at = ?2
                 WHERE project_id = ?1 AND closed_at IS NULL",
                params![project_id, handoff.at],
            )?;
            append_events(conn, events)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn latest_handoff(&self, project_id: &str) -> Result<Option<HandoffDetail>, AppError> {
        self.with_conn(|conn| latest_handoff(conn, project_id))
            .map_err(map_error)
    }

    fn start_session(
        &self,
        project_id: &str,
        session_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<SessionView, AppError> {
        self.with_write(|conn| {
            ensure_project(conn, project_id)?;
            let open: Option<(String, i64, Option<String>)> = conn
                .query_row(
                    "SELECT id, started_at, recall_text FROM sessions
                     WHERE project_id = ?1 AND closed_at IS NULL
                     ORDER BY started_at DESC LIMIT 1",
                    params![project_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((id, started_at, recall)) = open {
                return Ok(SessionView {
                    id,
                    project_id: project_id.to_owned(),
                    started_at: iso(started_at),
                    has_recall: recall.is_some(),
                });
            }
            conn.execute(
                "INSERT INTO sessions (id, project_id, started_at) VALUES (?1, ?2, ?3)",
                params![session_id, project_id, at],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            Ok(SessionView {
                id: session_id.to_owned(),
                project_id: project_id.to_owned(),
                started_at: iso(at),
                has_recall: false,
            })
        })
        .map_err(map_error)
    }

    fn submit_recall(
        &self,
        project_id: &str,
        _session_id: &str,
        text: &str,
        matched_lines: u32,
        _at: i64,
        event: &NewEvent,
    ) -> Result<RecallOutcome, AppError> {
        self.with_write(|conn| {
            let session: Option<String> = conn
                .query_row(
                    "SELECT id FROM sessions
                     WHERE project_id = ?1 AND closed_at IS NULL
                     ORDER BY started_at DESC LIMIT 1",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()?;
            let session = session.ok_or_else(|| {
                StoreError::Locked("start a session before storing a recall".to_owned())
            })?;
            conn.execute(
                "UPDATE sessions SET recall_text = ?2, matched_lines = ?3 WHERE id = ?1",
                params![session, text, matched_lines],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            Ok(RecallOutcome {
                matched_lines,
                handoff: latest_handoff(conn, project_id)?,
            })
        })
        .map_err(map_error)
    }

    fn handoff_for(&self, project_id: &str) -> Result<HandoffDetail, AppError> {
        self.with_conn(|conn| {
            let handoff = latest_handoff(conn, project_id)?.ok_or_else(|| not_found("handoff"))?;
            let open: Option<Option<String>> = conn
                .query_row(
                    "SELECT recall_text FROM sessions
                     WHERE project_id = ?1 AND closed_at IS NULL
                     ORDER BY started_at DESC LIMIT 1",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()?;
            match open {
                Some(Some(_)) => Ok(handoff),
                _ => Err(StoreError::Locked(
                    "store the recall before reading the handoff".to_owned(),
                )),
            }
        })
        .map_err(map_error)
    }

    fn close_session(&self, project_id: &str, at: i64, event: &NewEvent) -> Result<(), AppError> {
        self.with_write(|conn| {
            let closed = conn.execute(
                "UPDATE sessions SET closed_at = ?2
                 WHERE project_id = ?1 AND closed_at IS NULL",
                params![project_id, at],
            )?;
            if closed > 0 {
                append_event(conn, event)?;
                bump_revision(conn)?;
            }
            Ok(())
        })
        .map_err(map_error)
    }

    // ---- Decisions and questions -----------------------------------------

    fn add_decision(
        &self,
        project_id: &str,
        decision: NewDecision,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            ensure_project(conn, project_id)?;
            if let Some(task_id) = &decision.task_id {
                let exists: Option<i64> = conn
                    .query_row(
                        "SELECT 1 FROM tasks WHERE id = ?1 AND project_id = ?2",
                        params![task_id, project_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if exists.is_none() {
                    return Err(not_found("task"));
                }
            }
            conn.execute(
                "INSERT INTO decisions (id, project_id, task_id, at, title, why, cost)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    decision.id,
                    project_id,
                    decision.task_id,
                    decision.at,
                    decision.title,
                    decision.why,
                    decision.cost
                ],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn add_question(
        &self,
        project_id: &str,
        question: NewQuestion,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            ensure_project(conn, project_id)?;
            conn.execute(
                "INSERT INTO questions (id, project_id, at, text)
                 VALUES (?1, ?2, ?3, ?4)",
                params![question.id, project_id, question.at, question.text],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, project_id)
        })
        .map_err(map_error)
    }

    fn answer_question(
        &self,
        question_id: &str,
        answer: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError> {
        self.with_write(|conn| {
            let project_id: Option<String> = conn
                .query_row(
                    "SELECT project_id FROM questions WHERE id = ?1",
                    params![question_id],
                    |row| row.get(0),
                )
                .optional()?;
            let project_id = project_id.ok_or_else(|| not_found("question"))?;
            let changed = conn.execute(
                "UPDATE questions SET answer = ?2, answered_at = ?3
                 WHERE id = ?1 AND answered_at IS NULL",
                params![question_id, answer, at],
            )?;
            if changed == 0 {
                return Err(StoreError::Conflict(
                    "this question is already answered".to_owned(),
                ));
            }
            append_event(conn, event)?;
            bump_revision(conn)?;
            project_detail(conn, &project_id)
        })
        .map_err(map_error)
    }

    fn question_project(&self, question_id: &str) -> Result<String, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT project_id FROM questions WHERE id = ?1",
                params![question_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| not_found("question"))
        })
        .map_err(map_error)
    }

    // ---- Sensors ---------------------------------------------------------

    fn check_targets(&self, task_id: &str) -> Result<Vec<CheckTarget>, AppError> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare_cached(
                "SELECT id, label, spec_json FROM checks WHERE task_id = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map(params![task_id], |row| {
                Ok(CheckTarget {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    spec_json: row.get(2)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(StoreError::from)
        })
        .map_err(map_error)
    }
    fn add_check(
        &self,
        task_id: &str,
        check: NewCheck,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_task(conn, task_id)?;
            conn.execute(
                "INSERT INTO checks (id, task_id, label, detail, spec_json, result)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
                params![
                    check.id,
                    task_id,
                    check.label,
                    check.detail,
                    check.spec_json
                ],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn check_task(&self, check_id: &str) -> Result<String, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT task_id FROM checks WHERE id = ?1",
                params![check_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| not_found("check"))
        })
        .map_err(map_error)
    }

    fn remove_check(&self, check_id: &str, event: &NewEvent) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let task_id: Option<String> = conn
                .query_row(
                    "SELECT task_id FROM checks WHERE id = ?1",
                    params![check_id],
                    |row| row.get(0),
                )
                .optional()?;
            let task_id = task_id.ok_or_else(|| not_found("check"))?;
            conn.execute("DELETE FROM checks WHERE id = ?1", params![check_id])?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn begin_check_run(&self, run: NewCheckRun, event: &NewEvent) -> Result<(), AppError> {
        self.with_write(|conn| {
            conn.execute(
                "INSERT INTO check_runs (id, task_id, started_at, status)
                 VALUES (?1, ?2, ?3, 'running')",
                params![&run.id, &run.task_id, run.at],
            )?;
            conn.execute(
                "INSERT INTO jobs (id, kind, status, started_at, payload_json)
                 VALUES (?1, 'checks', 'running', ?2, ?3)",
                params![
                    &run.job_id,
                    run.at,
                    json!({ "runId": run.id, "taskId": run.task_id }).to_string()
                ],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            Ok(())
        })
        .map_err(map_error)
    }

    fn finish_check_run(
        &self,
        run_id: &str,
        results: &[NewCheckResult],
        at: i64,
        event: &NewEvent,
    ) -> Result<RunDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT task_id, status FROM check_runs WHERE id = ?1",
                    params![run_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (task_id, status) = row.ok_or_else(|| not_found("run"))?;
            if status != "running" {
                return Err(StoreError::Conflict(
                    "this run has already finished".to_owned(),
                ));
            }
            conn.execute(
                "UPDATE check_runs SET status = 'done', finished_at = ?2 WHERE id = ?1",
                params![run_id, at],
            )?;
            for result in results {
                conn.execute(
                    "INSERT INTO check_results
                       (run_id, check_id, exit_code, result, duration_ms,
                        stdout_tail, stderr_tail, truncated)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        run_id,
                        result.check_id,
                        result.exit_code,
                        result.result.as_str(),
                        result.duration_ms,
                        result.stdout_tail,
                        result.stderr_tail,
                        i64::from(result.truncated)
                    ],
                )?;
                conn.execute(
                    "UPDATE checks SET result = ?2, evaluated_at = ?3, last_run_id = ?4
                     WHERE id = ?1 AND task_id = ?5",
                    params![result.check_id, result.result.as_str(), at, run_id, task_id],
                )?;
            }
            conn.execute(
                "UPDATE jobs SET status = 'done', finished_at = ?2
                 WHERE json_extract(payload_json, '$.runId') = ?1",
                params![run_id, at],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            run_detail(conn, run_id)
        })
        .map_err(map_error)
    }

    fn abandon_check_run(
        &self,
        run_id: &str,
        cancelled: bool,
        note: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<RunDetail, AppError> {
        let status = if cancelled { "cancelled" } else { "failed" };
        self.with_write(|conn| {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT task_id, status FROM check_runs WHERE id = ?1",
                    params![run_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (task_id, current) = row.ok_or_else(|| not_found("run"))?;
            if current != "running" {
                return Err(StoreError::Conflict(
                    "this run has already finished".to_owned(),
                ));
            }
            // No results, and no check rows touched: a cancelled run leaves the
            // batch exactly as it found it (I3).
            conn.execute(
                "UPDATE check_runs SET status = ?2, finished_at = ?3 WHERE id = ?1",
                params![run_id, status, at],
            )?;
            conn.execute(
                "UPDATE jobs SET status = ?2, finished_at = ?3,
                        payload_json = json_set(payload_json, '$.note', ?4)
                 WHERE json_extract(payload_json, '$.runId') = ?1",
                params![run_id, status, at, note],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            run_detail(conn, run_id)
        })
        .map_err(map_error)
    }

    fn check_run(&self, run_id: &str) -> Result<RunDetail, AppError> {
        self.with_conn(|conn| run_detail(conn, run_id))
            .map_err(map_error)
    }

    fn add_drill(&self, drill: NewDrill, event: &NewEvent) -> Result<DrillSummary, AppError> {
        self.with_write(|conn| {
            conn.execute(
                "INSERT INTO drills (id, skill, name, command, attempts, passed)
                 VALUES (?1, ?2, ?3, ?4, 0, 0)",
                params![drill.id, drill.skill, drill.name, drill.command],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            drill_summary(conn, &drill.id)
        })
        .map_err(map_error)
    }

    fn drill(&self, drill_id: &str) -> Result<DrillRecord, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, name, command FROM drills WHERE id = ?1",
                params![drill_id],
                |row| {
                    Ok(DrillRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        command: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| not_found("drill"))
        })
        .map_err(map_error)
    }

    fn record_drill_run(
        &self,
        drill_id: &str,
        run: NewDrillRun,
        event: &NewEvent,
    ) -> Result<DrillOutcome, AppError> {
        self.with_write(|conn| {
            conn.execute(
                "INSERT INTO drill_runs (id, drill_id, at, exit_code, duration_ms, passed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &run.id,
                    drill_id,
                    run.at,
                    run.exit_code,
                    run.duration_ms,
                    i64::from(run.passed)
                ],
            )?;
            let changed = conn.execute(
                "UPDATE drills SET attempts = attempts + 1, last_run_at = ?2, passed = ?3
                 WHERE id = ?1",
                params![drill_id, run.at, i64::from(run.passed)],
            )?;
            if changed == 0 {
                return Err(not_found("drill"));
            }
            conn.execute(
                "INSERT INTO jobs (id, kind, status, started_at, finished_at, payload_json)
                 VALUES (?1, 'drill', ?2, ?3, ?3, ?4)",
                params![
                    &run.job_id,
                    if run.passed { "done" } else { "failed" },
                    run.at,
                    json!({ "drillId": drill_id, "passed": run.passed }).to_string()
                ],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            let drill = drill_summary(conn, drill_id)?;
            Ok(DrillOutcome {
                drill,
                passed: run.passed,
                exit_code: run.exit_code,
                duration_ms: run.duration_ms,
                output: run.output,
            })
        })
        .map_err(map_error)
    }

    fn review(&self, days: u32, cutoff: i64) -> Result<Review, AppError> {
        self.with_conn(|conn| review_from(conn, days, cutoff))
            .map_err(map_error)
    }

    fn export_store(&self, at: i64) -> Result<String, AppError> {
        self.with_conn(|conn| export_from(conn, at))
            .map_err(map_error)
    }

    fn events(&self, project_id: Option<&str>, limit: u32) -> Result<Vec<EventSummary>, AppError> {
        self.with_conn(|conn| match project_id {
            Some(project_id) => {
                let mut statement = conn.prepare_cached(
                    "SELECT id, at, kind, project_id, task_id, summary
                     FROM events WHERE project_id = ?1
                     ORDER BY seq DESC LIMIT ?2",
                )?;
                let rows = statement.query_map(params![project_id, limit], event_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::from)
            }
            None => {
                let mut statement = conn.prepare_cached(
                    "SELECT id, at, kind, project_id, task_id, summary
                     FROM events ORDER BY seq DESC LIMIT ?1",
                )?;
                let rows = statement.query_map(params![limit], event_row)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(StoreError::from)
            }
        })
        .map_err(map_error)
    }

    fn gate(&self, gate_id: &str) -> Result<GateRecord, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, task_id, concept_id, n, status, rubric_json
                 FROM gates WHERE id = ?1",
                params![gate_id],
                |row| {
                    Ok(GateRecord {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        concept_id: row.get(2)?,
                        n: row.get::<_, i64>(3)?.max(0) as u32,
                        status: parse_gate_status(Some(row.get(4)?))?,
                        rubric_json: row.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| not_found("gate"))
        })
        .map_err(map_error)
    }

    fn gates_for_task(&self, task_id: &str) -> Result<Vec<GateDetail>, AppError> {
        self.with_conn(|conn| {
            ensure_task(conn, task_id)?;
            gates(conn, task_id)
        })
        .map_err(map_error)
    }

    fn draft_gate(
        &self,
        task_id: &str,
        gate: NewGate,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let phase: Option<String> = conn
                .query_row(
                    "SELECT phase FROM tasks WHERE id = ?1",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;
            match phase.as_deref() {
                None => Err(not_found("task")),
                Some("verify") => {
                    let open: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM gates
                         WHERE task_id = ?1 AND status IN ('due', 'failed')",
                        params![task_id],
                        |row| row.get(0),
                    )?;
                    if open > 0 {
                        return Err(StoreError::Invalid(
                            "resolve the open gate before drafting another".to_owned(),
                        ));
                    }
                    let n: i64 = conn.query_row(
                        "SELECT COALESCE(MAX(n), 0) + 1 FROM gates WHERE task_id = ?1",
                        params![task_id],
                        |row| row.get(0),
                    )?;
                    conn.execute(
                        "INSERT INTO gates
                           (id, task_id, concept_id, n, question, hint, rubric_json,
                            status, drafted_by)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'due', ?8)",
                        params![
                            gate.id,
                            task_id,
                            gate.concept_id,
                            n,
                            gate.question,
                            gate.hint,
                            gate.rubric_json,
                            gate.drafted_by.as_str()
                        ],
                    )?;
                    conn.execute(
                        "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                        params![task_id],
                    )?;
                    append_event(conn, event)?;
                    bump_revision(conn)?;
                    task_detail(conn, task_id)
                }
                Some(_) => Err(StoreError::Invalid(
                    "a gate is drafted when the task is in verify".to_owned(),
                )),
            }
        })
        .map_err(map_error)
    }

    fn answer_gate(
        &self,
        answer: GateAnswer<'_>,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        let GateAnswer {
            gate_id,
            attempt_id,
            answer,
            score,
            at,
            concept_review,
        } = answer;
        self.with_write(|conn| {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT task_id, status FROM gates WHERE id = ?1",
                    params![gate_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (task_id, status) = row.ok_or_else(|| not_found("gate"))?;
            if matches!(status.as_str(), "passed" | "overridden") {
                return Err(StoreError::Invalid(
                    "this gate is already resolved".to_owned(),
                ));
            }

            let next = if score.passed { "passed" } else { "failed" };
            let changed = conn.execute(
                "UPDATE gates SET status = ?2, answer = ?3, evaluated_at = ?4
                 WHERE id = ?1 AND status IN ('due', 'failed')",
                params![gate_id, next, answer, at],
            )?;
            if changed == 0 {
                return Err(StoreError::Invalid(
                    "the gate changed while the answer was scored".to_owned(),
                ));
            }
            conn.execute(
                "INSERT INTO gate_attempts (id, gate_id, at, answer, passed, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    attempt_id,
                    gate_id,
                    at,
                    answer,
                    i64::from(score.passed),
                    score.note
                ],
            )?;
            // A pass on a concept gate is also an explain-back: keep the
            // ledger and the concept's schedule in the same transaction.
            if let Some(review) = concept_review {
                conn.execute(
                    "UPDATE concepts
                     SET seen = seen + 1, status = 'passed',
                         last_reviewed_at = ?2, due_on = ?3
                     WHERE id = ?1",
                    params![review.concept_id, review.at, review.next_due.as_millis()],
                )?;
                conn.execute(
                    "INSERT INTO concept_reviews
                       (id, concept_id, at, answer, verdict, next_due)
                     VALUES (?1, ?2, ?3, ?4, 'pass', ?5)",
                    params![
                        review.id,
                        review.concept_id,
                        review.at,
                        review.answer,
                        review.next_due.as_millis()
                    ],
                )?;
            }
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn use_hint(&self, gate_id: &str, event: &NewEvent) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, String, i64)> = conn
                .query_row(
                    "SELECT task_id, status, hint_used FROM gates WHERE id = ?1",
                    params![gate_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let (task_id, status, hint_used) = row.ok_or_else(|| not_found("gate"))?;
            if matches!(status.as_str(), "passed" | "overridden") {
                return Err(StoreError::Invalid(
                    "this gate is already resolved".to_owned(),
                ));
            }
            if hint_used != 0 {
                // Idempotent: showing the hint twice is not two events.
                return task_detail(conn, &task_id);
            }
            conn.execute(
                "UPDATE gates SET hint_used = 1 WHERE id = ?1",
                params![gate_id],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn override_gate(
        &self,
        gate_id: &str,
        reason: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let row: Option<(String, String)> = conn
                .query_row(
                    "SELECT task_id, status FROM gates WHERE id = ?1",
                    params![gate_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (task_id, status) = row.ok_or_else(|| not_found("gate"))?;
            if matches!(status.as_str(), "passed" | "overridden") {
                return Err(StoreError::Invalid(
                    "this gate is already resolved".to_owned(),
                ));
            }
            let changed = conn.execute(
                "UPDATE gates
                 SET status = 'overridden', override_reason = ?2, evaluated_at = ?3
                 WHERE id = ?1 AND status IN ('due', 'failed')",
                params![gate_id, reason, at],
            )?;
            if changed == 0 {
                return Err(StoreError::Invalid(
                    "the gate changed while the override was written".to_owned(),
                ));
            }
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, &task_id)
        })
        .map_err(map_error)
    }

    fn concept(&self, concept_id: &str) -> Result<ConceptRecord, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, name, definition FROM concepts WHERE id = ?1",
                params![concept_id],
                |row| {
                    Ok(ConceptRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        definition: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| not_found("concept"))
        })
        .map_err(map_error)
    }

    fn concepts(&self, status: Option<ConceptStatus>) -> Result<Vec<ConceptSummary>, AppError> {
        self.with_conn(|conn| concept_list(conn, status))
            .map_err(map_error)
    }

    fn create_concept(
        &self,
        concept: NewConcept,
        event: &NewEvent,
    ) -> Result<ConceptSummary, AppError> {
        self.with_write(|conn| {
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM concepts WHERE id = ?1",
                    params![concept.id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_some() {
                return Err(StoreError::Invalid(
                    "a concept with that id already exists".to_owned(),
                ));
            }
            conn.execute(
                "INSERT INTO concepts (id, name, area, definition, status, help, seen)
                 VALUES (?1, ?2, ?3, ?4, 'due', 'full', 0)",
                params![concept.id, concept.name, concept.area, concept.definition],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            concept_summary(conn, &concept.id)
        })
        .map_err(map_error)
    }

    fn attach_concept(
        &self,
        task_id: &str,
        concept_id: &str,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            ensure_task(conn, task_id)?;
            let concept: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM concepts WHERE id = ?1",
                    params![concept_id],
                    |row| row.get(0),
                )
                .optional()?;
            if concept.is_none() {
                return Err(not_found("concept"));
            }
            conn.execute(
                "INSERT OR IGNORE INTO task_concepts (task_id, concept_id) VALUES (?1, ?2)",
                params![task_id, concept_id],
            )?;
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn detach_concept(
        &self,
        task_id: &str,
        concept_id: &str,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError> {
        self.with_write(|conn| {
            let removed = conn.execute(
                "DELETE FROM task_concepts WHERE task_id = ?1 AND concept_id = ?2",
                params![task_id, concept_id],
            )?;
            if removed == 0 {
                return Err(StoreError::Invalid(
                    "that concept is not attached to this task".to_owned(),
                ));
            }
            conn.execute(
                "UPDATE tasks SET revision = revision + 1 WHERE id = ?1",
                params![task_id],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            task_detail(conn, task_id)
        })
        .map_err(map_error)
    }

    fn review_concept(
        &self,
        review: NewConceptReview,
        event: &NewEvent,
    ) -> Result<ConceptSummary, AppError> {
        self.with_write(|conn| {
            let status = match review.verdict {
                ConceptVerdict::Pass => "passed",
                ConceptVerdict::Thin => "weak",
            };
            let changed = conn.execute(
                "UPDATE concepts
                 SET seen = seen + 1, status = ?2, last_reviewed_at = ?3, due_on = ?4
                 WHERE id = ?1",
                params![
                    review.concept_id,
                    status,
                    review.at,
                    review.next_due.as_millis()
                ],
            )?;
            if changed == 0 {
                return Err(not_found("concept"));
            }
            conn.execute(
                "INSERT INTO concept_reviews
                   (id, concept_id, at, answer, verdict, next_due)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    review.id,
                    review.concept_id,
                    review.at,
                    review.answer,
                    match review.verdict {
                        ConceptVerdict::Pass => "pass",
                        ConceptVerdict::Thin => "thin",
                    },
                    review.next_due.as_millis()
                ],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            concept_summary(conn, &review.concept_id)
        })
        .map_err(map_error)
    }

    fn set_concept_help(
        &self,
        concept_id: &str,
        help: HelpLevel,
        event: &NewEvent,
    ) -> Result<ConceptSummary, AppError> {
        self.with_write(|conn| {
            let changed = conn.execute(
                "UPDATE concepts SET help = ?2 WHERE id = ?1",
                params![concept_id, help.as_str()],
            )?;
            if changed == 0 {
                return Err(not_found("concept"));
            }
            append_event(conn, event)?;
            bump_revision(conn)?;
            concept_summary(conn, concept_id)
        })
        .map_err(map_error)
    }

    fn due_concept_for_task(
        &self,
        task_id: &str,
        cutoff: i64,
    ) -> Result<Option<ConceptSummary>, AppError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT c.id, c.name, c.area, c.definition, c.status, c.help,
                        c.seen, c.due_on, c.last_reviewed_at
                 FROM concepts c
                 JOIN task_concepts tc ON tc.concept_id = c.id
                 WHERE tc.task_id = ?1
                   AND (c.status IN ('due', 'weak') OR c.due_on IS NULL OR c.due_on <= ?2)
                 ORDER BY CASE c.status WHEN 'weak' THEN 0 WHEN 'due' THEN 1 ELSE 2 END,
                          COALESCE(c.due_on, 0), c.id
                 LIMIT 1",
                params![task_id, cutoff],
                concept_row,
            )
            .optional()
            .map_err(StoreError::from)
        })
        .map_err(map_error)
    }

    fn settings(&self) -> Result<Settings, AppError> {
        self.with_conn(settings).map_err(map_error)
    }

    fn set_model(&self, model: ModelChoice, event: &NewEvent) -> Result<Settings, AppError> {
        self.with_write(|conn| {
            let current = settings(conn)?;
            if current.model == model {
                // Re-saving the same choice is not a mutation.
                return Ok(current);
            }
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('model', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![model.as_str()],
            )?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            settings(conn)
        })
        .map_err(map_error)
    }

    fn set_model_endpoint(
        &self,
        base_url: &str,
        name: &str,
        event: &NewEvent,
    ) -> Result<Settings, AppError> {
        self.with_write(|conn| {
            let current = settings(conn)?;
            if current.model_url == base_url && current.model_name == name {
                return Ok(current);
            }
            put_setting(conn, "model_url", base_url)?;
            put_setting(conn, "model_name", name)?;
            append_event(conn, event)?;
            bump_revision(conn)?;
            settings(conn)
        })
        .map_err(map_error)
    }

    fn add_allowlist_path(
        &self,
        path: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<Settings, AppError> {
        self.with_write(|conn| {
            let changed = conn.execute(
                "INSERT OR IGNORE INTO allowlist (path, added_at) VALUES (?1, ?2)",
                params![path, at],
            )?;
            if changed > 0 {
                append_event(conn, event)?;
                bump_revision(conn)?;
            }
            settings(conn)
        })
        .map_err(map_error)
    }

    fn remove_allowlist_path(&self, path: &str, event: &NewEvent) -> Result<Settings, AppError> {
        self.with_write(|conn| {
            let removed = conn.execute("DELETE FROM allowlist WHERE path = ?1", params![path])?;
            if removed == 0 {
                return Err(not_found("read scope path"));
            }
            append_event(conn, event)?;
            bump_revision(conn)?;
            settings(conn)
        })
        .map_err(map_error)
    }
}

/// The command line a check will run, derived from the stored spec so the
/// screen shows what will actually execute rather than a paraphrase.
fn command_of(spec_json: &str) -> String {
    serde_json::from_str::<antik_domain::sensor::CheckSpec>(spec_json)
        .map(|spec| spec.command_line())
        .unwrap_or_default()
}

fn run_detail(conn: &Connection, run_id: &str) -> Result<RunDetail, StoreError> {
    let row: Option<(String, String, String, i64, Option<i64>)> = conn
        .query_row(
            "SELECT id, task_id, status, started_at, finished_at FROM check_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let (id, task_id, status, started_at, finished_at) = row.ok_or_else(|| not_found("run"))?;

    let mut statement = conn.prepare_cached(
        "SELECT r.check_id, COALESCE(c.label, r.check_id) AS label, r.result, r.exit_code,
                r.duration_ms, r.stdout_tail, r.stderr_tail, r.truncated
         FROM check_results r LEFT JOIN checks c ON c.id = r.check_id
         WHERE r.run_id = ?1 ORDER BY c.id, r.check_id",
    )?;
    let results = statement.query_map(params![run_id], |row| {
        Ok(CheckOutcome {
            check_id: row.get(0)?,
            label: row.get(1)?,
            result: parse(Some(row.get(2)?), "check result")?,
            exit_code: row.get::<_, Option<i64>>(3)?.map(|code| code as i32),
            duration_ms: row.get::<_, i64>(4)?.max(0) as u32,
            stdout_tail: row.get(5)?,
            stderr_tail: row.get(6)?,
            truncated: row.get::<_, i64>(7)? != 0,
        })
    })?;
    let results = results.collect::<Result<Vec<_>, _>>()?;

    Ok(RunDetail {
        id,
        task_id,
        status: status.parse().map_err(StoreError::Corrupt)?,
        started: iso(started_at),
        finished: finished_at.map(iso),
        results,
    })
}

fn drill_summary(conn: &Connection, drill_id: &str) -> Result<DrillSummary, StoreError> {
    conn.query_row(
        "SELECT id, skill, name, command, attempts, last_run_at, passed
         FROM drills WHERE id = ?1",
        params![drill_id],
        |row| {
            Ok(DrillSummary {
                id: row.get(0)?,
                skill: row.get(1)?,
                name: row.get(2)?,
                command: row.get(3)?,
                attempts: row.get::<_, i64>(4)?.max(0) as u32,
                last_run_at: row.get::<_, Option<i64>>(5)?.map(iso),
                passed: row.get::<_, i64>(6)? != 0,
            })
        },
    )
    .optional()?
    .ok_or_else(|| not_found("drill"))
}

fn review_from(conn: &Connection, days: u32, cutoff: i64) -> Result<Review, StoreError> {
    let tasks: i64 = conn.query_row(
        // Drive from tasks, not from the log: a few hundred indexed existence
        // probes beat counting distinct ids across the whole window.
        "SELECT COUNT(*) FROM tasks t
         WHERE EXISTS (
           SELECT 1 FROM events e
           WHERE e.task_id = t.id AND e.at >= ?1
             AND e.kind IN ('task_planned','handoff_written')
         )",
        params![cutoff],
        |row| row.get(0),
    )?;

    let handed_off: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events
         WHERE kind = 'handoff_written' AND at >= ?1",
        params![cutoff],
        |row| row.get(0),
    )?;

    let lead_ms: Option<f64> = conn.query_row(
        // Distinct handed-off tasks are few; resolve their first plan and last
        // handoff through the (task, kind, at) index instead of grouping 100k.
        "WITH handed AS MATERIALIZED (
           SELECT DISTINCT task_id FROM events
           WHERE kind = 'handoff_written' AND at >= ?1
         )
         SELECT AVG(
           (SELECT MAX(at) FROM events h
             WHERE h.task_id = handed.task_id AND h.kind = 'handoff_written')
           - (SELECT MIN(at) FROM events p
             WHERE p.task_id = handed.task_id AND p.kind = 'task_planned')
         ) FROM handed",
        params![cutoff],
        |row| row.get(0),
    )?;

    let rework: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events
         WHERE kind = 'attempt_recorded' AND at >= ?1",
        params![cutoff],
        |row| row.get(0),
    )?;
    let attempted: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events
         WHERE kind = 'gate_attempted' AND at >= ?1",
        params![cutoff],
        |row| row.get(0),
    )?;
    let passed: i64 = conn.query_row(
        // The hint matters: without it SQLite scans every attempt and parses
        // each payload instead of reading the partial index.
        "SELECT COUNT(*) FROM events INDEXED BY events_gate_passed
         WHERE kind = 'gate_attempted' AND at >= ?1
           AND json_extract(payload, '$.pass') = 1",
        params![cutoff],
        |row| row.get(0),
    )?;
    let bypasses: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events
         WHERE kind = 'gate_overridden' AND at >= ?1",
        params![cutoff],
        |row| row.get(0),
    )?;

    let failing_checks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM checks c
         WHERE c.result = 'fail'
           AND EXISTS (
             SELECT 1 FROM events e
             WHERE e.task_id = c.task_id AND e.at >= ?1
               AND e.kind IN ('task_planned','handoff_written')
           )",
        params![cutoff],
        |row| row.get(0),
    )?;

    let tasks = tasks.max(0) as u32;
    let handed_off = handed_off.max(0) as u32;
    let rework = rework.max(0) as u32;
    let attempted = attempted.max(0) as u32;
    let passed = passed.max(0) as u32;
    let bypasses = bypasses.max(0) as u32;
    let failing_checks = failing_checks.max(0) as u32;
    let lead_time_days = lead_ms.map(|ms| (ms / 86_400_000.0).max(0.0));
    let gate_pass_rate = (attempted > 0)
        .then(|| ((f64::from(passed) / f64::from(attempted)) * 100.0).round() as u32);

    Ok(Review {
        days,
        from: iso(cutoff),
        tasks,
        lead_time_days,
        rework,
        gate_pass_rate,
        resolved_gates: attempted,
        passed_gates: passed,
        bypasses,
        failing_checks,
        basis: ReviewBasis {
            tasks: format!("task_planned or handoff_written · {tasks} tasks in range"),
            lead_time: if handed_off == 0 {
                "no handoff in range".to_owned()
            } else {
                format!("mean planned→handoff · {handed_off} handed off in range")
            },
            rework: format!("attempt_recorded events in range · {rework}"),
            gate_pass_rate: if attempted == 0 {
                "no scored gate attempts in range".to_owned()
            } else {
                format!("{passed} passed / {attempted} scored attempts in range")
            },
            bypasses: format!("gate_overridden events in range · {bypasses}"),
            failing_checks: format!("current failing checks on tasks in range · {failing_checks}"),
        },
        rows: review_rows(conn, cutoff)?,
    })
}

fn review_rows(conn: &Connection, cutoff: i64) -> Result<Vec<ReviewRow>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT t.id, t.project_id, p.name, t.title, t.phase, t.finished_at,
                (SELECT MIN(at) FROM events
                  WHERE task_id = t.id AND kind = 'task_planned') AS planned_at,
                (SELECT MAX(at) FROM events
                  WHERE task_id = t.id AND kind = 'handoff_written') AS handoff_at,
                (SELECT COUNT(*) FROM attempts a WHERE a.task_id = t.id),
                g.n, g.status,
                (SELECT COUNT(*) FROM checks c
                  WHERE c.task_id = t.id AND c.result IN ('pass','done')),
                (SELECT COUNT(*) FROM checks c WHERE c.task_id = t.id)
         FROM tasks t
         JOIN projects p ON p.id = t.project_id
         LEFT JOIN gates g ON g.id = (
           SELECT id FROM gates WHERE task_id = t.id
           ORDER BY CASE WHEN status IN ('due','failed') THEN 0 ELSE 1 END, n
           LIMIT 1
         )
         WHERE EXISTS (
           SELECT 1 FROM events e
           WHERE e.task_id = t.id AND e.at >= ?1
             AND e.kind IN ('task_planned','handoff_written')
         )
         ORDER BY COALESCE(handoff_at, planned_at) DESC, t.id DESC",
    )?;

    let rows = statement.query_map(params![cutoff], |row| {
        let planned_at: Option<i64> = row.get(6)?;
        let handoff_at: Option<i64> = row.get(7)?;
        let lead_days = match (planned_at, handoff_at) {
            (Some(planned), Some(handoff)) => {
                Some((((handoff - planned) as f64) / 86_400_000.0).max(0.0))
            }
            _ => None,
        };
        let gate_n: Option<i64> = row.get(9)?;
        let gate_status: Option<String> = row.get(10)?;
        let gate = match (gate_n, gate_status) {
            (Some(n), Some(status)) => Some(ReviewGate {
                n: n.max(0) as u32,
                status: parse_gate_status(Some(status))?,
            }),
            _ => None,
        };

        Ok(ReviewRow {
            task_id: row.get(0)?,
            project_id: row.get(1)?,
            project: row.get(2)?,
            title: row.get(3)?,
            phase: parse(Some(row.get(4)?), "phase")?,
            lead_days,
            attempts: row.get::<_, i64>(8)?.max(0) as u32,
            gate,
            checks_passed: row.get::<_, i64>(11)?.max(0) as u32,
            checks_total: row.get::<_, i64>(12)?.max(0) as u32,
            closed: row.get::<_, Option<i64>>(5)?.map(iso),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// The database spells the first gate state `not_yet`; the wire spells it
/// `not-yet`. Convert at the storage edge so every read agrees.
fn parse_gate_status(value: Option<String>) -> Result<GateStatus, rusqlite::Error> {
    if value.as_deref() == Some("not_yet") {
        return Ok(GateStatus::NotYet);
    }
    parse(value, "gate status")
}

fn export_from(conn: &Connection, at: i64) -> Result<String, StoreError> {
    let schema_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let revision = revision(conn)?;
    let names = {
        let mut statement = conn.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut tables = serde_json::Map::new();
    for name in names {
        tables.insert(name.clone(), Value::Array(export_table(conn, &name)?));
    }
    let root = json!({
        "antik": 1,
        "schemaVersion": schema_version,
        "revision": revision,
        "exportedAt": iso(at),
        "tables": tables,
    });
    serde_json::to_string_pretty(&root)
        .map_err(|error| StoreError::Corrupt(format!("export: {error}")))
}

/// Export deliberately reads every column of every table; that is the point of
/// a whole-store snapshot. Table names come from `sqlite_master` and are quoted.
fn export_table(conn: &Connection, table: &str) -> Result<Vec<Value>, StoreError> {
    let quoted = format!("\"{}\"", table.replace('"', "\"\""));
    let mut statement = conn.prepare(&format!("SELECT * FROM {quoted} ORDER BY rowid"))?;
    let columns: Vec<String> = statement.column_names().map(str::to_owned).collect();
    let mut rows = statement.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            object.insert(column.clone(), export_value(row, index, column)?);
        }
        out.push(Value::Object(object));
    }
    Ok(out)
}

fn export_value(row: &Row<'_>, index: usize, column: &str) -> Result<Value, StoreError> {
    Ok(match row.get_ref(index)? {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(bytes) => {
            let text = String::from_utf8_lossy(bytes);
            if matches!(
                column,
                "payload" | "payload_json" | "rubric_json" | "spec_json"
            ) {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Ok(value);
                }
            }
            Value::String(text.into_owned())
        }
        ValueRef::Blob(bytes) => Value::Array(bytes.iter().map(|byte| json!(byte)).collect()),
    })
}

fn bootstrap_from(conn: &Connection) -> Result<Bootstrap, StoreError> {
    Ok(Bootstrap {
        projects: projects(conn)?,
        concepts: concepts(conn)?,
        drills: drills(conn)?,
        settings: settings(conn)?,
        session: session(conn)?,
        events: events(conn, 50)?,
        revision: revision(conn)?,
        schema_version: conn.query_row("PRAGMA user_version", [], |row| row.get(0))?,
    })
}

fn projects(conn: &Connection) -> Result<Vec<ProjectSummary>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT
            p.id AS id,
            p.name AS name,
            p.brief_excerpt AS brief,
            t.id AS task_id,
            t.title AS task_title,
            t.phase AS task_phase,
            t.lane AS task_lane,
            EXISTS(
              SELECT 1 FROM gates g
              WHERE g.task_id = p.current_task_id AND g.status IN ('due','failed')
            ) AS pending_gate,
            h.at AS handoff_at,
            COALESCE((
              SELECT s.recall_text IS NOT NULL
              FROM sessions s
              WHERE s.project_id = p.id
              ORDER BY s.started_at DESC
              LIMIT 1
            ), 0) AS recalled,
            COALESCE(
              (SELECT MAX(a.at) FROM attempts a WHERE a.task_id = p.current_task_id),
              (SELECT MAX(n.at) FROM task_notes n WHERE n.task_id = p.current_task_id),
              (SELECT MAX(h2.at) FROM handoffs h2 WHERE h2.project_id = p.id),
              p.created_at
            ) AS last_touched_at
         FROM projects p
         LEFT JOIN tasks t ON t.id = p.current_task_id
         LEFT JOIN handoffs h ON h.id = (
           SELECT id FROM handoffs WHERE project_id = p.id ORDER BY at DESC LIMIT 1
         )
         WHERE p.archived_at IS NULL
         ORDER BY p.name COLLATE NOCASE",
    )?;

    let rows = statement.query_map([], project_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn project_row(row: &Row<'_>) -> Result<ProjectSummary, rusqlite::Error> {
    let task_id: Option<String> = row.get("task_id")?;
    let current_task = match task_id {
        Some(id) => Some(CurrentTaskSummary {
            id,
            title: row.get("task_title")?,
            phase: parse(row.get::<_, Option<String>>("task_phase")?, "phase")?,
            lane: parse(row.get::<_, Option<String>>("task_lane")?, "lane")?,
            pending_gate: row.get::<_, i64>("pending_gate")? != 0,
        }),
        None => None,
    };

    Ok(ProjectSummary {
        id: row.get("id")?,
        name: row.get("name")?,
        brief: row.get("brief")?,
        current_task,
        handoff: match row.get::<_, Option<i64>>("handoff_at")? {
            Some(at) => Some(HandoffSummary {
                at: iso(at),
                recalled: row.get::<_, i64>("recalled")? != 0,
            }),
            None => None,
        },
        last_touched_at: row.get::<_, Option<i64>>("last_touched_at")?.map(iso),
    })
}

fn project_detail(conn: &Connection, id: &str) -> Result<ProjectDetail, StoreError> {
    let row: Option<(
        String,
        String,
        String,
        i64,
        Option<String>,
        u32,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT id, name, brief, created_at, current_task_id, revision, repo_path
             FROM projects WHERE id = ?1 AND archived_at IS NULL",
            params![id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()?;
    let (id, name, brief, created_at, current_task_id, revision, repo_path) =
        row.ok_or_else(|| not_found("project"))?;

    let tasks = task_ids(conn, &id)?
        .iter()
        .map(|task_id| task_detail(conn, task_id))
        .collect::<Result<Vec<_>, _>>()?;

    let notes = project_notes(conn, &id)?;
    let decisions = decisions(conn, &id)?;
    let questions = questions(conn, &id)?;
    let handoff = latest_handoff(conn, &id)?;
    let recall = open_recall(conn, &id)?;
    let handoff_locked = handoff.is_some() && recall.is_none();

    Ok(ProjectDetail {
        id,
        name,
        brief,
        repo_path,
        created: iso(created_at),
        current_task_id,
        tasks,
        notes,
        decisions,
        questions,
        handoff: if handoff_locked { None } else { handoff },
        handoff_locked,
        recall,
        revision,
    })
}

fn task_ids(conn: &Connection, project_id: &str) -> Result<Vec<String>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id FROM tasks WHERE project_id = ?1 ORDER BY started_at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![project_id], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn project_notes(conn: &Connection, project_id: &str) -> Result<Vec<ProjectNote>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, at, title, text FROM project_notes
         WHERE project_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![project_id], |row| {
        Ok(ProjectNote {
            id: row.get(0)?,
            at: iso(row.get(1)?),
            title: row.get(2)?,
            text: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn decisions(conn: &Connection, project_id: &str) -> Result<Vec<DecisionSummary>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, project_id, task_id, at, title, why, cost FROM decisions
         WHERE project_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![project_id], |row| {
        Ok(DecisionSummary {
            id: row.get(0)?,
            project_id: row.get(1)?,
            task_id: row.get(2)?,
            at: iso(row.get(3)?),
            title: row.get(4)?,
            why: row.get(5)?,
            cost: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn questions(conn: &Connection, project_id: &str) -> Result<Vec<QuestionSummary>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, at, text, answer, answered_at FROM questions
         WHERE project_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![project_id], |row| {
        Ok(QuestionSummary {
            id: row.get(0)?,
            at: iso(row.get(1)?),
            text: row.get(2)?,
            answer: row.get(3)?,
            answered_at: row.get::<_, Option<i64>>(4)?.map(iso),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

type TaskRow = (
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
    i64,
    String,
    String,
    String,
    Option<i64>,
    Option<i64>,
    u32,
);

fn task_detail(conn: &Connection, id: &str) -> Result<TaskDetail, StoreError> {
    let row: Option<TaskRow> = conn
        .query_row(
            "SELECT id, project_id, title, lane, phase, started_at, finished_at, minutes,
                    batch, scope, prediction, plan_approved_at, plan_updated_at, revision
             FROM tasks WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                ))
            },
        )
        .optional()?;
    let (
        id,
        project_id,
        title,
        lane,
        phase,
        started_at,
        finished_at,
        minutes,
        batch,
        scope,
        prediction,
        plan_approved_at,
        plan_updated_at,
        revision,
    ) = row.ok_or_else(|| not_found("task"))?;

    let plan_revised = match (plan_approved_at, plan_updated_at) {
        (Some(approved), Some(updated)) => updated > approved,
        _ => false,
    };
    let phase: antik_app::dto::Phase = phase.parse().map_err(StoreError::Corrupt)?;
    let lane: antik_app::dto::Lane = lane.parse().map_err(StoreError::Corrupt)?;
    let reflected = phase == antik_app::dto::Phase::Reflect;

    Ok(TaskDetail {
        id: id.clone(),
        project_id,
        title,
        lane,
        phase,
        started: iso(started_at),
        finished: finished_at.map(iso),
        minutes: minutes.max(0) as u32,
        batch,
        scope,
        prediction,
        plan_approved: plan_approved_at.is_some(),
        plan_revised,
        plan_items: plan_items(conn, &id)?,
        plan_messages: plan_messages(conn, &id)?,
        notes: task_notes(conn, &id)?,
        attempts: attempts(conn, &id)?,
        checks: checks(conn, &id)?,
        gates: gates(conn, &id)?,
        concept_ids: concept_ids(conn, &id)?,
        reflected,
        revision,
    })
}

fn task_notes(conn: &Connection, task_id: &str) -> Result<Vec<TaskNote>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, at, text FROM task_notes WHERE task_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        Ok(TaskNote {
            id: row.get(0)?,
            at: iso(row.get(1)?),
            text: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn plan_items(conn: &Connection, task_id: &str) -> Result<Vec<PlanItem>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, parent_id, kind, title, body, position, status, source, check_id
         FROM plan_items WHERE task_id = ?1
         ORDER BY position, created_at, id",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        Ok(PlanItem {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            kind: parse(Some(row.get(2)?), "plan item kind")?,
            title: row.get(3)?,
            body: row.get(4)?,
            position: row
                .get::<_, i64>(5)?
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            status: parse(Some(row.get(6)?), "plan item status")?,
            source: parse(Some(row.get(7)?), "plan item source")?,
            check_id: row.get(8)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn plan_messages(conn: &Connection, task_id: &str) -> Result<Vec<PlanMessage>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, at, role, content, model FROM plan_messages
         WHERE task_id = ?1 ORDER BY at, id",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        Ok(PlanMessage {
            id: row.get(0)?,
            at: iso(row.get(1)?),
            role: parse(Some(row.get(2)?), "plan message role")?,
            content: row.get(3)?,
            model: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn attempts(conn: &Connection, task_id: &str) -> Result<Vec<Attempt>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, at, text FROM attempts WHERE task_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        Ok(Attempt {
            id: row.get(0)?,
            at: iso(row.get(1)?),
            text: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn checks(conn: &Connection, task_id: &str) -> Result<Vec<CheckSummary>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, label, detail, spec_json, result, evaluated_at FROM checks
         WHERE task_id = ?1 ORDER BY id",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        let spec_json: String = row.get(3)?;
        Ok(CheckSummary {
            id: row.get(0)?,
            label: row.get(1)?,
            detail: row.get(2)?,
            command: command_of(&spec_json),
            result: parse(Some(row.get(4)?), "check result")?,
            evaluated_at: row.get::<_, Option<i64>>(5)?.map(iso),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn gates(conn: &Connection, task_id: &str) -> Result<Vec<GateDetail>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, concept_id, n, question, hint, status, hint_used, answer,
                override_reason, evaluated_at, drafted_by
         FROM gates WHERE task_id = ?1 ORDER BY n",
    )?;
    let rows = statement.query_map(params![task_id], |row| {
        let id: String = row.get(0)?;
        let attempt_log = gate_attempts(conn, &id)?;
        Ok(GateDetail {
            id,
            concept_id: row.get(1)?,
            drafted_by: parse(Some(row.get(10)?), "gate source")?,
            n: row.get::<_, i64>(2)?.max(0) as u32,
            question: row.get(3)?,
            hint: row.get(4)?,
            status: parse_gate_status(row.get(5)?)?,
            attempts: attempt_log.len() as u32,
            hint_used: row.get::<_, i64>(6)? != 0,
            answer: row.get(7)?,
            reason: row.get(8)?,
            evaluated_at: row.get::<_, Option<i64>>(9)?.map(iso),
            attempt_log,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn gate_attempts(conn: &Connection, gate_id: &str) -> Result<Vec<GateAttempt>, rusqlite::Error> {
    let mut statement = conn.prepare_cached(
        "SELECT at, answer, passed, note FROM gate_attempts
         WHERE gate_id = ?1 ORDER BY at DESC, id DESC",
    )?;
    let rows = statement.query_map(params![gate_id], |row| {
        Ok(GateAttempt {
            at: iso(row.get(0)?),
            answer: row.get(1)?,
            pass: row.get::<_, i64>(2)? != 0,
            note: row.get(3)?,
        })
    })?;
    rows.collect()
}

fn concept_ids(conn: &Connection, task_id: &str) -> Result<Vec<String>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT concept_id FROM task_concepts WHERE task_id = ?1 ORDER BY concept_id",
    )?;
    let rows = statement.query_map(params![task_id], |row| row.get(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn latest_handoff(
    conn: &Connection,
    project_id: &str,
) -> Result<Option<HandoffDetail>, StoreError> {
    let row = conn
        .query_row(
            "SELECT at, next, blocking, thinking FROM handoffs
             WHERE project_id = ?1 ORDER BY at DESC, id DESC LIMIT 1",
            params![project_id],
            |row| {
                Ok(HandoffDetail {
                    at: iso(row.get(0)?),
                    next: row.get(1)?,
                    blocking: row.get(2)?,
                    thinking: row.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

fn open_recall(conn: &Connection, project_id: &str) -> Result<Option<String>, StoreError> {
    let recall: Option<Option<String>> = conn
        .query_row(
            "SELECT recall_text FROM sessions
             WHERE project_id = ?1 AND closed_at IS NULL
             ORDER BY started_at DESC LIMIT 1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(recall.flatten())
}

fn insert_task(conn: &Connection, task: &NewTask) -> Result<(), StoreError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE project_id = ?1",
        params![task.project_id],
        |row| row.get(0),
    )?;
    let batch = format!("{} of {}", count + 1, count + 1);
    let prediction_at = if task.prediction.trim().is_empty() {
        None
    } else {
        Some(task.at)
    };
    conn.execute(
        "INSERT INTO tasks
           (id, project_id, title, lane, phase, batch, scope, prediction, prediction_at,
            started_at, created_at)
         VALUES (?1, ?2, ?3, 'learn', 'plan', ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            &task.id,
            &task.project_id,
            &task.title,
            batch,
            &task.scope,
            &task.prediction,
            prediction_at,
            task.at
        ],
    )?;
    conn.execute(
        "UPDATE projects SET current_task_id = ?1 WHERE id = ?2",
        params![task.id, task.project_id],
    )?;
    Ok(())
}

fn append_events(conn: &Connection, events: &[NewEvent]) -> Result<(), StoreError> {
    for event in events {
        append_event(conn, event)?;
    }
    Ok(())
}

fn append_event(conn: &Connection, event: &NewEvent) -> Result<(), StoreError> {
    // Task events inherit their project so the project log is complete.
    let project_id = match (&event.project_id, &event.task_id) {
        (Some(project), _) => Some(project.clone()),
        (None, Some(task)) => conn
            .query_row(
                "SELECT project_id FROM tasks WHERE id = ?1",
                params![task],
                |row| row.get(0),
            )
            .optional()?,
        (None, None) => None,
    };
    let payload = event
        .payload
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "{}".to_owned());
    conn.execute(
        "INSERT INTO events (id, at, kind, actor, project_id, task_id, payload, summary)
         VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?6, ?7)",
        params![
            &event.id,
            event.at,
            event.kind.as_str(),
            project_id.as_deref(),
            event.task_id.as_deref(),
            payload,
            &event.summary
        ],
    )?;
    Ok(())
}

fn bump_revision(conn: &Connection) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE meta SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)
         WHERE key = 'revision'",
        [],
    )?;
    Ok(())
}

fn ensure_project(conn: &Connection, id: &str) -> Result<(), StoreError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    exists.map(|_| ()).ok_or_else(|| not_found("project"))
}

fn ensure_task(conn: &Connection, id: &str) -> Result<(), StoreError> {
    let exists: Option<i64> = conn
        .query_row("SELECT 1 FROM tasks WHERE id = ?1", params![id], |row| {
            row.get(0)
        })
        .optional()?;
    exists.map(|_| ()).ok_or_else(|| not_found("task"))
}

fn event_row(row: &Row<'_>) -> Result<EventSummary, rusqlite::Error> {
    Ok(EventSummary {
        id: row.get("id")?,
        at: iso(row.get("at")?),
        kind: parse(Some(row.get("kind")?), "event kind")?,
        project_id: row.get("project_id")?,
        task_id: row.get("task_id")?,
        summary: row.get("summary")?,
    })
}

fn concepts(conn: &Connection) -> Result<Vec<ConceptSummary>, StoreError> {
    concept_list(conn, None)
}

fn concept_list(
    conn: &Connection,
    status: Option<ConceptStatus>,
) -> Result<Vec<ConceptSummary>, StoreError> {
    let filter = status.map(|status| status.as_str());
    let mut statement = conn.prepare_cached(
        "SELECT id, name, area, definition, status, help, seen, due_on, last_reviewed_at
         FROM concepts
         WHERE (?1 IS NULL OR status = ?1)
         ORDER BY name COLLATE NOCASE",
    )?;
    let rows = statement.query_map(params![filter], concept_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn concept_summary(conn: &Connection, id: &str) -> Result<ConceptSummary, StoreError> {
    conn.query_row(
        "SELECT id, name, area, definition, status, help, seen, due_on, last_reviewed_at
         FROM concepts WHERE id = ?1",
        params![id],
        concept_row,
    )
    .optional()
    .map_err(StoreError::from)?
    .ok_or_else(|| not_found("concept"))
}

fn concept_row(row: &Row<'_>) -> Result<ConceptSummary, rusqlite::Error> {
    Ok(ConceptSummary {
        id: row.get("id")?,
        name: row.get("name")?,
        area: row.get("area")?,
        definition: row.get("definition")?,
        status: parse(Some(row.get("status")?), "concept status")?,
        help: parse(Some(row.get("help")?), "help level")?,
        seen: row.get::<_, i64>("seen")?.max(0) as u32,
        due_on: row.get::<_, Option<i64>>("due_on")?.map(iso),
        last_reviewed_at: row.get::<_, Option<i64>>("last_reviewed_at")?.map(iso),
    })
}

fn drills(conn: &Connection) -> Result<Vec<DrillSummary>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, skill, name, command, attempts, last_run_at, passed
         FROM drills
         ORDER BY skill COLLATE NOCASE, name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(DrillSummary {
            id: row.get("id")?,
            skill: row.get("skill")?,
            name: row.get("name")?,
            command: row.get("command")?,
            attempts: row.get::<_, i64>("attempts")?.max(0) as u32,
            last_run_at: row.get::<_, Option<i64>>("last_run_at")?.map(iso),
            passed: row.get::<_, i64>("passed")? != 0,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn settings(conn: &Connection) -> Result<Settings, StoreError> {
    let default = Settings::default();
    let model = setting(conn, "model")?
        .map(|value| {
            value
                .parse::<antik_app::dto::ModelChoice>()
                .map_err(StoreError::Corrupt)
        })
        .transpose()?
        .unwrap_or(default.model);

    let allowlist = {
        let mut statement = conn.prepare_cached("SELECT path FROM allowlist ORDER BY path")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    Ok(Settings {
        model,
        allowlist,
        write_access: setting(conn, "write_access")?.as_deref() == Some("1"),
        model_url: setting(conn, "model_url")?.unwrap_or(default.model_url),
        model_name: setting(conn, "model_name")?.unwrap_or(default.model_name),
    })
}

fn session(conn: &Connection) -> Result<Session, StoreError> {
    let row = conn
        .query_row(
            "SELECT started_at, project_id, recall_text
             FROM sessions
             WHERE closed_at IS NULL
             ORDER BY started_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>("started_at")?,
                    row.get::<_, String>("project_id")?,
                    row.get::<_, Option<String>>("recall_text")?,
                ))
            },
        )
        .optional()?;

    Ok(match row {
        Some((started_at, project_id, recall)) => Session {
            started_at: Some(iso(started_at)),
            project_id: Some(project_id),
            has_recall: recall.is_some(),
        },
        None => Session::default(),
    })
}

fn events(conn: &Connection, limit: u32) -> Result<Vec<EventSummary>, StoreError> {
    let mut statement = conn.prepare_cached(
        "SELECT id, at, kind, project_id, task_id, summary
         FROM events
         ORDER BY seq DESC
         LIMIT ?1",
    )?;
    let rows = statement.query_map(params![limit], event_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn revision(conn: &Connection) -> Result<u32, StoreError> {
    let value = conn
        .query_row("SELECT value FROM meta WHERE key = 'revision'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    match value {
        Some(raw) => raw
            .parse::<u32>()
            .map_err(|error| StoreError::Corrupt(format!("revision: {error}"))),
        None => Ok(0),
    }
}

fn put_setting(conn: &Connection, key: &str, value: &str) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn setting(conn: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(StoreError::from)
}

fn parse<T: std::str::FromStr<Err = String>>(
    value: Option<String>,
    field: &str,
) -> Result<T, rusqlite::Error> {
    let raw = value.ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Null,
            format!("missing {field}").into(),
        )
    })?;
    raw.parse::<T>().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, error.into())
    })
}

fn map_error(error: StoreError) -> AppError {
    match error {
        StoreError::NotFound(message) => AppError::NotFound(message),
        StoreError::Invalid(message) => AppError::Invalid(message),
        StoreError::Conflict(message) => AppError::Conflict(message),
        StoreError::Locked(message) => AppError::Locked(message),
        other => AppError::Internal(other.to_string()),
    }
}

fn not_found(what: &str) -> StoreError {
    StoreError::NotFound(format!("no such {what}"))
}

fn excerpt(brief: &str) -> String {
    brief
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect()
}

fn iso(millis: i64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
        .ok()
        .and_then(|at| at.format(&Rfc3339).ok())
        .unwrap_or_else(|| millis.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use antik_app::dto::EventKind;

    fn seed(db: &Db) {
        db.with_conn(|conn| {
            conn.execute_batch(
                "INSERT INTO projects (id, slug, name, root_path, brief_excerpt, created_at)
                   VALUES ('p1', 'kestrel', 'kestrel', '/tmp/kestrel', 'A local event log.', 1000);
                 INSERT INTO tasks (id, project_id, title, lane, phase, started_at)
                   VALUES ('t1', 'p1', 'Retry policy', 'learn', 'verify', 2000);
                 UPDATE projects SET current_task_id = 't1' WHERE id = 'p1';
                 INSERT INTO gates (id, task_id, n, question, rubric_json, status, drafted_by)
                   VALUES ('g1', 't1', 3, 'Why five?', '[]', 'due', 'app');
                 INSERT INTO handoffs (id, project_id, task_id, at, next, blocking, thinking)
                   VALUES ('h1', 'p1', 't1', 3000, 'next', 'blocking', 'thinking');
                 INSERT INTO sessions (id, project_id, started_at, recall_text)
                   VALUES ('s1', 'p1', 3500, 'I was ...');",
            )
            .map_err(StoreError::from)
        })
        .unwrap();
    }

    fn seed_review(db: &Db) {
        db.with_conn(|conn| {
            conn.execute_batch(
                "INSERT INTO projects (id, slug, name, root_path, brief_excerpt, created_at)
                   VALUES ('p1', 'kestrel', 'kestrel', '/tmp/kestrel', 'A local event log.', 0);
                 INSERT INTO tasks (id, project_id, title, lane, phase, started_at)
                   VALUES ('t1', 'p1', 'Retry policy', 'learn', 'verify', 1000100),
                          ('t2', 'p1', 'Outbox schema', 'learn', 'reflect', 900000),
                          ('t3', 'p1', 'Second batch', 'learn', 'plan', 1000400);
                 UPDATE projects SET current_task_id = 't3' WHERE id = 'p1';
                 INSERT INTO gates (id, task_id, n, question, rubric_json, status, drafted_by)
                   VALUES ('g1', 't1', 1, 'Why five?', '[]', 'due', 'app'),
                          ('g2', 't2', 2, 'Why delivered_at?', '[]', 'passed', 'app'),
                          ('g3', 't3', 1, 'What is done?', '[]', 'not_yet', 'app');
                 INSERT INTO checks (id, task_id, label, result)
                   VALUES ('c1', 't1', 'unit', 'pass'),
                          ('c2', 't1', 'property', 'fail'),
                          ('c3', 't2', 'migration', 'fail');
                 INSERT INTO attempts (id, task_id, at, text)
                   VALUES ('a1', 't1', 1000250, 'first'),
                          ('a0', 't1', 900000, 'outside');
                 INSERT INTO events
                   (id, at, kind, actor, project_id, task_id, payload, summary)
                 VALUES
                   ('e1', 1000100, 'task_planned', 'user', 'p1', 't1', '{}', 'planned'),
                   ('e2', 1000300, 'handoff_written', 'user', 'p1', 't1', '{}', 'handoff'),
                   ('e3', 900000, 'task_planned', 'user', 'p1', 't2', '{}', 'planned'),
                   ('e4', 1000200, 'handoff_written', 'user', 'p1', 't2', '{}', 'handoff'),
                   ('e5', 1000400, 'task_planned', 'user', 'p1', 't3', '{}', 'planned'),
                   ('e6', 1000250, 'attempt_recorded', 'user', 'p1', 't1', '{}', 'attempt'),
                   ('e7', 900000, 'attempt_recorded', 'user', 'p1', 't1', '{}', 'outside'),
                   ('e8', 1000150, 'gate_attempted', 'user', 'p1', 't1', '{\"pass\":true}', 'passed'),
                   ('e9', 1000160, 'gate_attempted', 'user', 'p1', 't1', '{\"pass\":false}', 'failed'),
                   ('e10', 900000, 'gate_attempted', 'user', 'p1', 't1', '{\"pass\":true}', 'outside'),
                   ('e11', 1000170, 'gate_overridden', 'user', 'p1', 't1', '{\"reason\":\"time\"}', 'override');",
            )
            .map_err(StoreError::from)
        })
        .unwrap();
    }

    #[test]
    fn review_figures_match_a_hand_computed_fixture() {
        let db = Db::open_in_memory().unwrap();
        seed_review(&db);

        let review = db.review(30, 1_000_000).unwrap();
        assert_eq!(review.days, 30);
        assert_eq!(review.from, iso(1_000_000));
        assert_eq!(review.tasks, 3);
        assert_eq!(review.rework, 1);
        assert_eq!(review.resolved_gates, 2);
        assert_eq!(review.passed_gates, 1);
        assert_eq!(review.gate_pass_rate, Some(50));
        assert_eq!(review.bypasses, 1);
        assert_eq!(review.failing_checks, 2);
        assert_eq!(review.rows.len(), 3);

        let expected_lead = ((200.0 + 100_200.0) / 2.0) / 86_400_000.0;
        let lead = review.lead_time_days.expect("lead time");
        assert!((lead - expected_lead).abs() < 1e-12, "lead {lead}");

        let t1 = review
            .rows
            .iter()
            .find(|row| row.task_id == "t1")
            .expect("t1 row");
        assert_eq!(t1.gate.as_ref().unwrap().status, GateStatus::Due);
        assert_eq!(t1.attempts, 2);
        assert_eq!(t1.checks_passed, 1);
        assert_eq!(t1.checks_total, 2);
        assert!((t1.lead_days.unwrap() - 200.0 / 86_400_000.0).abs() < 1e-12);

        let t3 = review
            .rows
            .iter()
            .find(|row| row.task_id == "t3")
            .expect("t3 row");
        assert_eq!(t3.gate.as_ref().unwrap().status, GateStatus::NotYet);
        assert!(t3.lead_days.is_none());

        assert!(review.basis.tasks.contains("3 tasks"));
        assert!(review.basis.gate_pass_rate.contains("1 passed / 2"));

        // The database value `not_yet` converts to the wire name `not-yet`.
        let task = db.task("t3").unwrap();
        assert_eq!(task.gates[0].status, GateStatus::NotYet);
    }

    #[test]
    fn export_contains_every_table_and_parses_json_columns() {
        let db = Db::open_in_memory().unwrap();
        seed(&db);
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO events (id, at, kind, actor, project_id, task_id, payload, summary)
                 VALUES ('e1', 1000, 'gate_attempted', 'user', 'p1', 't1',
                         '{\"pass\":true}', 'gate attempt')",
                [],
            )
            .map_err(StoreError::from)
        })
        .unwrap();

        let export = db.export_store(6000).unwrap();
        let value: Value = serde_json::from_str(&export).expect("valid JSON");
        assert_eq!(value["antik"], json!(1));
        assert_eq!(value["schemaVersion"], json!(6));
        assert_eq!(value["exportedAt"], json!(iso(6000)));
        assert_eq!(value["tables"]["projects"][0]["id"], json!("p1"));
        assert_eq!(value["tables"]["tasks"][0]["id"], json!("t1"));
        assert_eq!(value["tables"]["gates"][0]["rubric_json"], json!([]));
        assert_eq!(value["tables"]["events"][0]["payload"]["pass"], json!(true));
        assert_eq!(value["tables"]["sessions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn empty_bootstrap_has_defaults() {
        let db = Db::open_in_memory().unwrap();
        let bootstrap = db.bootstrap().unwrap();
        assert!(bootstrap.projects.is_empty());
        assert!(bootstrap.concepts.is_empty());
        assert!(bootstrap.events.is_empty());
        assert_eq!(bootstrap.schema_version, 6);
        assert_eq!(bootstrap.revision, 0);
        assert_eq!(bootstrap.settings.model, antik_app::dto::ModelChoice::Local);
        assert!(!bootstrap.settings.write_access);
        assert_eq!(bootstrap.session.started_at, None);
    }

    #[test]
    fn project_reports_gate_handoff_and_recall_state() {
        let db = Db::open_in_memory().unwrap();
        seed(&db);
        let bootstrap = db.bootstrap().unwrap();
        let project = &bootstrap.projects[0];

        assert_eq!(project.name, "kestrel");
        let task = project.current_task.as_ref().expect("current task");
        assert_eq!(task.phase, antik_app::dto::Phase::Verify);
        assert!(task.pending_gate);
        let handoff = project.handoff.as_ref().expect("handoff");
        assert!(handoff.recalled);
        assert_eq!(handoff.at, iso(3000));
    }

    #[test]
    fn events_are_append_only() {
        let db = Db::open_in_memory().unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO events (id, at, kind, actor, summary)
                 VALUES ('e1', 1000, 'project_created', 'user', 'project created')",
                [],
            )
            .map_err(StoreError::from)
        })
        .unwrap();

        let update = db.with_conn(|conn| {
            conn.execute("UPDATE events SET summary = 'changed' WHERE id = 'e1'", [])
                .map_err(StoreError::from)
        });
        assert!(update.is_err(), "events must reject updates");

        let delete = db.with_conn(|conn| {
            conn.execute("DELETE FROM events WHERE id = 'e1'", [])
                .map_err(StoreError::from)
        });
        assert!(delete.is_err(), "events must reject deletes");
    }

    #[test]
    fn bootstrap_returns_newest_events_first() {
        let db = Db::open_in_memory().unwrap();
        db.with_conn(|conn| {
            for (id, at) in [("e1", 1000), ("e2", 2000), ("e3", 3000)] {
                conn.execute(
                    "INSERT INTO events (id, at, kind, actor, summary)
                     VALUES (?1, ?2, 'note_added', 'user', 'note')",
                    params![id, at],
                )
                .map_err(StoreError::from)?;
            }
            Ok(())
        })
        .unwrap();

        let events = db.bootstrap().unwrap().events;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, EventKind::NoteAdded);
        assert_eq!(events[0].at, iso(3000));
    }

    #[test]
    fn iso_formats_known_timestamps() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert!(iso(1_700_000_000_000).starts_with("2023-11-14T"));
    }
}
