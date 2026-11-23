//! Wire types. These are the only shapes that cross IPC.
//!
//! Identifiers are ULID strings and timestamps are RFC 3339 strings. Keeping
//! timestamps as strings on the wire avoids large-integer precision debates in
//! TypeScript and matches what the frontend already parses.

use serde::{Deserialize, Serialize};

macro_rules! wire_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
        )]
        pub enum $name {
            $(#[serde(rename = $wire)] $variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(format!("unknown {}: {other}", stringify!($name))),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

wire_enum!(Phase {
    Plan => "plan",
    Implement => "implement",
    Verify => "verify",
    Reflect => "reflect",
});

wire_enum!(Lane {
    Learn => "learn",
    Verify => "verify",
});

wire_enum!(ConceptStatus {
    Passed => "passed",
    Weak => "weak",
    Due => "due",
});

wire_enum!(HelpLevel {
    Full => "full",
    Hint => "hint",
    None => "none",
});

wire_enum!(ModelChoice {
    Local => "local",
    Api => "api",
    None => "none",
});

wire_enum!(GateStatus {
    NotYet => "not-yet",
    Due => "due",
    Passed => "passed",
    Failed => "failed",
    Overridden => "overridden",
});

wire_enum!(GateSource {
    App => "app",
    Model => "model",
});

wire_enum!(CheckResult {
    Pass => "pass",
    Fail => "fail",
    Pending => "pending",
    Done => "done",
});

wire_enum!(PlanItemKind {
    Section => "section",
    Subtask => "subtask",
    Dod => "dod",
});

wire_enum!(PlanItemStatus {
    Open => "open",
    Done => "done",
});

wire_enum!(PlanSource {
    User => "user",
    Model => "model",
});

wire_enum!(PlanMessageRole {
    User => "user",
    Assistant => "assistant",
});

// The runner's own outcome, separate from a check's verdict: a run can finish
// with failing checks (the runner did its job) or fail to start at all.
wire_enum!(RunStatus {
    Running => "running",
    Done => "done",
    Failed => "failed",
    Cancelled => "cancelled",
});

wire_enum!(EventKind {
    ProjectCreated => "project_created",
    BriefEdited => "brief_edited",
    NoteAdded => "note_added",
    NoteDeleted => "note_deleted",
    TaskPlanned => "task_planned",
    TaskSwitched => "task_switched",
    TaskDeleted => "task_deleted",
    PredictionWritten => "prediction_written",
    PlanApproved => "plan_approved",
    PlanItemAdded => "plan_item_added",
    PlanItemUpdated => "plan_item_updated",
    PlanItemMoved => "plan_item_moved",
    PlanItemCompleted => "plan_item_completed",
    PlanItemRemoved => "plan_item_removed",
    PlanMessageAdded => "plan_message_added",
    AttemptRecorded => "attempt_recorded",
    ImplementClosed => "implement_closed",
    GateDrafted => "gate_drafted",
    GateAttempted => "gate_attempted",
    GateOverridden => "gate_overridden",
    HintUsed => "hint_used",
    CheckAdded => "check_added",
    CheckRemoved => "check_removed",
    CheckRun => "check_run",
    CheckResult => "check_result",
    HandoffWritten => "handoff_written",
    RecallStored => "recall_stored",
    SessionOpened => "session_opened",
    SessionClosed => "session_closed",
    DecisionRecorded => "decision_recorded",
    QuestionRecorded => "question_recorded",
    QuestionAnswered => "question_answered",
    ConceptAdded => "concept_added",
    ConceptAttached => "concept_attached",
    ConceptDetached => "concept_detached",
    ConceptReviewed => "concept_reviewed",
    HelpChanged => "help_changed",
    DrillAdded => "drill_added",
    DrillRun => "drill_run",
    SettingChanged => "setting_changed",
    ReadScopeChanged => "read_scope_changed",
});

/// Everything the shell needs at launch, in one round trip.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub projects: Vec<ProjectSummary>,
    pub concepts: Vec<ConceptSummary>,
    pub drills: Vec<DrillSummary>,
    pub settings: Settings,
    pub session: Session,
    pub events: Vec<EventSummary>,
    pub revision: u32,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub brief: String,
    pub current_task: Option<CurrentTaskSummary>,
    pub handoff: Option<HandoffSummary>,
    pub last_touched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTaskSummary {
    pub id: String,
    pub title: String,
    pub phase: Phase,
    pub lane: Lane,
    pub pending_gate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HandoffSummary {
    pub at: String,
    pub recalled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConceptSummary {
    pub id: String,
    pub name: String,
    pub area: String,
    pub definition: String,
    pub status: ConceptStatus,
    pub help: HelpLevel,
    pub seen: u32,
    pub due_on: Option<String>,
    pub last_reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DrillSummary {
    pub id: String,
    pub skill: String,
    pub name: String,
    pub command: String,
    pub attempts: u32,
    pub last_run_at: Option<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model: ModelChoice,
    pub allowlist: Vec<String>,
    pub write_access: bool,
    /// Where the local model listens. Loopback only, validated on write.
    pub model_url: String,
    /// The Ollama model name, e.g. `qwen3.5:9b`.
    pub model_name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: ModelChoice::Local,
            allowlist: Vec::new(),
            write_access: false,
            model_url: "http://127.0.0.1:11434".to_owned(),
            model_name: "qwen3.5:9b".to_owned(),
        }
    }
}

/// What the Settings screen shows about the model. `configured` says the choice
/// is on; `reachable` says the endpoint answered. No credential is involved, so
/// there is nothing here to keep out of the payload.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelHealth {
    pub configured: bool,
    pub reachable: bool,
    pub base_url: String,
    pub model: String,
    pub detail: String,
}

/// Model advice on a plan. Display copy only: it is never stored and it cannot
/// approve, score or change anything.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Critique {
    pub model: String,
    pub points: Vec<String>,
    /// Project-relative paths that were in the prompt. Empty when nothing was
    /// allowlisted or the batch's scope matched nothing.
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub started_at: Option<String>,
    pub project_id: Option<String>,
    /// Whether a recall exists; the text itself is not part of the launch payload.
    pub has_recall: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EventSummary {
    pub id: String,
    pub at: String,
    pub kind: EventKind,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub days: u32,
    pub from: String,
    /// Tasks with a plan or handoff event in the window.
    pub tasks: u32,
    pub lead_time_days: Option<f64>,
    pub rework: u32,
    pub gate_pass_rate: Option<u32>,
    pub resolved_gates: u32,
    pub passed_gates: u32,
    pub bypasses: u32,
    pub failing_checks: u32,
    pub basis: ReviewBasis,
    pub rows: Vec<ReviewRow>,
}

/// Every figure states what it was computed from. Empty windows say so.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBasis {
    pub tasks: String,
    pub lead_time: String,
    pub rework: String,
    pub gate_pass_rate: String,
    pub bypasses: String,
    pub failing_checks: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRow {
    pub task_id: String,
    pub project_id: String,
    pub project: String,
    pub title: String,
    pub phase: Phase,
    pub lead_days: Option<f64>,
    pub attempts: u32,
    pub gate: Option<ReviewGate>,
    pub checks_passed: u32,
    pub checks_total: u32,
    pub closed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewGate {
    pub n: u32,
    pub status: GateStatus,
}

impl Bootstrap {
    /// The state of a fresh installation: no projects, default settings.
    pub fn empty(revision: u32, schema_version: u32) -> Self {
        Self {
            projects: Vec::new(),
            concepts: Vec::new(),
            drills: Vec::new(),
            settings: Settings::default(),
            session: Session::default(),
            events: Vec::new(),
            revision,
            schema_version,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub id: String,
    pub name: String,
    pub brief: String,
    /// The repository this project is attached to, canonicalised on write.
    /// `None` means the project lives entirely in app data.
    pub repo_path: Option<String>,
    pub created: String,
    pub current_task_id: Option<String>,
    pub tasks: Vec<TaskDetail>,
    pub notes: Vec<ProjectNote>,
    /// Append-only. A decision without its cost is a preference.
    pub decisions: Vec<DecisionSummary>,
    pub questions: Vec<QuestionSummary>,
    pub handoff: Option<HandoffDetail>,
    /// True until the current session's recall is stored (I2).
    pub handoff_locked: bool,
    pub recall: Option<String>,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DecisionSummary {
    pub id: String,
    pub project_id: String,
    /// Set when the decision was made inside a batch.
    pub task_id: Option<String>,
    pub at: String,
    pub title: String,
    pub why: String,
    pub cost: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuestionSummary {
    pub id: String,
    pub at: String,
    pub text: String,
    /// Answering keeps the question in history; it is never deleted.
    pub answer: Option<String>,
    pub answered_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateDecisionInput {
    pub title: String,
    pub why: String,
    /// What the decision makes harder. Required: an unnamed cost is a guess.
    pub cost: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateQuestionInput {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub lane: Lane,
    pub phase: Phase,
    pub started: String,
    pub finished: Option<String>,
    pub minutes: u32,
    pub batch: String,
    pub scope: String,
    pub prediction: String,
    pub plan_approved: bool,
    /// True when a plan item changed after the plan was approved.
    pub plan_revised: bool,
    pub plan_items: Vec<PlanItem>,
    pub plan_messages: Vec<PlanMessage>,
    pub notes: Vec<TaskNote>,
    pub attempts: Vec<Attempt>,
    pub checks: Vec<CheckSummary>,
    pub gates: Vec<GateDetail>,
    pub concept_ids: Vec<String>,
    pub reflected: bool,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub id: String,
    /// Set for a subtask: the section it belongs to.
    pub parent_id: Option<String>,
    pub kind: PlanItemKind,
    pub title: String,
    pub body: String,
    pub position: i32,
    pub status: PlanItemStatus,
    pub source: PlanSource,
    /// Set when a definition-of-done item is backed by an executable check.
    pub check_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanMessage {
    pub id: String,
    pub at: String,
    pub role: PlanMessageRole,
    pub content: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanItemInput {
    pub kind: PlanItemKind,
    pub parent_id: Option<String>,
    pub title: String,
    pub body: String,
    /// An existing check on the same task, for a definition-of-done item.
    pub check_id: Option<String>,
}

/// A model-shaped plan. The app validates it and the user accepts it; a
/// proposal never writes rows by itself.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanProposal {
    pub model: String,
    pub sections: Vec<PlanProposalSection>,
    pub dods: Vec<PlanProposalItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanProposalSection {
    pub title: String,
    pub subtasks: Vec<PlanProposalItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanProposalItem {
    pub title: String,
    pub body: String,
}

/// What the review screen sends back after the user accepts a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanDraftInput {
    pub sections: Vec<PlanProposalSection>,
    pub dods: Vec<PlanProposalItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanChatReply {
    pub task: TaskDetail,
    /// A model failure is a note, not a lost user message.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskNote {
    pub id: String,
    pub at: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    pub id: String,
    pub at: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckSummary {
    pub id: String,
    pub label: String,
    pub detail: String,
    /// The command line the runner will execute, shown so authoring is honest.
    pub command: String,
    pub result: CheckResult,
    pub evaluated_at: Option<String>,
}

/// One process in a run, as the UI sees it.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckOutcome {
    pub check_id: String,
    pub label: String,
    pub result: CheckResult,
    pub exit_code: Option<i32>,
    pub duration_ms: u32,
    pub stdout_tail: String,
    pub stderr_tail: String,
    /// True when the output was longer than the tail kept.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub id: String,
    pub task_id: String,
    pub status: RunStatus,
    pub started: String,
    pub finished: Option<String>,
    /// Empty while running, and after a cancelled or failed run.
    pub results: Vec<CheckOutcome>,
}

/// Authoring input: the program is one of the allowlist, the arguments are
/// whitespace-separated and never see a shell.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CheckInput {
    pub label: String,
    pub detail: String,
    pub program: String,
    pub args: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateDrillInput {
    pub skill: String,
    pub name: String,
    /// One program from the allowlist plus its arguments, on one line.
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DrillOutcome {
    pub drill: DrillSummary,
    pub passed: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u32,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GateDetail {
    pub id: String,
    /// The concept this gate checks, when it was drafted from one.
    pub concept_id: Option<String>,
    /// Whether the question came from the app or from the local model. The
    /// rubric below it is always the app's.
    pub drafted_by: GateSource,
    pub n: u32,
    pub question: String,
    pub hint: String,
    pub status: GateStatus,
    pub attempts: u32,
    pub hint_used: bool,
    pub answer: Option<String>,
    pub reason: Option<String>,
    pub evaluated_at: Option<String>,
    pub attempt_log: Vec<GateAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GateAttempt {
    pub at: String,
    pub answer: String,
    pub pass: bool,
    /// What the rubric found, in one line. Written by the app, never the model.
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectNote {
    pub id: String,
    pub at: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HandoffDetail {
    pub at: String,
    pub next: String,
    pub blocking: String,
    pub thinking: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub project_id: String,
    pub started_at: String,
    pub has_recall: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecallOutcome {
    pub matched_lines: u32,
    pub handoff: Option<HandoffDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub brief: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub title: String,
    pub scope: String,
    pub prediction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HandoffInput {
    pub next: String,
    pub blocking: String,
    pub thinking: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateConceptInput {
    /// Human-authored and stable, for example `K-053`.
    pub id: String,
    pub name: String,
    pub area: String,
    pub definition: String,
}

/// A concept review carries its own note so the screen can say why an answer
/// stayed due without re-reading the attempt table.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConceptReviewOutcome {
    pub concept: ConceptSummary,
    pub passed: bool,
    pub note: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check<T: serde::Serialize + std::fmt::Display>(value: T, wire: &str) {
        assert_eq!(value.to_string(), wire);
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            serde_json::json!(wire)
        );
    }

    #[test]
    fn wire_names_agree_with_serde() {
        check(Phase::Plan, "plan");
        check(Phase::Implement, "implement");
        check(Phase::Verify, "verify");
        check(Phase::Reflect, "reflect");
        check(Lane::Learn, "learn");
        check(Lane::Verify, "verify");
        check(ConceptStatus::Passed, "passed");
        check(HelpLevel::None, "none");
        check(ModelChoice::None, "none");
        check(GateStatus::NotYet, "not-yet");
        check(EventKind::GateAttempted, "gate_attempted");
        check(EventKind::ConceptAttached, "concept_attached");
        check(EventKind::ReadScopeChanged, "read_scope_changed");
    }

    #[test]
    fn every_phase_round_trips() {
        for phase in [Phase::Plan, Phase::Implement, Phase::Verify, Phase::Reflect] {
            assert_eq!(phase.to_string().parse::<Phase>().unwrap(), phase);
        }
    }

    #[test]
    fn unknown_wire_value_is_an_error() {
        assert!("gone".parse::<Phase>().is_err());
    }
}
