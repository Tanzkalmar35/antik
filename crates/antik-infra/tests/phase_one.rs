//! The Phase 1 flow from `idea.md`, end to end against a real SQLite file.
//!
//! This is the M1 exit test: plan, prediction, approval, implement, verify,
//! handoff, recall. It also checks I2 (handoff withheld) and I4 (one event per
//! mutation, revision moves with it).

use std::sync::Arc;

use antik_app::{
    App,
    dto::{
        CreateProjectInput, CreateTaskInput, EventKind, HandoffInput, Phase, PlanItemInput,
        PlanItemKind,
    },
    error::AppError,
    ports::{IdGen, NoFiles},
};
use antik_domain::{Clock, Timestamp};
use antik_infra::{Db, MarkdownFiles};

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000)
    }
}

struct SystemIds;

impl IdGen for SystemIds {
    fn next(&self) -> String {
        ulid::Ulid::generate().to_string()
    }
}

fn app() -> App<Db> {
    let db = Db::open_in_memory().expect("open store");
    App::new(
        Arc::new(db),
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

#[test]
fn phase_one_runs_end_to_end() {
    let app = app();
    let project = create(&app, "kestrel");

    assert_eq!(project.tasks.len(), 1);
    assert_eq!(project.current_task_id, Some(project.tasks[0].id.clone()));
    let task_id = project.tasks[0].id.clone();
    assert_eq!(project.tasks[0].phase, Phase::Plan);
    assert!(!project.tasks[0].plan_approved);

    // A plan cannot be approved before a prediction exists.
    let refused = app.approve_plan(&task_id).unwrap_err();
    assert!(matches!(refused, AppError::Invalid(_)), "{refused:?}");

    let task = app
        .write_prediction(&task_id, "Four attempts are enough; the fifth is the cap.")
        .expect("prediction");
    assert!(!task.prediction.is_empty());
    app.add_plan_item(
        &task_id,
        PlanItemInput {
            kind: PlanItemKind::Dod,
            parent_id: None,
            title: "A sixth attempt is refused".to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .expect("definition of done");

    let task = app.approve_plan(&task_id).expect("approve");
    assert_eq!(task.phase, Phase::Implement);
    assert!(task.plan_approved);

    let task = app
        .add_task_note(&task_id, "Moved the cap into policy.ts.")
        .expect("note");
    assert_eq!(task.notes.len(), 1);

    let task = app
        .record_attempt(
            &task_id,
            "Counted the cap after the delay; the last wait failed.",
        )
        .expect("attempt");
    assert_eq!(task.attempts.len(), 1);

    let task = app.close_implement(&task_id).expect("close implement");
    assert_eq!(task.phase, Phase::Verify);

    // A handoff is written from verify and closes the open session.
    let session = app.start_session(&project.id).expect("start session");
    assert!(!session.has_recall);
    assert!(
        app.get_handoff(&project.id).is_err(),
        "there is no handoff yet"
    );

    let task = app
        .write_handoff(
            &task_id,
            HandoffInput {
                next: "Write the batch after the outbox.".to_owned(),
                blocking: "The timer can fire twice inside one window.".to_owned(),
                thinking: "The cap should be counted before the delay.".to_owned(),
            },
        )
        .expect("handoff");
    assert_eq!(task.phase, Phase::Reflect);
    assert!(task.reflected);

    // I2: the handoff is locked until this session's recall is stored.
    let locked = app.get_handoff(&project.id).unwrap_err();
    assert!(matches!(locked, AppError::Locked(_)), "{locked:?}");

    let session = app.start_session(&project.id).expect("new session");
    assert!(!session.has_recall);
    assert!(app.get_handoff(&project.id).is_err());

    let recall = app
        .submit_recall(
            &project.id,
            "The timer fires twice; next is the outbox batch.",
        )
        .expect("recall");
    assert_eq!(recall.matched_lines, 2);
    assert!(recall.handoff.is_some());

    let handoff = app.get_handoff(&project.id).expect("handoff unlocked");
    assert!(handoff.next.contains("outbox"));

    // I4: one event per mutation; create_project and the handoff write two.
    let events = app.list_events(Some(&project.id), 100).expect("events");
    assert_eq!(events.len(), 13, "events: {events:?}");
    assert_eq!(events[0].kind, EventKind::RecallStored);
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::HandoffWritten)
    );

    let bootstrap = app.bootstrap().expect("bootstrap");
    assert_eq!(bootstrap.revision, 11, "one bump per successful mutation");
    assert_eq!(bootstrap.projects.len(), 1);
    assert_eq!(
        bootstrap.projects[0].current_task.as_ref().unwrap().phase,
        Phase::Reflect
    );

    // The review read model is derived from the same log, not from the view.
    let review = app.review(30).expect("review");
    assert_eq!(review.tasks, 1);
    assert_eq!(review.rework, 1);
    assert_eq!(review.rows.len(), 1);
    assert_eq!(review.rows[0].task_id, task_id);
    assert_eq!(review.rows[0].lead_days, Some(0.0));
    assert_eq!(review.gate_pass_rate, None, "no gates exist yet");

    let export = app.export_store().expect("export");
    let snapshot: serde_json::Value = serde_json::from_str(&export).expect("JSON export");
    assert_eq!(snapshot["schemaVersion"], serde_json::json!(6));
    assert_eq!(snapshot["tables"]["projects"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["tables"]["events"].as_array().unwrap().len(), 13);
}

#[test]
fn external_brief_and_note_edits_are_imported() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("projects");
    std::fs::create_dir_all(&root).unwrap();
    let app = App::new(
        Arc::new(Db::open_in_memory().unwrap()),
        Arc::new(FixedClock),
        Arc::new(SystemIds),
        Arc::new(MarkdownFiles::new(&root)),
    );
    let project = create(&app, "kestrel");
    let project_dir = root.join(&project.id);

    // Editing in Obsidian: the brief body changes on disk.
    let brief = project_dir.join("brief.md");
    let raw = std::fs::read_to_string(&brief).unwrap();
    let edited = raw.replace(
        "A local event log for one developer's tools.",
        "An imported brief.",
    );
    std::fs::write(&brief, edited).unwrap();

    // A note written outside the app, with its own id.
    let note_id = ulid::Ulid::generate().to_string();
    std::fs::write(
        project_dir.join("notes").join(format!("{note_id}.md")),
        format!(
            "---\nantik: 1\nid: {note_id}\nkind: note\nproject: {}\ncreated: 2026-11-21T10:00:00Z\ntitle: Imported\n---\n\nA note from Obsidian.\n",
            project.id
        ),
    )
    .unwrap();

    let project = app.get_project(&project.id).expect("import");
    assert_eq!(project.brief, "An imported brief.");
    assert_eq!(project.notes.len(), 1);
    assert_eq!(project.notes[0].id, note_id);
    assert_eq!(project.notes[0].title, "Imported");

    // A second open changes nothing: the projection matches the store.
    let revision = project.revision;
    let again = app.get_project(&project.id).unwrap();
    assert_eq!(again.revision, revision);
    assert_eq!(again.notes.len(), 1);

    let events = app.list_events(Some(&project.id), 100).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event.summary.contains("imported from markdown"))
            .count(),
        2
    );
}

