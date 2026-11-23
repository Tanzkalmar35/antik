//! M3 exit checks: gates are drafted by the app, scored deterministically and
//! resolved only through an attempt or a reasoned override. Concepts are
//! authored, attached and spaced without a model in the loop.

use std::sync::Arc;

use antik_app::{
    App,
    dto::{
        ConceptStatus, CreateConceptInput, CreateProjectInput, CreateTaskInput, EventKind,
        GateStatus, HandoffInput, HelpLevel, ModelChoice, Phase, PlanItemInput, PlanItemKind,
    },
    error::AppError,
    ports::{IdGen, NoFiles},
};
use antik_domain::{Clock, Timestamp};
use antik_infra::Db;

const NOW: i64 = 1_700_000_000_000;
const DAY: i64 = 86_400_000;

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(NOW)
    }
}

struct SystemIds;

impl IdGen for SystemIds {
    fn next(&self) -> String {
        ulid::Ulid::generate().to_string()
    }
}

fn app() -> App<Db> {
    App::new(
        Arc::new(Db::open_in_memory().expect("open store")),
        Arc::new(FixedClock),
        Arc::new(SystemIds),
        Arc::new(NoFiles),
    )
}

fn create(app: &App<Db>, name: &str) -> antik_app::dto::ProjectDetail {
    app.create_project(CreateProjectInput {
        name: name.to_owned(),
        brief: "A local event log for one developer's tools.".to_owned(),
    })
    .expect("create project")
}

/// Moves a task to verify, ready for a gate.
fn ready_task(app: &App<Db>, project_id: &str, title: &str) -> String {
    let task = app
        .create_task(
            project_id,
            CreateTaskInput {
                title: title.to_owned(),
                scope: "src/outbox/policy.ts".to_owned(),
                prediction: String::new(),
            },
        )
        .expect("create task");
    app.write_prediction(&task.id, "Count the cap before the delay.")
        .expect("prediction");
    app.add_plan_item(
        &task.id,
        PlanItemInput {
            kind: PlanItemKind::Dod,
            parent_id: None,
            title: "The cap is counted before the delay".to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .expect("definition of done");
    app.approve_plan(&task.id).expect("approve");
    app.close_implement(&task.id).expect("close implement");
    task.id
}

fn millis(iso: &str) -> i64 {
    let at = time::OffsetDateTime::parse(iso, &time::format_description::well_known::Rfc3339)
        .expect("RFC 3339 timestamp");
    (at.unix_timestamp_nanos() / 1_000_000) as i64
}

const PASSING_ANSWER: &str =
    "The retry policy caps attempts in the outbox so the downstream does not fail under load.";

#[test]
fn a_gate_is_drafted_and_scored_by_the_app() {
    let app = app();
    let project = create(&app, "kestrel");
    let task_id = ready_task(&app, &project.id, "Retry policy");

    // Only verify can draft a gate, and only one open gate exists at a time.
    let task = app.draft_gate(&task_id).expect("draft");
    assert_eq!(task.gates.len(), 1);
    let gate = task.gates[0].clone();
    assert_eq!(gate.status, GateStatus::Due);
    assert!(gate.question.contains("Retry policy"));
    assert!(gate.attempt_log.is_empty());
    assert!(app.draft_gate(&task_id).is_err(), "one open gate at a time");
    assert_eq!(app.list_gates(&task_id).unwrap().len(), 1);

    // A thin answer fails and stays open, with the app's note attached.
    let task = app.answer_gate(&gate.id, "Too short.").expect("attempt");
    let gate = task.gates[0].clone();
    assert_eq!(gate.status, GateStatus::Failed);
    assert_eq!(gate.attempts, 1);
    assert_eq!(gate.attempt_log.len(), 1);
    assert!(!gate.attempt_log[0].pass);
    assert!(gate.attempt_log[0].note.starts_with("Not met."));

    let task = app
        .answer_gate(&gate.id, PASSING_ANSWER)
        .expect("second attempt");
    let gate = task.gates[0].clone();
    assert_eq!(gate.status, GateStatus::Passed);
    assert_eq!(gate.attempts, 2);
    assert!(gate.attempt_log[0].pass);
    assert!(gate.attempt_log[0].note.starts_with("Met."));

    // A resolved gate refuses further answers.
    let refused = app.answer_gate(&gate.id, PASSING_ANSWER).unwrap_err();
    assert!(matches!(refused, AppError::Invalid(_)), "{refused:?}");

    let task = app
        .write_handoff(
            &task_id,
            HandoffInput {
                next: "Write the outbox flush.".to_owned(),
                blocking: "Nothing yet.".to_owned(),
                thinking: "The cap belongs to the request.".to_owned(),
            },
        )
        .expect("handoff");
    assert_eq!(task.phase, Phase::Reflect);

    // The review reads the pass from the event payload, not from the gate row.
    let review = app.review(30).expect("review");
    assert_eq!(review.resolved_gates, 2);
    assert_eq!(review.passed_gates, 1);
    assert_eq!(review.gate_pass_rate, Some(50));
    assert_eq!(review.bypasses, 0);

    let export = app.export_store().expect("export");
    let snapshot: serde_json::Value = serde_json::from_str(&export).expect("JSON");
    let events = snapshot["tables"]["events"].as_array().expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event["kind"] == "gate_attempted")
            .count(),
        2
    );
    assert!(events.iter().any(|event| {
        event["kind"] == "gate_attempted" && event["payload"]["pass"] == serde_json::json!(true)
    }));
}

