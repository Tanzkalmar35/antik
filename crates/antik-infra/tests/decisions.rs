//! Decisions and questions are project artifacts, not view state: they persist,
//! they log one event each, and a question is answered once.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use antik_app::{
    App,
    dto::{CreateDecisionInput, CreateProjectInput, CreateQuestionInput, EventKind},
    error::AppError,
    ports::{IdGen, NoFiles},
};
use antik_domain::{Clock, Timestamp};
use antik_infra::Db;

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000)
    }
}

struct SeqIds(AtomicU64);

impl IdGen for SeqIds {
    fn next(&self) -> String {
        format!("id-{:04}", self.0.fetch_add(1, Ordering::SeqCst))
    }
}

fn app() -> App<Db> {
    App::new(
        Arc::new(Db::open_in_memory().expect("open store")),
        Arc::new(FixedClock),
        Arc::new(SeqIds(AtomicU64::new(0))),
        Arc::new(NoFiles),
    )
}

fn project(app: &App<Db>) -> antik_app::dto::ProjectDetail {
    app.create_project(CreateProjectInput {
        name: "kestrel".to_owned(),
        brief: "A local event log.".to_owned(),
    })
    .expect("create project")
}

#[test]
fn a_decision_is_stored_with_its_reason_and_cost() {
    let app = app();
    let project = project(&app);
    let task_id = project.tasks[0].id.clone();

    let detail = app
        .add_decision(
            &project.id,
            CreateDecisionInput {
                title: "Cap the outbox at five attempts".to_owned(),
                why: "A sixth wait outlives the request that started it.".to_owned(),
                cost: "A transient outage now fails loudly.".to_owned(),
                task_id: Some(task_id.clone()),
            },
        )
        .expect("add decision");

    assert_eq!(detail.decisions.len(), 1);
    let decision = &detail.decisions[0];
    assert_eq!(decision.project_id, project.id);
    assert_eq!(decision.task_id.as_deref(), Some(task_id.as_str()));
    assert!(decision.cost.contains("fails loudly"));

    // The decision survives a re-read, so the screen is showing storage.
    let again = app.get_project(&project.id).expect("re-read project");
    assert_eq!(again.decisions.len(), 1);

    let events = app.list_events(Some(&project.id), 50).expect("events");
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::DecisionRecorded)
    );
}

#[test]
fn a_decision_needs_a_cost_and_a_real_task() {
    let app = app();
    let project = project(&app);

    let thin = app.add_decision(
        &project.id,
        CreateDecisionInput {
            title: "Use SQLite".to_owned(),
            why: "One file.".to_owned(),
            cost: "   ".to_owned(),
            task_id: None,
        },
    );
    assert!(
        matches!(thin, Err(AppError::Invalid(_))),
        "a cost is required"
    );

    let unknown_task = app.add_decision(
        &project.id,
        CreateDecisionInput {
            title: "Use SQLite".to_owned(),
            why: "One file beats a directory walk.".to_owned(),
            cost: "The log is opaque without the app.".to_owned(),
            task_id: Some("no-such-task".to_owned()),
        },
    );
    assert!(matches!(unknown_task, Err(AppError::NotFound(_))));
    assert!(
        app.get_project(&project.id)
            .expect("project")
            .decisions
            .is_empty()
    );
}

#[test]
fn a_question_is_answered_once_and_stays_in_history() {
    let app = app();
    let project = project(&app);

    let detail = app
        .add_question(
            &project.id,
            CreateQuestionInput {
                text: "Is the cap a request budget or a retry counter?".to_owned(),
            },
        )
        .expect("add question");
    assert_eq!(detail.questions.len(), 1);
    let question = detail.questions[0].clone();
    assert!(question.answer.is_none());

    let answered = app
        .answer_question(
            &question.id,
            "A request budget: six waits outlive the caller.",
        )
        .expect("answer question");
    assert_eq!(answered.questions.len(), 1, "answering is not deleting");
    assert!(answered.questions[0].answer.is_some());
    assert!(answered.questions[0].answered_at.is_some());

    let twice = app.answer_question(&question.id, "A retry counter.");
    assert!(matches!(twice, Err(AppError::Conflict(_))));

    let events = app.list_events(Some(&project.id), 50).expect("events");
    let kinds: Vec<_> = events.iter().map(|event| event.kind).collect();
    assert!(kinds.contains(&EventKind::QuestionRecorded));
    assert!(kinds.contains(&EventKind::QuestionAnswered));
}
