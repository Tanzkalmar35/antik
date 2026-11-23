use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::dto::{
    Bootstrap, CheckResult, ConceptStatus, ConceptSummary, DrillOutcome, DrillSummary, EventKind,
    EventSummary, GateDetail, GateSource, GateStatus, HandoffDetail, HelpLevel, ModelChoice,
    ModelHealth, PlanItem, PlanItemKind, PlanItemStatus, PlanMessage, ProjectDetail, RecallOutcome,
    Review, RunDetail, SessionView, Settings, TaskDetail,
};
use crate::error::AppError;
use antik_domain::concept::ConceptVerdict;
use antik_domain::gate::GateScore;
use antik_domain::time::Timestamp;

pub use antik_domain::Clock;

/// Identifiers are generated outside the store so a use case can put the same
/// id in an event and in the row it describes.
pub trait IdGen: Send + Sync {
    fn next(&self) -> String;
}

/// A project and its first task, created in one transaction.
pub struct NewProject {
    pub id: String,
    pub task_id: String,
    pub name: String,
    pub brief: String,
    /// Stable directory name; never the display name.
    pub slug: String,
    pub at: i64,
}

pub struct NewTask {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub scope: String,
    pub prediction: String,
    pub at: i64,
}

pub struct NewNote {
    pub id: String,
    pub at: i64,
    pub title: String,
    pub text: String,
}

pub struct NewTaskNote {
    pub id: String,
    pub at: i64,
    pub text: String,
}

pub struct NewPlanItem {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: PlanItemKind,
    pub title: String,
    pub body: String,
    pub position: i32,
    pub status: PlanItemStatus,
    pub source: crate::dto::PlanSource,
    pub check_id: Option<String>,
    pub at: i64,
}

pub struct NewPlanMessage {
    pub id: String,
    pub at: i64,
    pub role: crate::dto::PlanMessageRole,
    pub content: String,
    pub model: String,
}

pub struct NewAttempt {
    pub id: String,
    pub at: i64,
    pub text: String,
}

pub struct NewHandoff {
    pub id: String,
    pub at: i64,
    pub next_line: String,
    pub blocking: String,
    pub thinking: String,
}

/// A gate row. The rubric is the one stored at draft time, so editing the task
/// later cannot change what an answer was scored against.
pub struct NewGate {
    pub id: String,
    pub task_id: String,
    /// Set when the gate was drafted from a concept the task touches.
    pub concept_id: Option<String>,
    pub question: String,
    pub hint: String,
    pub rubric_json: String,
    /// The app's rubric is stored either way; this only records who phrased
    /// the question.
    pub drafted_by: GateSource,
}

/// What scoring needs: the task the gate belongs to and the rubric it carries.
pub struct GateRecord {
    pub id: String,
    pub task_id: String,
    pub concept_id: Option<String>,
    pub n: u32,
    pub status: GateStatus,
    pub rubric_json: String,
}

/// A passed gate on a concept is also a review of that concept: the ledger
/// moves and the next review is scheduled in the same transaction.
pub struct GateConceptReview {
    pub id: String,
    pub concept_id: String,
    pub at: i64,
    pub answer: String,
    pub next_due: Timestamp,
}

/// One scored answer. The fields are grouped so the store method stays
/// readable and no caller can slip a gate status into the write.
pub struct GateAnswer<'a> {
    pub gate_id: &'a str,
    pub attempt_id: &'a str,
    pub answer: &'a str,
    pub score: &'a GateScore,
    pub at: i64,
    pub concept_review: Option<&'a GateConceptReview>,
}

pub struct NewDecision {
    pub id: String,
    pub at: i64,
    pub title: String,
    pub why: String,
    pub cost: String,
    pub task_id: Option<String>,
}

pub struct NewQuestion {
    pub id: String,
    pub at: i64,
    pub text: String,
}

pub struct NewConcept {
    pub id: String,
    pub at: i64,
    pub name: String,
    pub area: String,
    pub definition: String,
}

/// The stable fields a concept review needs. The answer is never copied in.
pub struct ConceptRecord {
    pub id: String,
    pub name: String,
    pub definition: String,
}

/// One authored check. The spec is stored as JSON so a later allowlist change
/// cannot reinterpret an existing row.
pub struct NewCheck {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub spec_json: String,
}

/// What a run needs from a stored check: its id for the result row and the
/// spec to execute. Parsing the spec is the application's job, not SQL's.
pub struct CheckTarget {
    pub id: String,
    pub label: String,
    pub spec_json: String,
}

pub struct NewCheckRun {
    pub id: String,
    pub task_id: String,
    pub at: i64,
    pub job_id: String,
}