#[test]
fn a_handoff_waits_for_the_override_reason() {
    let app = app();
    let project = create(&app, "plumb");
    let task_id = ready_task(&app, &project.id, "Parser positions");

    let task = app.draft_gate(&task_id).expect("draft");
    let gate = task.gates[0].clone();

    let refused = app.override_gate(&gate.id, "no").unwrap_err();
    assert!(matches!(refused, AppError::Invalid(_)), "{refused:?}");

    let task = app
        .override_gate(&gate.id, "I already know this one and the gate is noise.")
        .expect("override");
    let gate = &task.gates[0];
    assert_eq!(gate.status, GateStatus::Overridden);
    assert!(gate.reason.as_deref().unwrap().contains("noise"));

    let refused = app.answer_gate(&gate.id, PASSING_ANSWER).unwrap_err();
    assert!(matches!(refused, AppError::Invalid(_)), "{refused:?}");

    let task = app
        .write_handoff(
            &task_id,
            HandoffInput {
                next: "Next batch.".to_owned(),
                blocking: "Nothing.".to_owned(),
                thinking: "The position is the byte that broke the rule.".to_owned(),
            },
        )
        .expect("handoff");
    assert_eq!(task.phase, Phase::Reflect);

    let review = app.review(30).expect("review");
    assert_eq!(review.bypasses, 1);
    assert_eq!(review.gate_pass_rate, None, "an override is not an attempt");
}

#[test]
fn a_hint_is_recorded_once() {
    let app = app();
    let project = create(&app, "foundry");
    let task_id = ready_task(&app, &project.id, "Property generators");
    let task = app.draft_gate(&task_id).expect("draft");
    let gate = task.gates[0].clone();
    assert!(!gate.hint_used);

    let task = app.use_hint(&gate.id).expect("hint");
    assert!(task.gates[0].hint_used);
    let task = app.use_hint(&gate.id).expect("hint again");
    assert!(task.gates[0].hint_used);

    let events = app.list_events(None, 100).expect("events");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == antik_app::dto::EventKind::HintUsed)
            .count(),
        1,
        "a repeated hint does not write a second event"
    );
}

#[test]
fn concepts_are_authored_attached_and_spaced() {
    let app = app();
    let project = create(&app, "saltmarsh");
    let task_id = project.tasks[0].id.clone();

    let concept = app
        .create_concept(CreateConceptInput {
            id: "K-900".to_owned(),
            name: "Idempotency".to_owned(),
            area: "distributed".to_owned(),
            definition: "The same request applied twice leaves the same state.".to_owned(),
        })
        .expect("create concept");
    assert_eq!(concept.status, ConceptStatus::Due);
    assert_eq!(concept.help, HelpLevel::Full);

    let duplicate = app
        .create_concept(CreateConceptInput {
            id: "K-900".to_owned(),
            name: "Again".to_owned(),
            area: "distributed".to_owned(),
            definition: String::new(),
        })
        .unwrap_err();
    assert!(matches!(duplicate, AppError::Invalid(_)), "{duplicate:?}");

    assert_eq!(
        app.list_concepts(Some(ConceptStatus::Due)).unwrap().len(),
        1
    );
    assert_eq!(
        app.list_concepts(Some(ConceptStatus::Passed))
            .unwrap()
            .len(),
        0
    );

    let task = app.attach_concept(&task_id, "K-900").expect("attach");
    assert_eq!(task.concept_ids, vec!["K-900".to_owned()]);
    let task = app.attach_concept(&task_id, "K-900").expect("idempotent");
    assert_eq!(task.concept_ids.len(), 1);

    let outcome = app
        .review_concept(
            "K-900",
            "It is when you make sure doing something more than once works fine.",
        )
        .expect("thin review");
    assert!(!outcome.passed);
    assert_eq!(outcome.concept.status, ConceptStatus::Weak);
    assert_eq!(
        millis(outcome.concept.due_on.as_deref().unwrap()),
        NOW + DAY,
        "a thin answer returns tomorrow"
    );
    assert!(outcome.note.starts_with("Thin."));

    let outcome = app
        .review_concept(
            "K-900",
            "Idempotency means the same request applied twice leaves one state, not two.",
        )
        .expect("passing review");
    assert!(outcome.passed, "{}", outcome.note);
    assert_eq!(outcome.concept.status, ConceptStatus::Passed);
    assert_eq!(outcome.concept.seen, 2);
    assert_eq!(
        millis(outcome.concept.due_on.as_deref().unwrap()),
        NOW + 21 * DAY,
        "a pass moves three weeks out"
    );

    let concept = app.set_help("K-900", HelpLevel::None).expect("help level");
    assert_eq!(concept.help, HelpLevel::None);

    let task = app.detach_concept(&task_id, "K-900").expect("detach");
    assert!(task.concept_ids.is_empty());

    let missing = app.review_concept("K-404", "Unknown.").unwrap_err();
    assert!(matches!(missing, AppError::NotFound(_)), "{missing:?}");
}