#[test]
fn brief_writes_respect_the_loaded_revision() {
    let app = app();
    let project = create(&app, "plumb");

    let updated = app
        .update_brief(&project.id, "Second brief.", Some(project.revision))
        .expect("brief");
    assert_eq!(updated.brief, "Second brief.");
    assert_eq!(updated.revision, project.revision + 1);

    let conflict = app
        .update_brief(&project.id, "Third brief.", Some(project.revision))
        .unwrap_err();
    assert!(matches!(conflict, AppError::Conflict(_)), "{conflict:?}");
}

#[test]
fn unresolved_gate_blocks_the_handoff() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("antik.db");
    let app = App::new(
        Arc::new(Db::open(&path).unwrap()),
        Arc::new(FixedClock),
        Arc::new(SystemIds),
        Arc::new(NoFiles),
    );
    let project = create(&app, "saltmarsh");
    let task_id = project.tasks[0].id.clone();
    app.write_prediction(&task_id, "Read the chapter twice.")
        .unwrap();
    app.add_plan_item(
        &task_id,
        PlanItemInput {
            kind: PlanItemKind::Dod,
            parent_id: None,
            title: "Every claim has a source".to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .unwrap();
    app.approve_plan(&task_id).unwrap();
    app.close_implement(&task_id).unwrap();

    // Gates arrive in M3; this row is the guard's only input for now.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO gates (id, task_id, n, question, rubric_json, status, drafted_by)
         VALUES ('g1', ?1, 1, 'Why?', '[]', 'due', 'app')",
        [&task_id],
    )
    .unwrap();
    drop(conn);

    let denial = app
        .write_handoff(
            &task_id,
            HandoffInput {
                next: "next".to_owned(),
                blocking: "blocking".to_owned(),
                thinking: "thinking".to_owned(),
            },
        )
        .unwrap_err();
    assert!(matches!(denial, AppError::Invalid(_)), "{denial:?}");
}