pub struct NewCheckResult {
    pub check_id: String,
    pub exit_code: Option<i32>,
    pub result: CheckResult,
    pub duration_ms: u32,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub truncated: bool,
}

/// A drill as stored: one command line, run at the projects root.
pub struct DrillRecord {
    pub id: String,
    pub name: String,
    pub command: String,
}

pub struct NewDrill {
    pub id: String,
    pub skill: String,
    pub name: String,
    pub command: String,
}

pub struct NewDrillRun {
    pub id: String,
    pub job_id: String,
    pub at: i64,
    pub exit_code: Option<i32>,
    pub duration_ms: u32,
    pub passed: bool,
    pub output: String,
}

pub struct NewConceptReview {
    pub id: String,
    pub concept_id: String,
    pub at: i64,
    pub answer: String,
    pub verdict: ConceptVerdict,
    pub next_due: Timestamp,
}

/// The event a mutation appends. One mutation is one event (I4).
pub struct NewEvent {
    pub id: String,
    pub at: i64,
    pub kind: EventKind,
    pub summary: String,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    /// Optional structured detail. Gate attempts carry `{"pass": bool}` so the
    /// review can count passes from the append-only log.
    pub payload: Option<serde_json::Value>,
}

/// One project file the model may be shown. `path` is project-relative, never
/// absolute, so nothing about the machine's layout reaches a prompt.
#[derive(Debug, Clone)]
pub struct ScopedFile {
    pub path: String,
    pub text: String,
    pub truncated: bool,
}

/// A document read from the project folder. Only the four editable kinds are
/// imported; task documents and the glossary are projections.
pub struct ScannedProject {
    pub brief: Option<String>,
    pub notes: Vec<ScannedNote>,
}

pub struct ScannedNote {
    pub id: String,
    pub title: String,
    pub text: String,
}

/// Markdown projection and import lane. A failed projection is logged but
/// never invalidates a committed mutation.
pub trait ProjectFiles: Send + Sync {
    fn write_project(&self, project: &ProjectDetail) -> Result<(), AppError>;
    /// Removes a task's projected document. Adapters that keep no documents
    /// have nothing to do.
    fn remove_task_document(
        &self,
        _project: &ProjectDetail,
        _task_id: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }
    fn scan(&self, project: &ProjectDetail) -> Result<ScannedProject, AppError>;
    /// The directory a check runs in: the attached repository when there is
    /// one, the project's app-data folder otherwise. `None` means this build
    /// has no folder to run in, and running a check is refused rather than
    /// guessed at.
    fn project_root(&self, project: &crate::dto::ProjectDetail) -> Option<PathBuf>;
    /// Drills belong to no project, so they run at the projects root.
    fn sandbox_root(&self) -> Option<PathBuf>;

    /// The files the model may see: inside `fence` (the allowlist) and, when
    /// `scope` is non-empty, inside the batch's own scope as well. Confined to
    /// the project root either way. The default opens nothing.
    fn read_scope(
        &self,
        _project: &crate::dto::ProjectDetail,
        _fence: &[antik_domain::scope::Pattern],
        _scope: &[antik_domain::scope::Pattern],
    ) -> Vec<ScopedFile> {
        Vec::new()
    }
}

/// Headless adapter for tests and environments without a project root.
pub struct NoFiles;

impl ProjectFiles for NoFiles {
    fn write_project(&self, _project: &ProjectDetail) -> Result<(), AppError> {
        Ok(())
    }

    fn scan(&self, _project: &ProjectDetail) -> Result<ScannedProject, AppError> {
        Ok(ScannedProject {
            brief: None,
            notes: Vec::new(),
        })
    }

    fn project_root(&self, _project: &crate::dto::ProjectDetail) -> Option<PathBuf> {
        None
    }

    fn sandbox_root(&self) -> Option<PathBuf> {
        None
    }
}

/// A cooperative cancel flag. The runner polls it between checks and while a
/// process is alive; a cancelled run writes no results (I3).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// One process to run. `program` is checked against the runner's allowlist
/// again at the process edge, so a spec that slipped past authoring still
/// cannot start something the domain does not name.
pub struct RunRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: u64,
}

/// How a process ended. Only `Exited` carries a verdict: the exit status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStop {
    Exited(i32),
    TimedOut,
    Cancelled,
    /// The process never started, or the tree could not be reaped.
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub stop: RunStop,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub truncated: bool,
}

/// The only thing allowed to produce a check result.
pub trait Runner: Send + Sync {
    fn run(&self, request: &RunRequest, cancel: &CancelToken) -> RunOutput;
}

