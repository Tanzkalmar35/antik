//! M7: the plan is a structured document projected to markdown.
//!
//! The tests cover the pieces the UI relies on: section/subtask/DoD edits,
//! ordering, the approval guard, stored discussion, reviewed model proposals,
//! and the one-file projection.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use antik_app::{
    App,
    dto::{
        CreateProjectInput, EventKind, PlanDraftInput, PlanItemInput, PlanItemKind, PlanItemStatus,
        PlanMessageRole, PlanSource,
    },
    error::AppError,
    ports::{
        DraftPlan, DraftPlanSection, DraftPlanSubtask, GateContext, GateDraftText, IdGen, Model,
        ModelAdvice, ModelError, ModelTarget, PlanChatContext, PlanContext,
    },
};
use antik_domain::{Clock, Timestamp};
use antik_infra::{Db, MarkdownFiles};

struct StepClock(AtomicI64);

impl Clock for StepClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000 + self.0.fetch_add(1, Ordering::SeqCst))
    }
}

struct SeqIds(AtomicU64);

impl IdGen for SeqIds {
    fn next(&self) -> String {
        format!("id-{:04}", self.0.fetch_add(1, Ordering::SeqCst))
    }
}

struct Planner {
    fail: bool,
}

impl Model for Planner {
    fn draft_gate(
        &self,
        _target: &ModelTarget,
        _context: &GateContext,
    ) -> Result<GateDraftText, ModelError> {
        Ok(GateDraftText {
            question: "What does this protect?".to_owned(),
            hint: "Start at the failure.".to_owned(),
        })
    }

    fn critique_plan(
        &self,
        _target: &ModelTarget,
        _context: &PlanContext,
    ) -> Result<ModelAdvice, ModelError> {
        Ok(ModelAdvice {
            points: vec!["Name the failure first.".to_owned()],
        })
    }

    fn plan_chat(
        &self,
        _target: &ModelTarget,
        context: &PlanChatContext,
    ) -> Result<String, ModelError> {
        if self.fail {
            return Err(ModelError::new("the local model is not running"));
        }
        assert_eq!(
            context.messages.last().expect("the user's turn").role,
            PlanMessageRole::User
        );
        Ok(
            "Split the decision from the migration, and make the retry criterion observable."
                .to_owned(),
        )
    }

    fn suggest_plan(
        &self,
        _target: &ModelTarget,
        _context: &PlanChatContext,
    ) -> Result<DraftPlan, ModelError> {
        if self.fail {
            return Err(ModelError::new("the local model is not running"));
        }
        Ok(DraftPlan {
            sections: vec![DraftPlanSection {
                title: "Persist the key".to_owned(),
                subtasks: vec![DraftPlanSubtask {
                    title: "Add the column and migration".to_owned(),
                    body: "Keep the old rows readable.".to_owned(),
                }],
            }],
            dods: vec!["A retry writes one outbox row".to_owned()],
        })
    }

    fn health(&self, _target: &ModelTarget) -> antik_app::dto::ModelHealth {
        antik_app::dto::ModelHealth {
            configured: true,
            reachable: !self.fail,
            base_url: "http://127.0.0.1:11434".to_owned(),
            model: "fake".to_owned(),
            detail: "fake".to_owned(),
        }
    }
}

fn fixture(fail: bool) -> (App<Db>, tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("projects");
    std::fs::create_dir_all(&root).expect("projects dir");
    let app = App::with_model(
        Arc::new(Db::open_in_memory().expect("open store")),
        Arc::new(StepClock(AtomicI64::new(0))),
        Arc::new(SeqIds(AtomicU64::new(0))),
        Arc::new(MarkdownFiles::new(&root)),
        Arc::new(antik_app::ports::NoRunner),
        Arc::new(Planner { fail }),
    );
    let project = app
        .create_project(CreateProjectInput {
            name: "kestrel".to_owned(),
            brief: "A local event log.".to_owned(),
        })
        .expect("create project");
    let task_id = project.tasks[0].id.clone();
    (app, dir, project.id, task_id)
}

