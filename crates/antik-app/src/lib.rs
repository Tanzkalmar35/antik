//! Use cases and ports. The application layer owns no SQL and no Tauri types.

#![forbid(unsafe_code)]

pub mod dto;
pub mod error;
pub mod ports;
pub mod validation;

use std::sync::Arc;

use antik_domain::concept;
use antik_domain::gate::{GateDraft, GateRubric};
use antik_domain::scope::Pattern;
use antik_domain::sensor::{CheckSpec, DEFAULT_TIMEOUT_MS};
use dto::{
    Bootstrap, CheckInput, CheckResult, ConceptReviewOutcome, ConceptStatus, ConceptSummary,
    CreateConceptInput, CreateDecisionInput, CreateDrillInput, CreateProjectInput,
    CreateQuestionInput, CreateTaskInput, DrillOutcome, DrillSummary, EventKind, EventSummary,
    GateDetail, HandoffDetail, HandoffInput, HelpLevel, PlanChatReply, PlanDraftInput,
    PlanItemInput, PlanItemKind, PlanItemStatus, PlanProposal, PlanProposalItem,
    PlanProposalSection, PlanSource, ProjectDetail, RecallOutcome, Review, RunDetail, SessionView,
    TaskDetail,
};
use error::AppError;
use ports::{
    CancelToken, ConceptRecord, DraftPlan, GateAnswer, GateConceptReview, GateContext, IdGen,
    Model, ModelTarget, NewAttempt, NewCheck, NewCheckResult, NewCheckRun, NewConcept,
    NewConceptReview, NewDecision, NewDrill, NewDrillRun, NewEvent, NewGate, NewHandoff, NewNote,
    NewPlanItem, NewPlanMessage, NewProject, NewQuestion, NewTask, NewTaskNote, NoModel, NoRunner,
    PlanChatContext, PlanContext, ProjectFiles, RunRequest, RunStop, Runner, ScannedTask,
    ScopedFile, Store,
};
use validation as check;

/// A run that has been reserved in storage and is waiting for a process slot.
/// The launch data is owned by the caller so the run can happen off the IPC
/// thread while the command returns immediately.
pub struct CheckRunStart {
    pub run_id: String,
    pub task_id: String,
    pub project_id: String,
    pub cwd: std::path::PathBuf,
    pub checks: Vec<PlannedCheck>,
}

pub struct PlannedCheck {
    pub check_id: String,
    pub label: String,
    pub spec: CheckSpec,
}

/// The services the IPC layer talks to. Cheap to clone; the store is shared.
pub struct App<S: Store> {
    store: Arc<S>,
    clock: Arc<dyn antik_domain::Clock>,
    ids: Arc<dyn IdGen>,
    files: Arc<dyn ProjectFiles>,
    runner: Arc<dyn Runner>,
    model: Arc<dyn Model>,
}

impl<S: Store> Clone for App<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            clock: Arc::clone(&self.clock),
            ids: Arc::clone(&self.ids),
            files: Arc::clone(&self.files),
            runner: Arc::clone(&self.runner),
            model: Arc::clone(&self.model),
        }
    }
}

impl<S: Store> App<S> {
    /// A build with no processes to start. Running a check says so rather than
    /// pretending; tests and headless environments use this.
    pub fn new(
        store: Arc<S>,
        clock: Arc<dyn antik_domain::Clock>,
        ids: Arc<dyn IdGen>,
        files: Arc<dyn ProjectFiles>,
    ) -> Self {
        Self::with_runner(store, clock, ids, files, Arc::new(NoRunner))
    }

    pub fn with_runner(
        store: Arc<S>,
        clock: Arc<dyn antik_domain::Clock>,
        ids: Arc<dyn IdGen>,
        files: Arc<dyn ProjectFiles>,
        runner: Arc<dyn Runner>,
    ) -> Self {
        Self::with_model(store, clock, ids, files, runner, Arc::new(NoModel))
    }

    /// A build with both lanes attached: processes and a local model.
    pub fn with_model(
        store: Arc<S>,
        clock: Arc<dyn antik_domain::Clock>,
        ids: Arc<dyn IdGen>,
        files: Arc<dyn ProjectFiles>,
        runner: Arc<dyn Runner>,
        model: Arc<dyn Model>,
    ) -> Self {
        Self {
            store,
            clock,
            ids,
            files,
            runner,
            model,
        }
    }

    fn now(&self) -> i64 {
        self.clock.now().as_millis()
    }

    fn id(&self) -> String {
        self.ids.next()
    }

    fn event(
        &self,
        at: i64,
        kind: EventKind,
        summary: impl Into<String>,
        project_id: Option<&str>,
        task_id: Option<&str>,
    ) -> NewEvent {
        NewEvent {
            id: self.id(),
            at,
            kind,
            summary: summary.into(),
            project_id: project_id.map(str::to_owned),
            task_id: task_id.map(str::to_owned),
            payload: None,
        }
    }

    /// Projection is best-effort: a file error is logged and never rolls back
    /// a committed mutation.
    fn project_to_disk(&self, project: &ProjectDetail) {
        if let Err(error) = self.files.write_project(project) {
            tracing::warn!(project_id = %project.id, error = %error, "markdown projection failed");
        }
    }

    fn project_to_disk_by_id(&self, project_id: &str) {
        match self.store.project(project_id) {
            Ok(project) => self.project_to_disk(&project),
            Err(error) => {
                tracing::warn!(project_id, error = %error, "projection read failed")
            }
        }
    }

    fn task_to_disk(&self, task: &TaskDetail) {
        self.project_to_disk_by_id(&task.project_id);
    }

    /// The import lane: external edits to the brief and notes are applied as
    /// user events before the project is returned (I4/I5 untouched). A folder
    /// copied from another installation reads the same way: its documents name
    /// that installation's project, which this store has never seen, so they
    /// are adopted and then re-projected under this project's id.
    fn import_external(&self, project: &ProjectDetail) -> Result<ProjectDetail, AppError> {
        let scanned = match self.files.scan(project) {
            Ok(scanned) => scanned,
            Err(error) => {
                tracing::warn!(project_id = %project.id, error = %error, "markdown scan failed");
                return Ok(project.clone());
            }
        };
        let mut current = project.clone();

        if let Some(brief) = scanned.brief {
            if self.document_is_ours(&project.id, &brief.project) && brief.text != current.brief {
                let at = self.now();
                let event = self.event(
                    at,
                    EventKind::BriefEdited,
                    "brief imported from markdown",
                    Some(&project.id),
                    None,
                );
                current = self
                    .store
                    .update_brief(&project.id, &brief.text, None, at, &event)?;
            }
        }

        for note in scanned.notes {
            if !self.document_is_ours(&project.id, &note.project) {
                continue;
            }
            let existing = current.notes.iter().find(|entry| entry.id == note.id);
            match existing {
                Some(entry) if entry.text == note.text && entry.title == note.title => {}
                Some(_) => {
                    let at = self.now();
                    let event = self.event(
                        at,
                        EventKind::NoteAdded,
                        "note imported from markdown",
                        Some(&project.id),
                        None,
                    );
                    current = self.store.update_note(
                        &project.id,
                        &note.id,
                        &note.title,
                        &note.text,
                        at,
                        &event,
                    )?;
                }
                None => {
                    let at = self.now();
                    let event = self.event(
                        at,
                        EventKind::NoteAdded,
                        "note imported from markdown",
                        Some(&project.id),
                        None,
                    );
                    current = self.store.add_note(
                        &project.id,
                        NewNote {
                            id: note.id,
                            at,
                            title: note.title,
                            text: note.text,
                        },
                        &event,
                    )?;
                }
            }
        }

        let mut created = false;
        for task in scanned.tasks {
            if !self.document_is_ours(&project.id, &task.project) {
                continue;
            }
            match self.store.task(&task.id) {
                // Already known: the document is this task's projection, so the
                // store stays the source of truth.
                Ok(existing) => {
                    if existing.project_id != project.id {
                        tracing::warn!(task = %task.id, "task document names another project's task");
                    }
                    continue;
                }
                Err(AppError::NotFound(_)) => {}
                Err(error) => {
                    tracing::warn!(task = %task.id, error = %error, "task lookup failed");
                    continue;
                }
            }
            match self.import_task(&project.id, &task) {
                Ok(()) => created = true,
                Err(error) => {
                    tracing::warn!(task = %task.id, error = %error, "task import failed");
                }
            }
        }
        if created {
            // `create_task` makes the task it inserts current. A document that
            // arrived from disk must not move the batch the user is on.
            if let Some(previous) = project.current_task_id.as_deref() {
                let at = self.now();
                let event = self.event(
                    at,
                    EventKind::TaskSwitched,
                    "current task restored after import",
                    Some(&project.id),
                    Some(previous),
                );
                if let Err(error) = self.store.set_current_task(&project.id, previous, &event) {
                    tracing::warn!(project_id = %project.id, error = %error, "current task restore failed");
                }
            }
            current = self.store.project(&project.id)?;
        }

        Ok(current)
    }