/// A build without a process runner (tests, headless environments) reports
/// every run as failed-to-start rather than pretending it passed.
pub struct NoRunner;

impl Runner for NoRunner {
    fn run(&self, _request: &RunRequest, _cancel: &CancelToken) -> RunOutput {
        RunOutput {
            stop: RunStop::Failed("this build has no process runner".to_owned()),
            exit_code: None,
            duration_ms: 0,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            truncated: false,
        }
    }
}

/// Where the local model lives. Passed per call rather than held by the
/// adapter, so changing the name in Settings applies to the next request
/// instead of the next launch.
#[derive(Debug, Clone)]
pub struct ModelTarget {
    pub base_url: String,
    pub model: String,
}

/// The context a gate is drafted from. `files` are the only project contents a
/// model ever receives, and `checks` are deliberately absent: a gate asks about
/// the work, and a check's output is the runner's verdict, not the model's.
pub struct GateContext {
    pub title: String,
    pub scope: String,
    pub prediction: String,
    pub concept: Option<ConceptRecord>,
    pub files: Vec<ScopedFile>,
}

/// The context a plan critique is drafted from. Project and batch notes are
/// excluded on purpose (I6): they are the developer's thinking aloud.
pub struct PlanContext {
    pub title: String,
    pub scope: String,
    pub batch: String,
    pub prediction: String,
    pub files: Vec<ScopedFile>,
}

/// A model-authored question. The rubric is not part of it: the app owns every
/// criterion an answer is scored against.
#[derive(Debug, Clone)]
pub struct GateDraftText {
    pub question: String,
    pub hint: String,
}

#[derive(Debug, Clone)]
pub struct ModelAdvice {
    pub points: Vec<String>,
}

#[derive(Debug, Clone)]
/// The context a plan conversation and a plan proposal may see. This is the
/// high-level lane: the task, the written approach, the plan and the discussion.
/// Project files and their path scope deliberately never enter it.
pub struct PlanChatContext {
    pub title: String,
    pub prediction: String,
    pub items: Vec<PlanItem>,
    pub messages: Vec<PlanMessage>,
}

/// A model-shaped plan, before the app turns it into rows.
#[derive(Debug, Clone, Default)]
pub struct DraftPlan {
    pub sections: Vec<DraftPlanSection>,
    pub dods: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DraftPlanSection {
    pub title: String,
    pub subtasks: Vec<DraftPlanSubtask>,
}

#[derive(Debug, Clone, Default)]
pub struct DraftPlanSubtask {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct ModelError {
    pub message: String,
}

impl ModelError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The model lane. Every method is a request that may fail; none of them can
/// write a record, and the application treats a failure as a fallback rather
/// than an error on the gate path.
pub trait Model: Send + Sync {
    fn draft_gate(
        &self,
        target: &ModelTarget,
        context: &GateContext,
    ) -> Result<GateDraftText, ModelError>;

    fn critique_plan(
        &self,
        target: &ModelTarget,
        context: &PlanContext,
    ) -> Result<ModelAdvice, ModelError>;

    /// A conversational turn about the plan. Plain text, never JSON-shaped
    /// writes; the application stores the message and decides what to show.
    fn plan_chat(
        &self,
        _target: &ModelTarget,
        _context: &PlanChatContext,
    ) -> Result<String, ModelError> {
        Err(ModelError::new("this build has no plan chat"))
    }

    /// A structured proposal, reviewed by the user before it reaches storage.
    fn suggest_plan(
        &self,
        _target: &ModelTarget,
        _context: &PlanChatContext,
    ) -> Result<DraftPlan, ModelError> {
        Err(ModelError::new("this build has no plan suggestions"))
    }

    fn health(&self, target: &ModelTarget) -> ModelHealth;
}

/// A build with no model attached (tests, headless). Gates still draft, with
/// the app's own question.
pub struct NoModel;

impl Model for NoModel {
    fn draft_gate(
        &self,
        _target: &ModelTarget,
        _context: &GateContext,
    ) -> Result<GateDraftText, ModelError> {
        Err(ModelError::new("no model is attached to this build"))
    }

    fn critique_plan(
        &self,
        _target: &ModelTarget,
        _context: &PlanContext,
    ) -> Result<ModelAdvice, ModelError> {
        Err(ModelError::new("no model is attached to this build"))
    }