fn section(app: &App<Db>, task_id: &str, title: &str) -> String {
    app.add_plan_item(
        task_id,
        PlanItemInput {
            kind: PlanItemKind::Section,
            parent_id: None,
            title: title.to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .expect("section")
    .plan_items
    .into_iter()
    .find(|item| item.title == title)
    .expect("the section row")
    .id
}

fn subtask(app: &App<Db>, task_id: &str, parent_id: &str, title: &str) -> String {
    app.add_plan_item(
        task_id,
        PlanItemInput {
            kind: PlanItemKind::Subtask,
            parent_id: Some(parent_id.to_owned()),
            title: title.to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .expect("subtask")
    .plan_items
    .into_iter()
    .find(|item| item.title == title)
    .expect("the subtask row")
    .id
}

fn dod(app: &App<Db>, task_id: &str, title: &str) -> String {
    app.add_plan_item(
        task_id,
        PlanItemInput {
            kind: PlanItemKind::Dod,
            parent_id: None,
            title: title.to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .expect("definition of done")
    .plan_items
    .into_iter()
    .find(|item| item.kind == PlanItemKind::Dod && item.title == title)
    .expect("the DoD row")
    .id
}

#[test]
fn a_plan_is_edited_ordered_approved_and_projected() {
    let (app, dir, project_id, task_id) = fixture(false);

    // The approach is the first requirement, and a definition of done is the
    // second.
    assert!(matches!(
        app.approve_plan(&task_id).unwrap_err(),
        AppError::Invalid(_)
    ));
    app.write_prediction(
        &task_id,
        "The delivery worker retries on 503; the key should come from the request.",
    )
    .expect("approach");
    assert!(matches!(
        app.approve_plan(&task_id).unwrap_err(),
        AppError::Invalid(_)
    ));

    let decide = section(&app, &task_id, "Decide where the key is produced");
    let trace = subtask(&app, &task_id, &decide, "Trace every enqueue path");
    let choose = subtask(&app, &task_id, &decide, "Choose the key source");
    app.update_plan_item(
        &trace,
        "Trace every enqueue path",
        "Start at the caller that already has a request id.",
    )
    .expect("edit subtask");
    app.set_plan_item_status(&trace, true)
        .expect("complete subtask");
    let events = app.list_events(Some(&project_id), 100).expect("events");
    assert!(
        events.iter().any(|event| {
            event.kind == EventKind::PlanItemCompleted && event.task_id.as_deref() == Some(&task_id)
        }),
        "plan edits belong to the project's log"
    );
    let task = app.move_plan_item(&choose, -1).expect("move subtask up");
    let children: Vec<_> = task
        .plan_items
        .iter()
        .filter(|item| item.parent_id.as_deref() == Some(&decide))
        .collect();
    assert_eq!(children[0].title, "Choose the key source");

    let persist = section(&app, &task_id, "Persist the key");
    let task = app.move_plan_item(&persist, -1).expect("move section up");
    let sections: Vec<_> = task
        .plan_items
        .iter()
        .filter(|item| item.kind == PlanItemKind::Section)
        .collect();
    assert_eq!(sections[0].title, "Persist the key");

    dod(&app, &task_id, "A duplicate delivery writes one row");
    let task = app.approve_plan(&task_id).expect("approve");
    assert!(task.plan_approved);
    assert!(!task.plan_revised);

    // Approval is not a lock. A later edit is still allowed, but it is visible.
    let later = section(&app, &task_id, "After approval");
    let task = app.delete_plan_item(&later).expect("remove later section");
    assert!(task.plan_items.iter().all(|item| item.id != later));
    assert!(
        task.plan_revised,
        "an edit after approval is visible as a revision"
    );

    let file = dir
        .path()
        .join("projects")
        .join(&project_id)
        .join("tasks")
        .join(format!("{task_id}.md"));
    let text = std::fs::read_to_string(&file).expect("projected task document");
    assert!(text.contains("## Description"));
    assert!(text.contains("The delivery worker retries on 503"));
    assert!(text.contains("## Plan"));
    assert!(text.contains("### 1. Persist the key"));
    assert!(text.contains("### 2. Decide where the key is produced"));
    assert!(text.contains("## Definition of done"));
    assert!(text.contains("- [ ] A duplicate delivery writes one row — manual"));
    assert!(text.contains("## Discussion"));
    assert!(text.contains("_No discussion yet._"));
    assert!(text.contains("- [ ] Choose the key source"));
    assert!(text.contains("- [x] Trace every enqueue path"));
}

#[test]
fn chat_is_stored_and_a_proposal_waits_for_acceptance() {
    let (app, dir, project_id, task_id) = fixture(false);
    app.write_prediction(
        &task_id,
        "The delivery worker retries on 503; the key should come from the request.",
    )
    .expect("approach");

    let reply = app
        .send_plan_message(&task_id, "What am I missing in the retry path?")
        .expect("chat");
    assert!(reply.note.is_none());
    assert_eq!(reply.task.plan_messages.len(), 2);
    assert_eq!(reply.task.plan_messages[0].role, PlanMessageRole::User);
    assert_eq!(reply.task.plan_messages[1].role, PlanMessageRole::Assistant);
    assert!(reply.task.plan_messages[1].content.contains("Split"));

    // The proposal is display data until the user accepts it.
    let proposal = app.suggest_plan(&task_id).expect("proposal");
    assert_eq!(proposal.model, "qwen3.5:9b");
    assert_eq!(proposal.sections.len(), 1);
    let before = app.get_task(&task_id).expect("task");
    assert!(before.plan_items.is_empty());

    let task = app
        .accept_plan_proposal(
            &task_id,
            PlanDraftInput {
                sections: proposal.sections,
                dods: proposal.dods,
            },
        )
        .expect("accept proposal");
    assert!(
        task.plan_items
            .iter()
            .any(|item| item.kind == PlanItemKind::Section && item.source == PlanSource::Model)
    );
    assert!(task.plan_items.iter().any(|item| {
        item.kind == PlanItemKind::Dod
            && item.title == "A retry writes one outbox row"
            && item.status == PlanItemStatus::Open
    }));
    assert!(
        task.plan_items
            .iter()
            .any(|item| { item.kind == PlanItemKind::Subtask && item.parent_id.is_some() })
    );

    let file = dir
        .path()
        .join("projects")
        .join(&project_id)
        .join("tasks")
        .join(format!("{task_id}.md"));
    let text = std::fs::read_to_string(&file).expect("projection");
    assert!(text.contains("## Discussion"));
    assert!(text.contains("What am I missing in the retry path?"));
    assert!(text.contains("Split the decision from the migration"));
    assert!(text.contains("### 1. Persist the key"));
    assert!(text.contains("- [ ] A retry writes one outbox row — manual"));
}

#[test]
fn a_failed_model_keeps_the_user_turn() {
    let (app, _dir, _project_id, task_id) = fixture(true);
    app.write_prediction(&task_id, "Use the request id as the key.")
        .expect("approach");

    let reply = app
        .send_plan_message(&task_id, "Should the key be in the request?")
        .expect("the turn is stored even when the model is down");
    assert!(reply.note.is_some());
    assert_eq!(reply.task.plan_messages.len(), 1);
    assert_eq!(reply.task.plan_messages[0].role, PlanMessageRole::User);

    assert!(matches!(
        app.suggest_plan(&task_id).unwrap_err(),
        AppError::ModelUnavailable(_)
    ));
}