    /// A task document carries its own id, so importing one is a create in the
    /// plan phase: the tree in the file becomes the plan, and the app's
    /// projection of phase, lane and history takes over from there.
    fn import_task(&self, project_id: &str, scanned: &ScannedTask) -> Result<(), AppError> {
        let title = check::title(&scanned.title)?;
        let scope = check::optional(&scanned.scope, "scope", check::REASON_MAX)?;
        let description = check::optional(&scanned.description, "prediction", check::ANSWER_MAX)?;
        let at = self.now();
        let task = NewTask {
            id: scanned.id.clone(),
            project_id: project_id.to_owned(),
            title,
            scope,
            prediction: description,
            at,
        };
        let event = self.event(
            at,
            EventKind::TaskPlanned,
            "task imported from markdown".to_owned(),
            Some(project_id),
            Some(&scanned.id),
        );
        self.store.create_task(task, &event)?;

        let mut items = Vec::new();
        for (index, section) in scanned.sections.iter().enumerate() {
            let Ok(title) = check::title(&section.title) else {
                continue;
            };
            let Ok(body) = check::optional(&section.body, "plan detail", check::ANSWER_MAX) else {
                continue;
            };
            let section_id = self.id();
            items.push(NewPlanItem {
                id: section_id.clone(),
                parent_id: None,
                kind: PlanItemKind::Section,
                title,
                body,
                position: (index + 1) as i32,
                status: PlanItemStatus::Open,
                source: PlanSource::User,
                check_id: None,
                at,
            });
            for (order, subtask) in section.subtasks.iter().enumerate() {
                let Ok(title) = check::title(&subtask.title) else {
                    continue;
                };
                let Ok(body) = check::optional(&subtask.body, "plan detail", check::ANSWER_MAX)
                else {
                    continue;
                };
                items.push(NewPlanItem {
                    id: self.id(),
                    parent_id: Some(section_id.clone()),
                    kind: PlanItemKind::Subtask,
                    title,
                    body,
                    position: (order + 1) as i32,
                    status: if subtask.done {
                        PlanItemStatus::Done
                    } else {
                        PlanItemStatus::Open
                    },
                    source: PlanSource::User,
                    check_id: None,
                    at,
                });
            }
        }
        for (index, dod) in scanned.dods.iter().enumerate() {
            let Ok(title) = check::title(&dod.title) else {
                continue;
            };
            let Ok(body) = check::optional(&dod.body, "plan detail", check::ANSWER_MAX) else {
                continue;
            };
            items.push(NewPlanItem {
                id: self.id(),
                parent_id: None,
                kind: PlanItemKind::Dod,
                title,
                body,
                position: (index + 1) as i32,
                status: if dod.done {
                    PlanItemStatus::Done
                } else {
                    PlanItemStatus::Open
                },
                source: PlanSource::User,
                check_id: None,
                at,
            });
        }
        if !items.is_empty() {
            let summary = format!("{} plan items imported", items.len());
            let event = self.event(
                at,
                EventKind::PlanItemAdded,
                summary,
                Some(project_id),
                Some(&scanned.id),
            );
            self.store.add_plan_items(&scanned.id, items, at, &event)?;
        }
        Ok(())
    }

    /// A document belongs here when its frontmatter names this project, or when
    /// it names a project this store has never seen — the mark of a folder that
    /// arrived from another machine. A document that names another local
    /// project is that project's, even if it sits in this folder.
    fn document_is_ours(&self, project_id: &str, owner: &str) -> bool {
        if owner == project_id {
            return true;
        }
        match self.store.project(owner) {
            Ok(_) => false,
            Err(AppError::NotFound(_)) => true,
            Err(error) => {
                tracing::warn!(project = owner, error = %error, "document ownership check failed");
                false
            }
        }
    }

    /// Launch reconciles every project with its folder before the shell draws,
    /// so documents that arrived on disk — a `git pull` of a repository with
    /// `.antik` inside, or a folder copied from another machine — are read once
    /// at startup rather than when the project is next opened.
    pub fn bootstrap(&self) -> Result<Bootstrap, AppError> {
        let boot = self.store.bootstrap()?;
        let mut changed = false;
        for summary in &boot.projects {
            changed |= self.refresh_from_disk(&summary.id);
        }
        if changed {
            return self.store.bootstrap();
        }
        Ok(boot)
    }

    /// Best-effort: a folder that cannot be read is logged and the launch goes
    /// on, because a document error must not lock the user out of the app.
    fn refresh_from_disk(&self, project_id: &str) -> bool {
        let project = match self.store.project(project_id) {
            Ok(project) => project,
            Err(error) => {
                tracing::warn!(project_id, error = %error, "startup import: project read failed");
                return false;
            }
        };
        match self.import_external(&project) {
            Ok(imported) if imported.revision != project.revision => {
                self.project_to_disk(&imported);
                true
            }
            Ok(_) => false,
            Err(error) => {
                tracing::warn!(project_id, error = %error, "startup import failed");
                false
            }
        }
    }

    pub fn create_project(&self, input: CreateProjectInput) -> Result<ProjectDetail, AppError> {
        let name = check::title(&input.name)?;
        let brief = check::optional(&input.brief, "brief", check::NOTE_MAX)?;
        let at = self.now();
        let id = self.id();
        let task_id = self.id();
        let events = [
            self.event(
                at,
                EventKind::ProjectCreated,
                format!("project created · {name}"),
                Some(&id),
                None,
            ),
            self.event(
                at,
                EventKind::TaskPlanned,
                "first task planned",
                Some(&id),
                Some(&task_id),
            ),
        ];
        let project = NewProject {
            id: id.clone(),
            task_id: task_id.clone(),
            name,
            brief,
            slug: id.clone(),
            at,
        };
        let task = NewTask {
            id: task_id,
            project_id: id,
            title: "First task".to_owned(),
            scope: String::new(),
            prediction: String::new(),
            at,
        };
        let project = self.store.create_project(project, task, &events)?;
        self.project_to_disk(&project);
        Ok(project)
    }