    fn health(&self, _target: &ModelTarget) -> ModelHealth {
        ModelHealth {
            configured: false,
            reachable: false,
            base_url: String::new(),
            model: String::new(),
            detail: "no model is attached to this build".to_owned(),
        }
    }
}

/// Typed storage for the no-AI flow.
///
/// Every mutating method runs one transaction that changes state, appends the
/// supplied events and bumps the revision. Validation that needs the current
/// record is repeated as a SQL guard, so a stale read cannot store an illegal
/// phase.
pub trait Store: Send + Sync {
    fn bootstrap(&self) -> Result<Bootstrap, AppError>;

    fn create_project(
        &self,
        project: NewProject,
        task: NewTask,
        events: &[NewEvent],
    ) -> Result<ProjectDetail, AppError>;

    /// Attaches or clears the repository. The path arrives canonicalised; the
    /// store only records it and appends the event.
    fn set_project_path(
        &self,
        project_id: &str,
        repo_path: Option<&str>,
        at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    /// Points the project at another of its own batches. The task keeps its
    /// phase, notes and gates; only `projects.current_task_id` moves.
    fn set_current_task(
        &self,
        project_id: &str,
        task_id: &str,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn project(&self, id: &str) -> Result<ProjectDetail, AppError>;

    fn update_brief(
        &self,
        project_id: &str,
        body: &str,
        expected_revision: Option<u32>,
        at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn update_note(
        &self,
        project_id: &str,
        note_id: &str,
        title: &str,
        text: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn add_note(
        &self,
        project_id: &str,
        note: NewNote,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn delete_note(
        &self,
        project_id: &str,
        note_id: &str,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn create_task(&self, task: NewTask, event: &NewEvent) -> Result<TaskDetail, AppError>;

    fn delete_task(&self, task_id: &str, event: &NewEvent) -> Result<ProjectDetail, AppError>;

    fn task(&self, id: &str) -> Result<TaskDetail, AppError>;

    fn write_prediction(
        &self,
        task_id: &str,
        text: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn approve_plan(
        &self,
        task_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn add_task_note(
        &self,
        task_id: &str,
        note: NewTaskNote,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    /// Inserts one or more plan items in one transaction. A proposal is one
    /// mutation, even when it creates a tree.
    fn add_plan_items(
        &self,
        task_id: &str,
        items: Vec<NewPlanItem>,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    /// The task a plan item belongs to, so edits can be logged against it.
    fn plan_item_task(&self, item_id: &str) -> Result<String, AppError>;

    fn update_plan_item(
        &self,
        item_id: &str,
        title: &str,
        body: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn set_plan_item_status(
        &self,
        item_id: &str,
        done: bool,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    /// Moves an item one place among its siblings. Sections and DoD items move
    /// at the root; a subtask moves inside its section.
    fn move_plan_item(
        &self,
        item_id: &str,
        direction: i32,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn delete_plan_item(
        &self,
        item_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn add_plan_message(
        &self,
        task_id: &str,
        message: NewPlanMessage,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn record_attempt(
        &self,
        task_id: &str,
        attempt: NewAttempt,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn close_implement(
        &self,
        task_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn write_handoff(
        &self,
        task_id: &str,
        handoff: NewHandoff,
        events: &[NewEvent],
    ) -> Result<TaskDetail, AppError>;

    /// The latest handoff regardless of the recall rule; callers other than
    /// `handoff_for` must not expose it.
    fn latest_handoff(&self, project_id: &str) -> Result<Option<HandoffDetail>, AppError>;

    fn start_session(
        &self,
        project_id: &str,
        session_id: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<SessionView, AppError>;

    fn submit_recall(
        &self,
        project_id: &str,
        session_id: &str,
        text: &str,
        matched_lines: u32,
        at: i64,
        event: &NewEvent,
    ) -> Result<RecallOutcome, AppError>;

    /// I2: the handoff is withheld until the open session has a recall.
    fn handoff_for(&self, project_id: &str) -> Result<HandoffDetail, AppError>;

    fn close_session(&self, project_id: &str, at: i64, event: &NewEvent) -> Result<(), AppError>;

    fn events(&self, project_id: Option<&str>, limit: u32) -> Result<Vec<EventSummary>, AppError>;

    // ---- Decisions and questions -----------------------------------------

    fn add_decision(
        &self,
        project_id: &str,
        decision: NewDecision,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn add_question(
        &self,
        project_id: &str,
        question: NewQuestion,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    fn answer_question(
        &self,
        question_id: &str,
        answer: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<ProjectDetail, AppError>;

    /// The project a question belongs to, so answering can log against it.
    fn question_project(&self, question_id: &str) -> Result<String, AppError>;

    // ---- Sensors ---------------------------------------------------------

    /// The stored checks a run will execute, in authoring order.
    fn check_targets(&self, task_id: &str) -> Result<Vec<CheckTarget>, AppError>;

    fn add_check(
        &self,
        task_id: &str,
        check: NewCheck,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn remove_check(&self, check_id: &str, event: &NewEvent) -> Result<TaskDetail, AppError>;

    /// The task a check belongs to, so removing it can log against the batch.
    fn check_task(&self, check_id: &str) -> Result<String, AppError>;

    /// Records the run before any process starts, so a crash leaves a visible
    /// `running` row rather than a silent absence.
    fn begin_check_run(&self, run: NewCheckRun, event: &NewEvent) -> Result<(), AppError>;

    /// Writes the results and the check states atomically. This is the only
    /// writer of `checks.result`.
    fn finish_check_run(
        &self,
        run_id: &str,
        results: &[NewCheckResult],
        at: i64,
        event: &NewEvent,
    ) -> Result<RunDetail, AppError>;

    /// A cancelled or failed run writes no results and leaves every check as it
    /// was (I3).
    fn abandon_check_run(
        &self,
        run_id: &str,
        cancelled: bool,
        note: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<RunDetail, AppError>;

    fn check_run(&self, run_id: &str) -> Result<RunDetail, AppError>;

    fn add_drill(&self, drill: NewDrill, event: &NewEvent) -> Result<DrillSummary, AppError>;

    fn drill(&self, drill_id: &str) -> Result<DrillRecord, AppError>;

    fn record_drill_run(
        &self,
        drill_id: &str,
        run: NewDrillRun,
        event: &NewEvent,
    ) -> Result<DrillOutcome, AppError>;

    // ---- Gates -----------------------------------------------------------

    fn gate(&self, gate_id: &str) -> Result<GateRecord, AppError>;

    fn gates_for_task(&self, task_id: &str) -> Result<Vec<GateDetail>, AppError>;

    /// Inserts one due gate. The caller has already checked the phase.
    fn draft_gate(
        &self,
        task_id: &str,
        gate: NewGate,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn answer_gate(&self, answer: GateAnswer<'_>, event: &NewEvent)
    -> Result<TaskDetail, AppError>;

    fn use_hint(&self, gate_id: &str, event: &NewEvent) -> Result<TaskDetail, AppError>;

    fn override_gate(
        &self,
        gate_id: &str,
        reason: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    // ---- Concepts --------------------------------------------------------

    fn concept(&self, concept_id: &str) -> Result<ConceptRecord, AppError>;

    fn concepts(&self, status: Option<ConceptStatus>) -> Result<Vec<ConceptSummary>, AppError>;

    fn create_concept(
        &self,
        concept: NewConcept,
        event: &NewEvent,
    ) -> Result<ConceptSummary, AppError>;

    fn attach_concept(
        &self,
        task_id: &str,
        concept_id: &str,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn detach_concept(
        &self,
        task_id: &str,
        concept_id: &str,
        event: &NewEvent,
    ) -> Result<TaskDetail, AppError>;

    fn review_concept(
        &self,
        review: NewConceptReview,
        event: &NewEvent,
    ) -> Result<ConceptSummary, AppError>;

    fn set_concept_help(
        &self,
        concept_id: &str,
        help: HelpLevel,
        event: &NewEvent,
    ) -> Result<ConceptSummary, AppError>;

    /// The first concept this task touches that is due or past its schedule.
    /// Gate drafting uses it so the check follows understanding, not the task.
    fn due_concept_for_task(
        &self,
        task_id: &str,
        cutoff: i64,
    ) -> Result<Option<ConceptSummary>, AppError>;

    // ---- Settings --------------------------------------------------------

    fn settings(&self) -> Result<Settings, AppError>;

    fn set_model(&self, model: ModelChoice, event: &NewEvent) -> Result<Settings, AppError>;

    /// The local endpoint and model name. One row each, so a failure to write
    /// the name cannot leave the URL half-applied.
    fn set_model_endpoint(
        &self,
        base_url: &str,
        name: &str,
        event: &NewEvent,
    ) -> Result<Settings, AppError>;

    fn add_allowlist_path(
        &self,
        path: &str,
        at: i64,
        event: &NewEvent,
    ) -> Result<Settings, AppError>;

    fn remove_allowlist_path(&self, path: &str, event: &NewEvent) -> Result<Settings, AppError>;

    /// The six review figures and their rows, computed from the event log.
    fn review(&self, days: u32, cutoff: i64) -> Result<Review, AppError>;

    /// Every table as JSON, for the export button.
    fn export_store(&self, at: i64) -> Result<String, AppError>;
}