#[test]
fn notes_can_be_added_and_deleted_and_are_logged() {
    let app = app();
    let project = create(&app, "foundry");

    let project = app
        .add_note(&project.id, "Timing", "The p99 is from staging.")
        .expect("add note");
    assert_eq!(project.notes.len(), 1);
    let note_id = project.notes[0].id.clone();

    let project = app.delete_note(&project.id, &note_id).expect("delete");
    assert!(project.notes.is_empty());

    let events = app.list_events(Some(&project.id), 100).unwrap();
    assert!(events.iter().any(|e| e.kind == EventKind::NoteDeleted));
}

#[test]
fn a_project_switches_between_its_batches() {
    let app = app();
    let project = create(&app, "kestrel");
    let first = project.tasks[0].id.clone();
    let second = app
        .create_task(
            &project.id,
            CreateTaskInput {
                title: "Second batch".to_owned(),
                scope: "src/outbox/**".to_owned(),
                prediction: String::new(),
            },
        )
        .expect("create task");
    assert_eq!(
        app.get_project(&project.id)
            .unwrap()
            .current_task_id
            .as_deref(),
        Some(second.id.as_str())
    );

    let switched = app
        .set_current_task(&project.id, &first)
        .expect("switch back");
    assert_eq!(switched.current_task_id.as_deref(), Some(first.as_str()));
    assert_eq!(switched.tasks.len(), 2);
    // Switching is a pointer move: the batch that was current keeps its phase.
    assert_eq!(second.phase, Phase::Plan);
    let first_task = switched
        .tasks
        .iter()
        .find(|task| task.id == first)
        .expect("first task");
    assert_eq!(first_task.phase, Phase::Plan);

    let events = app.list_events(Some(&project.id), 100).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::TaskSwitched)
    );

    // Asking for the batch that is already current writes no second event.
    let before = events.len();
    app.set_current_task(&project.id, &first).expect("no-op");
    assert_eq!(
        app.list_events(Some(&project.id), 100).unwrap().len(),
        before
    );
}

#[test]
fn a_task_from_another_project_cannot_become_current() {
    let app = app();
    let one = create(&app, "kestrel");
    let two = create(&app, "marsh");
    let foreign = two.tasks[0].id.clone();

    assert!(matches!(
        app.set_current_task(&one.id, &foreign),
        Err(AppError::NotFound(_))
    ));
    assert_eq!(
        app.get_project(&one.id).unwrap().current_task_id,
        one.current_task_id
    );
}

#[test]
fn a_task_can_be_deleted_and_the_pointer_moves() {
    let app = app();
    let project = create(&app, "kestrel");
    let first = project.tasks[0].id.clone();
    let second = app
        .create_task(
            &project.id,
            CreateTaskInput {
                title: "Second batch".to_owned(),
                scope: String::new(),
                prediction: String::new(),
            },
        )
        .expect("create task");

    // Deleting the current task hands the pointer to the remaining one.
    let project = app.delete_task(&second.id).expect("delete current");
    assert_eq!(project.tasks.len(), 1);
    assert_eq!(project.current_task_id.as_deref(), Some(first.as_str()));

    let events = app.list_events(Some(&project.id), 100).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::TaskDeleted)
    );

    // A project with no tasks keeps no dangling pointer, and the task is gone.
    let project = app.delete_task(&first).expect("delete last");
    assert!(project.tasks.is_empty());
    assert_eq!(project.current_task_id, None);
    assert!(matches!(app.get_task(&first), Err(AppError::NotFound(_))));
}

#[test]
fn a_second_task_becomes_current() {
    let app = app();
    let project = create(&app, "kestrel");

    let task = app
        .create_task(
            &project.id,
            CreateTaskInput {
                title: "Second batch".to_owned(),
                scope: "src/outbox/**".to_owned(),
                prediction: String::new(),
            },
        )
        .expect("create task");
    assert_eq!(task.phase, Phase::Plan);
    assert_eq!(task.batch, "2 of 2");

    let project = app.get_project(&project.id).unwrap();
    assert_eq!(project.current_task_id, Some(task.id));
    assert_eq!(project.tasks.len(), 2);
}