    /// Attaches a local repository to a project, or clears it with `None`.
    /// The path is stored canonical, so checks and the read scope resolve the
    /// same directory on every call.
    pub fn set_project_path(
        &self,
        project_id: &str,
        path: Option<&str>,
    ) -> Result<ProjectDetail, AppError> {
        let repo = match path.map(str::trim).filter(|entry| !entry.is_empty()) {
            Some(raw) => {
                if raw.len() > 4096 {
                    return Err(AppError::Invalid(
                        "the path is longer than 4096 bytes".to_owned(),
                    ));
                }
                let candidate = std::path::Path::new(raw);
                if !candidate.is_absolute() {
                    return Err(AppError::Invalid(
                        "a project path must be absolute".to_owned(),
                    ));
                }
                let resolved = candidate.canonicalize().map_err(|_| {
                    AppError::Invalid("that directory does not exist on this machine".to_owned())
                })?;
                if !resolved.is_dir() {
                    return Err(AppError::Invalid(
                        "a project path must be a directory".to_owned(),
                    ));
                }
                Some(resolved.to_string_lossy().into_owned())
            }
            None => None,
        };
        let at = self.now();
        let summary = match &repo {
            Some(path) => format!("project path set · {path}"),
            None => "project path cleared".to_owned(),
        };
        let event = self.event(
            at,
            EventKind::ReadScopeChanged,
            summary,
            Some(project_id),
            None,
        );
        let project = self
            .store
            .set_project_path(project_id, repo.as_deref(), at, &event)?;
        // A directory that already carries antik documents is read before the
        // projection writes over them: that is how a cloned repository's brief
        // and notes arrive on a new machine.
        let project = match self.import_external(&project) {
            Ok(imported) => imported,
            Err(error) => {
                tracing::warn!(project_id, error = %error, "import after attaching a path failed");
                project
            }
        };
        self.project_to_disk(&project);
        Ok(project)
    }

    /// Moves the project's current pointer to another of its batches. The batch
    /// that was current keeps its phase, notes and gates, so switching back
    /// resumes where it stopped rather than starting over.
    pub fn set_current_task(
        &self,
        project_id: &str,
        task_id: &str,
    ) -> Result<ProjectDetail, AppError> {
        let task = self.store.task(task_id)?;
        if task.project_id != project_id {
            return Err(AppError::NotFound("task not found".to_owned()));
        }
        let event = self.event(
            self.now(),
            EventKind::TaskSwitched,
            format!("current task switched · {}", task.title),
            Some(project_id),
            Some(task_id),
        );
        self.store.set_current_task(project_id, task_id, &event)
    }

    pub fn get_project(&self, id: &str) -> Result<ProjectDetail, AppError> {
        let project = self.store.project(id)?;
        let project = self.import_external(&project)?;
        self.project_to_disk(&project);
        Ok(project)
    }