#[test]
fn a_gate_follows_a_due_concept() {
    let app = app();
    let project = create(&app, "kestrel");
    let task_id = ready_task(&app, &project.id, "Retry policy");

    app.create_concept(CreateConceptInput {
        id: "K-901".to_owned(),
        name: "attempt cap".to_owned(),
        area: "reliability".to_owned(),
        definition: "The line past which a retry becomes a leak.".to_owned(),
    })
    .expect("create concept");
    app.attach_concept(&task_id, "K-901").expect("attach");

    let task = app.draft_gate(&task_id).expect("draft from concept");
    let gate = task.gates[0].clone();
    assert_eq!(gate.concept_id.as_deref(), Some("K-901"));
    assert!(gate.question.contains("attempt cap"));

    let task = app
        .answer_gate(
            &gate.id,
            "The attempt cap stops retries so the downstream does not fail under load.",
        )
        .expect("concept gate");
    assert_eq!(task.gates[0].status, GateStatus::Passed);

    // Passing the gate is also an explain-back: the ledger moved and the
    // concept is scheduled three weeks out.
    let concepts = app.list_concepts(Some(ConceptStatus::Passed)).unwrap();
    assert_eq!(concepts.len(), 1);
    assert_eq!(concepts[0].seen, 1);
    assert_eq!(
        millis(concepts[0].due_on.as_deref().unwrap()),
        NOW + 21 * DAY
    );

    // The concept is no longer due, so the next gate falls back to the task.
    let task = app.draft_gate(&task_id).expect("fallback draft");
    let next = task.gates.last().expect("second gate");
    assert_eq!(next.concept_id, None);
    assert!(next.question.contains("Retry policy"));
}

#[test]
fn settings_are_persisted_with_an_event() {
    let app = app();
    assert_eq!(app.bootstrap().unwrap().settings.model, ModelChoice::Local);

    let settings = app.set_model(ModelChoice::Api).expect("set model");
    assert_eq!(settings.model, ModelChoice::Api);
    assert_eq!(app.bootstrap().unwrap().settings.model, ModelChoice::Api);

    let settings = app.add_allowlist_path("src/outbox/**").expect("add path");
    assert_eq!(settings.allowlist, vec!["src/outbox/**".to_owned()]);

    // Re-adding the same path is a no-op and writes no second event.
    let events = app.list_events(None, 100).unwrap().len();
    let settings = app.add_allowlist_path("src/outbox/**").expect("idempotent");
    assert_eq!(settings.allowlist.len(), 1);
    assert_eq!(app.list_events(None, 100).unwrap().len(), events);

    let refused = app.add_allowlist_path("../etc").unwrap_err();
    assert!(matches!(refused, AppError::Invalid(_)), "{refused:?}");

    let settings = app
        .remove_allowlist_path("src/outbox/**")
        .expect("remove path");
    assert!(settings.allowlist.is_empty());
    let missing = app.remove_allowlist_path("src/outbox/**").unwrap_err();
    assert!(matches!(missing, AppError::NotFound(_)), "{missing:?}");

    let events = app.list_events(None, 100).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::SettingChanged)
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::ReadScopeChanged)
    );
}