    pub fn update_brief(
        &self,
        project_id: &str,
        body: &str,
        expected_revision: Option<u32>,
    ) -> Result<ProjectDetail, AppError> {
        let body = check::optional(body, "brief", check::NOTE_MAX)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::BriefEdited,
            format!("brief edited · {} chars", body.chars().count()),
            Some(project_id),
            None,
        );
        let project = self
            .store
            .update_brief(project_id, &body, expected_revision, at, &event)?;
        self.project_to_disk(&project);
        Ok(project)
    }

    pub fn add_note(
        &self,
        project_id: &str,
        title: &str,
        text: &str,
    ) -> Result<ProjectDetail, AppError> {
        let title = check::optional(title, "title", check::TITLE_MAX)?;
        let text = check::text(text, "note", check::NOTE_MAX)?;
        let note = NewNote {
            id: self.id(),
            at: self.now(),
            title,
            text,
        };
        let event = self.event(
            note.at,
            EventKind::NoteAdded,
            format!("note added · {} chars", note.text.chars().count()),
            Some(project_id),
            None,
        );
        let project = self.store.add_note(project_id, note, &event)?;
        self.project_to_disk(&project);
        Ok(project)
    }

    pub fn delete_note(&self, project_id: &str, note_id: &str) -> Result<ProjectDetail, AppError> {
        let at = self.now();
        let event = self.event(
            at,
            EventKind::NoteDeleted,
            "note deleted",
            Some(project_id),
            None,
        );
        let project = self.store.delete_note(project_id, note_id, &event)?;
        self.project_to_disk(&project);
        Ok(project)
    }

    pub fn create_task(
        &self,
        project_id: &str,
        input: CreateTaskInput,
    ) -> Result<TaskDetail, AppError> {
        let title = check::title(&input.title)?;
        let scope = check::optional(&input.scope, "scope", check::REASON_MAX)?;
        let prediction = check::optional(&input.prediction, "prediction", check::ANSWER_MAX)?;
        let at = self.now();
        let task = NewTask {
            id: self.id(),
            project_id: project_id.to_owned(),
            title: title.clone(),
            scope,
            prediction,
            at,
        };
        let event = self.event(
            at,
            EventKind::TaskPlanned,
            format!("task planned · {title}"),
            Some(project_id),
            Some(&task.id),
        );
        let task = self.store.create_task(task, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    /// Removes a task and everything that belonged to it. The event log is
    /// append-only, so the deletion is recorded there rather than erased.
    pub fn delete_task(&self, task_id: &str) -> Result<ProjectDetail, AppError> {
        let task = self.store.task(task_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::TaskDeleted,
            format!("task deleted · {}", task.title),
            Some(&task.project_id),
            Some(task_id),
        );
        let project = self.store.delete_task(task_id, &event)?;
        if let Err(error) = self.files.remove_task_document(&project, task_id) {
            tracing::warn!(task_id, error = %error, "task document removal failed");
        }
        self.project_to_disk(&project);
        Ok(project)
    }

    pub fn get_task(&self, id: &str) -> Result<TaskDetail, AppError> {
        self.store.task(id)
    }

    pub fn write_prediction(&self, task_id: &str, text: &str) -> Result<TaskDetail, AppError> {
        let text = check::text(text, "prediction", check::ANSWER_MAX)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::PredictionWritten,
            format!("prediction written · {} chars", text.chars().count()),
            None,
            Some(task_id),
        );
        let task = self.store.write_prediction(task_id, &text, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn approve_plan(&self, task_id: &str) -> Result<TaskDetail, AppError> {
        let at = self.now();
        let event = self.event(
            at,
            EventKind::PlanApproved,
            "plan approved",
            None,
            Some(task_id),
        );
        let task = self.store.approve_plan(task_id, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn add_task_note(&self, task_id: &str, text: &str) -> Result<TaskDetail, AppError> {
        let text = check::text(text, "note", check::NOTE_MAX)?;
        let note = NewTaskNote {
            id: self.id(),
            at: self.now(),
            text,
        };
        let event = self.event(
            note.at,
            EventKind::NoteAdded,
            format!("working note added · {} chars", note.text.chars().count()),
            None,
            Some(task_id),
        );
        let task = self.store.add_task_note(task_id, note, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn add_plan_item(
        &self,
        task_id: &str,
        input: PlanItemInput,
    ) -> Result<TaskDetail, AppError> {
        let title = check::title(&input.title)?;
        let body = check::optional(&input.body, "plan item detail", check::ANSWER_MAX)?;
        let task = self.store.task(task_id)?;
        let position = task
            .plan_items
            .iter()
            .filter(|item| item.kind == input.kind && item.parent_id == input.parent_id)
            .map(|item| item.position)
            .max()
            .unwrap_or(0)
            + 1;
        let at = self.now();
        let item = NewPlanItem {
            id: self.id(),
            parent_id: input.parent_id,
            kind: input.kind,
            title: title.clone(),
            body,
            position,
            status: PlanItemStatus::Open,
            source: PlanSource::User,
            check_id: input.check_id,
            at,
        };
        let event = self.event(
            at,
            EventKind::PlanItemAdded,
            format!("plan item added · {title}"),
            None,
            Some(task_id),
        );
        let task = self.store.add_plan_items(task_id, vec![item], at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn update_plan_item(
        &self,
        item_id: &str,
        title: &str,
        body: &str,
    ) -> Result<TaskDetail, AppError> {
        let title = check::title(title)?;
        let body = check::optional(body, "plan item detail", check::ANSWER_MAX)?;
        let task_id = self.store.plan_item_task(item_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::PlanItemUpdated,
            format!("plan item edited · {title}"),
            None,
            Some(&task_id),
        );
        let task = self
            .store
            .update_plan_item(item_id, &title, &body, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn set_plan_item_status(&self, item_id: &str, done: bool) -> Result<TaskDetail, AppError> {
        let task_id = self.store.plan_item_task(item_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::PlanItemCompleted,
            if done {
                "plan item completed"
            } else {
                "plan item reopened"
            },
            None,
            Some(&task_id),
        );
        let task = self.store.set_plan_item_status(item_id, done, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn move_plan_item(&self, item_id: &str, direction: i32) -> Result<TaskDetail, AppError> {
        let direction = direction.clamp(-1, 1);
        let task_id = self.store.plan_item_task(item_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::PlanItemMoved,
            "plan item reordered",
            None,
            Some(&task_id),
        );
        let task = self.store.move_plan_item(item_id, direction, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn delete_plan_item(&self, item_id: &str) -> Result<TaskDetail, AppError> {
        let task_id = self.store.plan_item_task(item_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::PlanItemRemoved,
            "plan item removed",
            None,
            Some(&task_id),
        );
        let task = self.store.delete_plan_item(item_id, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    /// Adds the user's turn before asking the model. A model failure is
    /// returned as a note, so the turn is never lost from the document.
    pub fn send_plan_message(&self, task_id: &str, text: &str) -> Result<PlanChatReply, AppError> {
        let text = check::text(text, "message", check::ANSWER_MAX)?;
        let task = self.store.task(task_id)?;
        if task.prediction.trim().is_empty() {
            return Err(AppError::Invalid(
                "write the approach before starting the discussion".to_owned(),
            ));
        }
        let at = self.now();
        let user = NewPlanMessage {
            id: self.id(),
            at,
            role: dto::PlanMessageRole::User,
            content: text,
            model: String::new(),
        };
        let event = self.event(
            at,
            EventKind::PlanMessageAdded,
            "plan discussion · your turn",
            None,
            Some(task_id),
        );
        let task = self.store.add_plan_message(task_id, user, &event)?;
        self.task_to_disk(&task);

        let settings = self.store.settings()?;
        let target = match self.model_target(&settings) {
            Ok(target) => target,
            Err(error) => {
                return Ok(PlanChatReply {
                    task,
                    note: Some(error.to_string()),
                });
            }
        };
        let context = self.plan_context(&task);
        let answer = match self.model.plan_chat(&target, &context) {
            Ok(answer) => answer,
            Err(error) => {
                return Ok(PlanChatReply {
                    task,
                    note: Some(error.message),
                });
            }
        };
        if answer.trim().is_empty() {
            return Ok(PlanChatReply {
                task,
                note: Some("the model answered with nothing".to_owned()),
            });
        }
        let at = self.now();
        let assistant = NewPlanMessage {
            id: self.id(),
            at,
            role: dto::PlanMessageRole::Assistant,
            content: answer.trim().to_owned(),
            model: settings.model_name.clone(),
        };
        let event = self.event(
            at,
            EventKind::PlanMessageAdded,
            "plan discussion · model reply",
            None,
            Some(task_id),
        );
        let task = self.store.add_plan_message(task_id, assistant, &event)?;
        self.task_to_disk(&task);
        Ok(PlanChatReply { task, note: None })
    }

    /// Asks for a structured proposal. Nothing is stored here; acceptance is a
    /// separate, explicit command.
    pub fn suggest_plan(&self, task_id: &str) -> Result<PlanProposal, AppError> {
        let task = self.store.task(task_id)?;
        if task.prediction.trim().is_empty() {
            return Err(AppError::Invalid(
                "write the approach before asking for a plan".to_owned(),
            ));
        }
        let settings = self.store.settings()?;
        let target = self.model_target(&settings)?;
        let context = self.plan_context(&task);
        let draft = self
            .model
            .suggest_plan(&target, &context)
            .map_err(|error| AppError::ModelUnavailable(error.message))?;
        proposal_from_draft(&settings.model_name, draft)
    }

    pub fn accept_plan_proposal(
        &self,
        task_id: &str,
        input: PlanDraftInput,
    ) -> Result<TaskDetail, AppError> {
        let task = self.store.task(task_id)?;
        let mut items = Vec::new();
        let at = self.now();
        let mut section_position = task
            .plan_items
            .iter()
            .filter(|item| item.kind == PlanItemKind::Section)
            .map(|item| item.position)
            .max()
            .unwrap_or(0)
            + 1;
        for section in input.sections.into_iter().take(20) {
            let title = check::title(&section.title)?;
            let id = self.id();
            items.push(NewPlanItem {
                id: id.clone(),
                parent_id: None,
                kind: PlanItemKind::Section,
                title,
                body: String::new(),
                position: section_position,
                status: PlanItemStatus::Open,
                source: PlanSource::Model,
                check_id: None,
                at,
            });
            section_position += 1;
            let mut subtask_position = 1;
            for subtask in section.subtasks.into_iter().take(50) {
                let title = check::title(&subtask.title)?;
                let body = check::optional(&subtask.body, "plan item detail", check::ANSWER_MAX)?;
                items.push(NewPlanItem {
                    id: self.id(),
                    parent_id: Some(id.clone()),
                    kind: PlanItemKind::Subtask,
                    title,
                    body,
                    position: subtask_position,
                    status: PlanItemStatus::Open,
                    source: PlanSource::Model,
                    check_id: None,
                    at,
                });
                subtask_position += 1;
            }
        }
        let mut dod_position = task
            .plan_items
            .iter()
            .filter(|item| item.kind == PlanItemKind::Dod)
            .map(|item| item.position)
            .max()
            .unwrap_or(0)
            + 1;
        for dod in input.dods.into_iter().take(50) {
            let title = check::title(&dod.title)?;
            let body = check::optional(&dod.body, "definition of done detail", check::ANSWER_MAX)?;
            items.push(NewPlanItem {
                id: self.id(),
                parent_id: None,
                kind: PlanItemKind::Dod,
                title,
                body,
                position: dod_position,
                status: PlanItemStatus::Open,
                source: PlanSource::Model,
                check_id: None,
                at,
            });
            dod_position += 1;
        }
        if items.is_empty() {
            return Err(AppError::Invalid(
                "that proposal has no plan items to add".to_owned(),
            ));
        }
        let count = items.len();
        let event = self.event(
            at,
            EventKind::PlanItemAdded,
            format!("plan proposal accepted · {count} items"),
            None,
            Some(task_id),
        );
        let task = self.store.add_plan_items(task_id, items, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    /// The plan lane never carries source files or the path scope. The model
    /// works from the developer's stated outcome, the current plan and the
    /// discussion, so it cannot drift into implementation detail.
    fn plan_context(&self, task: &TaskDetail) -> PlanChatContext {
        PlanChatContext {
            title: task.title.clone(),
            prediction: task.prediction.clone(),
            items: task.plan_items.clone(),
            messages: task.plan_messages.clone(),
        }
    }

    pub fn record_attempt(&self, task_id: &str, text: &str) -> Result<TaskDetail, AppError> {
        let text = check::text(text, "attempt", check::NOTE_MAX)?;
        let attempt = NewAttempt {
            id: self.id(),
            at: self.now(),
            text,
        };
        let event = self.event(
            attempt.at,
            EventKind::AttemptRecorded,
            format!("attempt recorded · {} chars", attempt.text.chars().count()),
            None,
            Some(task_id),
        );
        let task = self.store.record_attempt(task_id, attempt, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn close_implement(&self, task_id: &str) -> Result<TaskDetail, AppError> {
        let at = self.now();
        let event = self.event(
            at,
            EventKind::ImplementClosed,
            "implement closed",
            None,
            Some(task_id),
        );
        let task = self.store.close_implement(task_id, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn write_handoff(
        &self,
        task_id: &str,
        input: HandoffInput,
    ) -> Result<TaskDetail, AppError> {
        let next = check::text(&input.next, "next line", check::ANSWER_MAX)?;
        let blocking = check::text(&input.blocking, "blocking line", check::ANSWER_MAX)?;
        let thinking = check::text(&input.thinking, "thinking line", check::ANSWER_MAX)?;
        let at = self.now();
        let handoff = NewHandoff {
            id: self.id(),
            at,
            next_line: next,
            blocking,
            thinking,
        };
        let events = [
            self.event(
                at,
                EventKind::HandoffWritten,
                "handoff written · 3 lines",
                None,
                Some(task_id),
            ),
            self.event(
                at,
                EventKind::SessionClosed,
                "session closed",
                None,
                Some(task_id),
            ),
        ];
        let task = self.store.write_handoff(task_id, handoff, &events)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn start_session(&self, project_id: &str) -> Result<SessionView, AppError> {
        let at = self.now();
        let session_id = self.id();
        let event = self.event(
            at,
            EventKind::SessionOpened,
            "session opened",
            Some(project_id),
            None,
        );
        self.store
            .start_session(project_id, &session_id, at, &event)
    }

    pub fn submit_recall(&self, project_id: &str, text: &str) -> Result<RecallOutcome, AppError> {
        let text = check::text(text, "recall", check::ANSWER_MAX)?;
        // Matching uses the handoff text, but the text is not returned until
        // the recall is stored (I2).
        let matched = match self.store.latest_handoff(project_id)? {
            Some(handoff) => check::matched_lines(&handoff_body(&handoff), &text),
            None => 0,
        };
        let at = self.now();
        let session_id = self.id();
        let event = self.event(
            at,
            EventKind::RecallStored,
            format!("recall stored · {matched} lines matched"),
            Some(project_id),
            None,
        );
        self.store
            .submit_recall(project_id, &session_id, &text, matched, at, &event)
    }

    pub fn get_handoff(&self, project_id: &str) -> Result<HandoffDetail, AppError> {
        self.store.handoff_for(project_id)
    }

    pub fn close_session(&self, project_id: &str) -> Result<(), AppError> {
        let at = self.now();
        let event = self.event(
            at,
            EventKind::SessionClosed,
            "session closed",
            Some(project_id),
            None,
        );
        self.store.close_session(project_id, at, &event)
    }

    pub fn list_events(
        &self,
        project_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<EventSummary>, AppError> {
        self.store.events(project_id, limit.clamp(1, 500))
    }

    pub fn review(&self, days: u32) -> Result<Review, AppError> {
        let days = days.clamp(1, 365);
        let cutoff = self.now() - i64::from(days) * 86_400_000;
        self.store.review(days, cutoff)
    }

    pub fn export_store(&self) -> Result<String, AppError> {
        self.store.export_store(self.now())
    }

    // ---- Decisions and questions -----------------------------------------

    /// A decision carries its reason and its cost. Both are required, because
    /// an unnamed cost is a preference wearing a decision's clothes.
    pub fn add_decision(
        &self,
        project_id: &str,
        input: CreateDecisionInput,
    ) -> Result<ProjectDetail, AppError> {
        let title = check::title(&input.title)?;
        let why = check::text(&input.why, "why", check::REASON_MAX)?;
        let cost = check::text(&input.cost, "cost", check::REASON_MAX)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::DecisionRecorded,
            format!("decision recorded · {title}"),
            Some(project_id),
            input.task_id.as_deref(),
        );
        let project = self.store.add_decision(
            project_id,
            NewDecision {
                id: self.id(),
                at,
                title,
                why,
                cost,
                task_id: input.task_id,
            },
            &event,
        )?;
        self.project_to_disk(&project);
        Ok(project)
    }

    pub fn add_question(
        &self,
        project_id: &str,
        input: CreateQuestionInput,
    ) -> Result<ProjectDetail, AppError> {
        let text = check::text(&input.text, "question", check::ANSWER_MAX)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::QuestionRecorded,
            "question recorded",
            Some(project_id),
            None,
        );
        let project = self.store.add_question(
            project_id,
            NewQuestion {
                id: self.id(),
                at,
                text,
            },
            &event,
        )?;
        self.project_to_disk(&project);
        Ok(project)
    }

    /// Answering keeps the question in history. It is not a deletion.
    pub fn answer_question(
        &self,
        question_id: &str,
        answer: &str,
    ) -> Result<ProjectDetail, AppError> {
        let answer = check::text(answer, "answer", check::ANSWER_MAX)?;
        let project_id = self.store.question_project(question_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::QuestionAnswered,
            "question answered",
            Some(&project_id),
            None,
        );
        let project = self
            .store
            .answer_question(question_id, &answer, at, &event)?;
        self.project_to_disk(&project);
        Ok(project)
    }

    // ---- Sensors ---------------------------------------------------------

    /// A check is authored as program plus arguments; the spec is validated
    /// against the allowlist before it is stored, and again before it runs.
    pub fn add_check(&self, task_id: &str, input: CheckInput) -> Result<TaskDetail, AppError> {
        let label = check::title(&input.label)?;
        let detail = check::optional(&input.detail, "what the check proves", check::REASON_MAX)?;
        let spec = CheckSpec::new(&input.program, &input.args).map_err(AppError::Invalid)?;
        let spec_json = serde_json::to_string(&spec)
            .map_err(|error| AppError::Internal(format!("check spec: {error}")))?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::CheckAdded,
            format!("check added · {label}"),
            None,
            Some(task_id),
        );
        let task = self.store.add_check(
            task_id,
            NewCheck {
                id: self.id(),
                label,
                detail,
                spec_json,
            },
            &event,
        )?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn remove_check(&self, check_id: &str) -> Result<TaskDetail, AppError> {
        let task_id = self.store.check_task(check_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::CheckRemoved,
            "check removed",
            None,
            Some(&task_id),
        );
        let task = self.store.remove_check(check_id, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn check_run(&self, run_id: &str) -> Result<RunDetail, AppError> {
        self.store.check_run(run_id)
    }

    /// Reserves the run in storage and returns what to execute. No process has
    /// started yet, so a caller that gives up leaves a visible cancelled row
    /// rather than a check that silently never ran.
    pub fn start_checks(&self, task_id: &str) -> Result<CheckRunStart, AppError> {
        let task = self.store.task(task_id)?;
        if task.checks.is_empty() {
            return Err(AppError::Invalid(
                "this task has no checks to run".to_owned(),
            ));
        }
        let project = self.store.project(&task.project_id)?;
        let cwd = self.files.project_root(&project).ok_or_else(|| {
            AppError::Invalid("this build has no project folder to run checks in".to_owned())
        })?;
        let mut planned = Vec::new();
        for target in self.store.check_targets(task_id)? {
            let spec: CheckSpec = serde_json::from_str(&target.spec_json)
                .map_err(|error| AppError::Internal(format!("stored check spec: {error}")))?;
            planned.push(PlannedCheck {
                check_id: target.id,
                label: target.label,
                spec,
            });
        }
        let at = self.now();
        let run_id = self.id();
        let job_id = self.id();
        let event = self.event(
            at,
            EventKind::CheckRun,
            format!("check run started · {} checks", planned.len()),
            Some(&task.project_id),
            Some(task_id),
        );
        self.store.begin_check_run(
            NewCheckRun {
                id: run_id.clone(),
                task_id: task_id.to_owned(),
                at,
                job_id,
            },
            &event,
        )?;
        Ok(CheckRunStart {
            run_id,
            task_id: task_id.to_owned(),
            project_id: task.project_id,
            cwd,
            checks: planned,
        })
    }

    /// Runs every check in turn. The exit status is the only verdict: a
    /// timeout, a cancel or a process that cannot start fails the run and
    /// writes no results at all.
    pub fn execute_checks(
        &self,
        start: CheckRunStart,
        cancel: &CancelToken,
    ) -> Result<RunDetail, AppError> {
        let mut results = Vec::new();
        for planned in &start.checks {
            if cancel.is_cancelled() {
                return self.abandon(&start, true, "cancelled before the check started");
            }
            let output = self.runner.run(
                &RunRequest {
                    program: planned.spec.program.to_string(),
                    args: planned.spec.args.clone(),
                    cwd: start.cwd.clone(),
                    timeout_ms: DEFAULT_TIMEOUT_MS,
                },
                cancel,
            );
            match output.stop {
                RunStop::Cancelled => {
                    return self.abandon(&start, true, "cancelled while a check was running");
                }
                RunStop::TimedOut => return self.abandon(&start, false, "the run timed out"),
                RunStop::Failed(message) => return self.abandon(&start, false, &message),
                RunStop::Exited(code) => results.push(NewCheckResult {
                    check_id: planned.check_id.clone(),
                    exit_code: Some(code),
                    result: if code == 0 {
                        CheckResult::Pass
                    } else {
                        CheckResult::Fail
                    },
                    duration_ms: millis(output.duration_ms),
                    stdout_tail: output.stdout_tail,
                    stderr_tail: output.stderr_tail,
                    truncated: output.truncated,
                }),
            }
        }
        let at = self.now();
        let passed = results
            .iter()
            .filter(|result| result.result == CheckResult::Pass)
            .count();
        let event = self.event(
            at,
            EventKind::CheckResult,
            format!("check run finished · {passed} of {} passed", results.len()),
            Some(&start.project_id),
            Some(&start.task_id),
        );
        self.store
            .finish_check_run(&start.run_id, &results, at, &event)
    }

    fn abandon(
        &self,
        start: &CheckRunStart,
        cancelled: bool,
        note: &str,
    ) -> Result<RunDetail, AppError> {
        let at = self.now();
        let summary = if cancelled {
            format!("check run cancelled · {note}")
        } else {
            format!("check run failed · {note}")
        };
        let event = self.event(
            at,
            EventKind::CheckResult,
            summary,
            Some(&start.project_id),
            Some(&start.task_id),
        );
        self.store
            .abandon_check_run(&start.run_id, cancelled, note, at, &event)
    }

    /// A drill is authored as one allowlisted program plus arguments; it is
    /// refused here if it would need a shell, before it reaches storage.
    pub fn add_drill(&self, input: CreateDrillInput) -> Result<DrillSummary, AppError> {
        let skill = check::title(&input.skill)?;
        let name = check::title(&input.name)?;
        let command = check::text(&input.command, "command", check::REASON_MAX)?;
        CheckSpec::from_line(&command).map_err(AppError::Invalid)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::DrillAdded,
            format!("drill added · {name}"),
            None,
            None,
        );
        self.store.add_drill(
            NewDrill {
                id: self.id(),
                skill,
                name,
                command,
            },
            &event,
        )
    }

    /// A drill is one command line, run at the projects root. Failures are
    /// kept: the attempt count moves whether the exit status is zero or not.
    pub fn run_drill(&self, drill_id: &str) -> Result<DrillOutcome, AppError> {
        let drill = self.store.drill(drill_id)?;
        let spec = CheckSpec::from_line(&drill.command).map_err(AppError::Invalid)?;
        let cwd = self.files.sandbox_root().ok_or_else(|| {
            AppError::Invalid("this build has no folder to run drills in".to_owned())
        })?;
        let at = self.now();
        let output = self.runner.run(
            &RunRequest {
                program: spec.program.to_string(),
                args: spec.args,
                cwd,
                timeout_ms: DEFAULT_TIMEOUT_MS,
            },
            &CancelToken::new(),
        );
        let duration_ms = millis(output.duration_ms);
        let (passed, exit_code, note) = match output.stop {
            RunStop::Exited(code) => (code == 0, Some(code), format!("exit {code}")),
            RunStop::TimedOut => (false, None, "timed out".to_owned()),
            RunStop::Cancelled => (false, None, "cancelled".to_owned()),
            RunStop::Failed(message) => (false, None, message),
        };
        let event = self.event(
            at,
            EventKind::DrillRun,
            format!("drill run · {} · {note}", drill.name),
            None,
            None,
        );
        let tail = if output.stdout_tail.trim().is_empty() {
            output.stderr_tail
        } else {
            output.stdout_tail
        };
        self.store.record_drill_run(
            drill_id,
            NewDrillRun {
                id: self.id(),
                job_id: self.id(),
                at,
                exit_code,
                duration_ms,
                passed,
                output: tail,
            },
            &event,
        )
    }

    // ---- Gates -----------------------------------------------------------

    /// Drafts one app-authored gate from the task. The rubric is stored with
    /// the gate so later edits cannot change what an answer is scored against.
    pub fn draft_gate(&self, task_id: &str) -> Result<TaskDetail, AppError> {
        let task = self.store.task(task_id)?;
        if task.phase != dto::Phase::Verify {
            return Err(AppError::Invalid(
                "a gate is drafted when the task is in verify".to_owned(),
            ));
        }
        if task
            .gates
            .iter()
            .any(|gate| matches!(gate.status, dto::GateStatus::Due | dto::GateStatus::Failed))
        {
            return Err(AppError::Invalid(
                "resolve the open gate before drafting another".to_owned(),
            ));
        }

        let at = self.now();
        let due = self.store.due_concept_for_task(task_id, at)?;
        let (mut draft, concept_id, concept, mut summary) = match due {
            Some(concept) => {
                let summary = format!(
                    "gate drafted from concept {} · {}",
                    concept.id, concept.name
                );
                (
                    GateDraft::from_concept(&concept.name, &concept.definition, &concept.area),
                    Some(concept.id.clone()),
                    Some(concept),
                    summary,
                )
            }
            None => (
                GateDraft::from_task(&task.title, &task.scope),
                None,
                None,
                "gate drafted from the task".to_owned(),
            ),
        };

        // The model may phrase the question; it may not touch the rubric, which
        // is built from the task above and decides every later score (I1).
        let settings = self.store.settings()?;
        let mut drafted_by = dto::GateSource::App;
        if settings.model == dto::ModelChoice::Local {
            let target = ModelTarget {
                base_url: settings.model_url.clone(),
                model: settings.model_name.clone(),
            };
            let project = self.store.project(&task.project_id)?;
            let context = GateContext {
                title: task.title.clone(),
                scope: task.scope.clone(),
                prediction: task.prediction.clone(),
                concept: concept.map(|concept| ConceptRecord {
                    id: concept.id,
                    name: concept.name,
                    definition: concept.definition,
                }),
                files: self.scoped_files(&project, &task.scope, &settings),
            };
            match self.model.draft_gate(&target, &context) {
                Ok(text) => {
                    match antik_domain::model::one_line(
                        &text.question,
                        antik_domain::model::QUESTION_MAX_CHARS,
                    ) {
                        Some(question) => {
                            draft.question = question;
                            if let Some(hint) = antik_domain::model::one_line(
                                &text.hint,
                                antik_domain::model::HINT_MAX_CHARS,
                            ) {
                                draft.hint = hint;
                            }
                            drafted_by = dto::GateSource::Model;
                            summary.push_str(" · question drafted by the local model");
                        }
                        None => {
                            summary
                                .push_str(" · the model's question was unusable, kept the app's");
                        }
                    }
                }
                Err(error) => {
                    // Fail-open: the gate runs anyway, and the log says why the
                    // question is the app's rather than the model's.
                    summary.push_str(&format!(" · model unavailable: {}", error.message));
                }
            }
        }

        let rubric_json = serde_json::to_string(&draft.rubric)
            .map_err(|error| AppError::Internal(format!("gate rubric: {error}")))?;
        let gate = NewGate {
            id: self.id(),
            task_id: task_id.to_owned(),
            concept_id,
            question: draft.question,
            hint: draft.hint,
            rubric_json,
            drafted_by,
        };
        let event = self.event(at, EventKind::GateDrafted, summary, None, Some(task_id));
        let task = self.store.draft_gate(task_id, gate, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn list_gates(&self, task_id: &str) -> Result<Vec<GateDetail>, AppError> {
        self.store.gates_for_task(task_id)
    }

    /// Scores an answer against the rubric stored at draft time. No model is
    /// consulted, and no caller can supply a status (I1).
    pub fn answer_gate(&self, gate_id: &str, answer: &str) -> Result<TaskDetail, AppError> {
        let answer = check::text(answer, "answer", check::ANSWER_MAX)?;
        let gate = self.store.gate(gate_id)?;
        if matches!(
            gate.status,
            dto::GateStatus::Passed | dto::GateStatus::Overridden
        ) {
            return Err(AppError::Invalid(
                "this gate is already resolved".to_owned(),
            ));
        }

        let rubric: GateRubric = serde_json::from_str(&gate.rubric_json)
            .map_err(|error| AppError::Internal(format!("stored gate rubric: {error}")))?;
        let score = rubric.score(&answer);
        let at = self.now();
        let mut event = self.event(
            at,
            EventKind::GateAttempted,
            format!(
                "gate 0{} {} · {} of {} criteria",
                gate.n,
                if score.passed { "met" } else { "not met" },
                score.criteria_hit,
                score.criteria_total
            ),
            None,
            Some(&gate.task_id),
        );
        event.payload = Some(serde_json::json!({ "pass": score.passed }));

        // Passing a concept gate is also an explain-back for that concept, so
        // its spaced schedule moves in the same transaction.
        let concept_review = match (score.passed, gate.concept_id.as_deref()) {
            (true, Some(concept_id)) => Some(GateConceptReview {
                id: self.id(),
                concept_id: concept_id.to_owned(),
                at,
                answer: answer.clone(),
                next_due: concept::next_due(self.clock.now(), concept::ConceptVerdict::Pass),
            }),
            _ => None,
        };
        let attempt_id = self.id();
        let task = self.store.answer_gate(
            GateAnswer {
                gate_id,
                attempt_id: &attempt_id,
                answer: &answer,
                score: &score,
                at,
                concept_review: concept_review.as_ref(),
            },
            &event,
        )?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn use_hint(&self, gate_id: &str) -> Result<TaskDetail, AppError> {
        let gate = self.store.gate(gate_id)?;
        if matches!(
            gate.status,
            dto::GateStatus::Passed | dto::GateStatus::Overridden
        ) {
            return Err(AppError::Invalid(
                "this gate is already resolved".to_owned(),
            ));
        }
        let at = self.now();
        let event = self.event(
            at,
            EventKind::HintUsed,
            format!("hint used on gate 0{}", gate.n),
            None,
            Some(&gate.task_id),
        );
        let task = self.store.use_hint(gate_id, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    /// An override is a bypass, never a pass. The reason is required and is
    /// written to the log with the state change.
    pub fn override_gate(&self, gate_id: &str, reason: &str) -> Result<TaskDetail, AppError> {
        let reason = check::sentence(reason, "override reason", check::REASON_MAX)?;
        let gate = self.store.gate(gate_id)?;
        if matches!(
            gate.status,
            dto::GateStatus::Passed | dto::GateStatus::Overridden
        ) {
            return Err(AppError::Invalid(
                "this gate is already resolved".to_owned(),
            ));
        }
        let at = self.now();
        let event = self.event(
            at,
            EventKind::GateOverridden,
            format!(
                "gate 0{} overridden · {} chars",
                gate.n,
                reason.chars().count()
            ),
            None,
            Some(&gate.task_id),
        );
        let task = self.store.override_gate(gate_id, &reason, at, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    // ---- Concepts --------------------------------------------------------

    pub fn list_concepts(
        &self,
        status: Option<ConceptStatus>,
    ) -> Result<Vec<ConceptSummary>, AppError> {
        self.store.concepts(status)
    }

    pub fn create_concept(&self, input: CreateConceptInput) -> Result<ConceptSummary, AppError> {
        let id = check::concept_id(&input.id)?;
        let name = check::title(&input.name)?;
        let area = check::text(&input.area, "area", check::TITLE_MAX)?;
        let definition = check::optional(&input.definition, "definition", check::NOTE_MAX)?;
        let at = self.now();
        let concept = NewConcept {
            id: id.clone(),
            at,
            name: name.clone(),
            area,
            definition,
        };
        let event = self.event(
            at,
            EventKind::ConceptAdded,
            format!("concept added · {name}"),
            None,
            None,
        );
        self.store.create_concept(concept, &event)
    }

    pub fn attach_concept(&self, task_id: &str, concept_id: &str) -> Result<TaskDetail, AppError> {
        let concept_id = check::concept_id(concept_id)?;
        let task = self.store.task(task_id)?;
        if task.concept_ids.iter().any(|id| id == &concept_id) {
            return Ok(task);
        }
        let at = self.now();
        let event = self.event(
            at,
            EventKind::ConceptAttached,
            format!("concept attached · {concept_id}"),
            None,
            Some(task_id),
        );
        let task = self.store.attach_concept(task_id, &concept_id, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    pub fn detach_concept(&self, task_id: &str, concept_id: &str) -> Result<TaskDetail, AppError> {
        let concept_id = check::concept_id(concept_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::ConceptDetached,
            format!("concept detached · {concept_id}"),
            None,
            Some(task_id),
        );
        let task = self.store.detach_concept(task_id, &concept_id, &event)?;
        self.task_to_disk(&task);
        Ok(task)
    }

    /// A short, deterministic explain-back check. The verdict and the next due
    /// date are app-owned; only the answer comes from the developer.
    pub fn review_concept(
        &self,
        concept_id: &str,
        answer: &str,
    ) -> Result<ConceptReviewOutcome, AppError> {
        let answer = check::text(answer, "answer", check::ANSWER_MAX)?;
        let record = self.store.concept(concept_id)?;
        let review = concept::review_concept(&record.name, &record.definition, &answer);
        let at = self.now();
        let next_due = concept::next_due(self.clock.now(), review.verdict);
        let event = self.event(
            at,
            EventKind::ConceptReviewed,
            format!(
                "concept {} · {}",
                if review.passed() { "explained" } else { "thin" },
                record.name
            ),
            None,
            None,
        );
        let concept = self.store.review_concept(
            NewConceptReview {
                id: self.id(),
                concept_id: concept_id.to_owned(),
                at,
                answer,
                verdict: review.verdict,
                next_due,
            },
            &event,
        )?;
        let outcome = ConceptReviewOutcome {
            concept,
            passed: review.passed(),
            note: review.note,
        };
        Ok(outcome)
    }

    pub fn set_help(&self, concept_id: &str, help: HelpLevel) -> Result<ConceptSummary, AppError> {
        let concept = self.store.concept(concept_id)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::HelpChanged,
            format!("help level {help} · {}", concept.name),
            None,
            None,
        );
        self.store.set_concept_help(concept_id, help, &event)
    }

    // ---- Settings --------------------------------------------------------

    pub fn set_model(&self, model: dto::ModelChoice) -> Result<dto::Settings, AppError> {
        let at = self.now();
        let event = self.event(
            at,
            EventKind::SettingChanged,
            format!("model set to {}", model.as_str()),
            None,
            None,
        );
        self.store.set_model(model, &event)
    }

    pub fn add_allowlist_path(&self, path: &str) -> Result<dto::Settings, AppError> {
        let path = check::allowlist_path(path)?;
        let current = self.store.settings()?;
        if !current.allowlist.iter().any(|entry| entry == &path) && current.allowlist.len() >= 64 {
            return Err(AppError::Invalid(
                "the read scope is capped at 64 paths".to_owned(),
            ));
        }
        let at = self.now();
        let event = self.event(
            at,
            EventKind::ReadScopeChanged,
            format!("read scope widened · {path}"),
            None,
            None,
        );
        self.store.add_allowlist_path(&path, at, &event)
    }

    pub fn remove_allowlist_path(&self, path: &str) -> Result<dto::Settings, AppError> {
        let path = check::allowlist_path(path)?;
        let at = self.now();
        let event = self.event(
            at,
            EventKind::ReadScopeChanged,
            format!("read scope narrowed · {path}"),
            None,
            None,
        );
        self.store.remove_allowlist_path(&path, &event)
    }

    /// Sets the local endpoint and model name. Only loopback is accepted, so a
    /// typo cannot quietly send a project to somebody else's server.
    pub fn set_model_endpoint(
        &self,
        base_url: &str,
        name: &str,
    ) -> Result<dto::Settings, AppError> {
        let base_url = antik_domain::model::loopback_base_url(base_url).ok_or_else(|| {
            AppError::Invalid(
                "the endpoint must be http on this machine: 127.0.0.1, localhost or [::1]"
                    .to_owned(),
            )
        })?;
        let name = name.trim();
        if name.is_empty()
            || name.len() > 100
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/'))
        {
            return Err(AppError::Invalid(
                "a model name is letters, digits and . _ - : /".to_owned(),
            ));
        }
        let at = self.now();
        let event = self.event(
            at,
            EventKind::SettingChanged,
            format!("local model set to {name} at {base_url}"),
            None,
            None,
        );
        self.store.set_model_endpoint(&base_url, name, &event)
    }

    // ---- Model -----------------------------------------------------------

    /// Asks the local model for advice on a plan. Nothing is stored: the answer
    /// is display copy, and a failed call is an error the screen can show.
    pub fn critique_plan(&self, task_id: &str) -> Result<dto::Critique, AppError> {
        let task = self.store.task(task_id)?;
        if task.prediction.trim().is_empty() {
            return Err(AppError::Invalid(
                "write the prediction before asking for critique".to_owned(),
            ));
        }
        let settings = self.store.settings()?;
        let target = self.model_target(&settings)?;
        let project = self.store.project(&task.project_id)?;
        let files = self.scoped_files(&project, &task.scope, &settings);
        let seen: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
        let context = PlanContext {
            title: task.title.clone(),
            scope: task.scope.clone(),
            batch: task.batch.clone(),
            prediction: task.prediction.clone(),
            files,
        };
        let advice = self
            .model
            .critique_plan(&target, &context)
            .map_err(|error| AppError::ModelUnavailable(error.message))?;
        let points: Vec<String> = advice
            .points
            .iter()
            .filter_map(|point| {
                antik_domain::model::one_line(point, antik_domain::model::POINT_MAX_CHARS)
            })
            .take(antik_domain::model::MAX_POINTS)
            .collect();
        if points.is_empty() {
            return Err(AppError::ModelUnavailable(
                "the model answered without usable advice".to_owned(),
            ));
        }
        Ok(dto::Critique {
            model: settings.model_name,
            points,
            files: seen,
        })
    }

    /// Reports the model lane without throwing: the Settings screen renders
    /// this, and "off" is a state, not a failure.
    pub fn model_health(&self) -> dto::ModelHealth {
        let settings = match self.store.settings() {
            Ok(settings) => settings,
            Err(error) => {
                return dto::ModelHealth {
                    configured: false,
                    reachable: false,
                    base_url: String::new(),
                    model: String::new(),
                    detail: format!("settings unavailable: {}", error.code()),
                };
            }
        };
        let base_url = settings.model_url.clone();
        let model = settings.model_name.clone();
        match settings.model {
            dto::ModelChoice::Local => {
                let target = ModelTarget {
                    base_url: base_url.clone(),
                    model: model.clone(),
                };
                let mut health = self.model.health(&target);
                health.configured = true;
                health.base_url = base_url;
                health.model = model;
                health
            }
            dto::ModelChoice::Api => dto::ModelHealth {
                configured: false,
                reachable: false,
                base_url,
                model,
                detail: "this build speaks to a local model only".to_owned(),
            },
            dto::ModelChoice::None => dto::ModelHealth {
                configured: false,
                reachable: false,
                base_url,
                model,
                detail: "the model is off; gates still run with the app's question".to_owned(),
            },
        }
    }

    fn model_target(&self, settings: &dto::Settings) -> Result<ModelTarget, AppError> {
        match settings.model {
            dto::ModelChoice::Local => Ok(ModelTarget {
                base_url: settings.model_url.clone(),
                model: settings.model_name.clone(),
            }),
            dto::ModelChoice::Api => Err(AppError::ModelUnavailable(
                "this build speaks to a local model only".to_owned(),
            )),
            dto::ModelChoice::None => Err(AppError::ModelUnavailable(
                "the model is off; the gate still runs with the app's question".to_owned(),
            )),
        }
    }

    /// The only project contents a model may receive: the batch's own scope
    /// intersected with the read-scope allowlist, confined to the project root.
    /// An empty allowlist opens nothing. An empty scope means the allowlist
    /// alone, so a batch that names no paths behaves as before.
    fn scoped_files(
        &self,
        project: &ProjectDetail,
        scope: &str,
        settings: &dto::Settings,
    ) -> Vec<ScopedFile> {
        let fence: Vec<Pattern> = settings
            .allowlist
            .iter()
            .filter_map(|raw| Pattern::parse(raw))
            .collect();
        if fence.is_empty() {
            return Vec::new();
        }
        let selected = antik_domain::scope::parse_scope(scope);
        self.files.read_scope(project, &fence, &selected)
    }
}

/// Durations cross IPC as milliseconds in a `u32`, which is a day and a half.
/// The model's plan is validated before it can be displayed as a proposal:
/// bounded counts, real titles, and no empty suggestion.
fn proposal_from_draft(model: &str, draft: DraftPlan) -> Result<PlanProposal, AppError> {
    let mut sections = Vec::new();
    for section in draft.sections.into_iter().take(20) {
        let title = check::title(&section.title)?;
        let mut subtasks = Vec::new();
        for subtask in section.subtasks.into_iter().take(50) {
            subtasks.push(PlanProposalItem {
                title: check::title(&subtask.title)?,
                body: check::optional(&subtask.body, "plan item detail", check::ANSWER_MAX)?,
            });
        }
        sections.push(PlanProposalSection { title, subtasks });
    }
    let mut dods = Vec::new();
    for dod in draft.dods.into_iter().take(50) {
        dods.push(PlanProposalItem {
            title: check::title(&dod)?,
            body: String::new(),
        });
    }
    if sections.is_empty() && dods.is_empty() {
        return Err(AppError::ModelUnavailable(
            "the model did not return a usable plan".to_owned(),
        ));
    }
    Ok(PlanProposal {
        model: model.to_owned(),
        sections,
        dods,
    })
}

fn millis(value: u64) -> u32 {
    value.min(u64::from(u32::MAX)) as u32
}

fn handoff_body(handoff: &HandoffDetail) -> String {
    format!(
        "{}\n{}\n{}",
        handoff.next, handoff.blocking, handoff.thinking
    )
}
