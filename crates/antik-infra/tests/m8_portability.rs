//! Portability: a folder that carries antik documents is readable by another
//! installation. The frontmatter names the project from the machine that wrote
//! it, so the folder — not the id — decides whose documents these are. A pull
//! that arrives after the repository is attached is read at the next launch.

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use antik_app::App;
use antik_app::dto::{
    CreateProjectInput, CreateTaskInput, Phase, PlanItemInput, PlanItemKind, ProjectDetail,
};
use antik_app::ports::IdGen;
use antik_domain::{Clock, Timestamp};
use antik_infra::{Db, MarkdownFiles};

struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_millis(1_700_000_000_000)
    }
}

/// Ids carry the installation's prefix so two stores in one test can never
/// agree by accident.
struct SeqIds {
    prefix: &'static str,
    next: AtomicU64,
}

impl SeqIds {
    fn new(prefix: &'static str) -> Self {
        Self {
            prefix,
            next: AtomicU64::new(0),
        }
    }
}

impl IdGen for SeqIds {
    fn next(&self) -> String {
        format!(
            "{}-{:04}",
            self.prefix,
            self.next.fetch_add(1, Ordering::SeqCst)
        )
    }
}

/// One installation: its own database and its own app-data folder.
struct Device {
    app: App<Db>,
    _data: tempfile::TempDir,
}

fn device(prefix: &'static str) -> Device {
    let data = tempfile::tempdir().unwrap();
    let app = App::new(
        Arc::new(Db::open_in_memory().unwrap()),
        Arc::new(FixedClock),
        Arc::new(SeqIds::new(prefix)),
        Arc::new(MarkdownFiles::new(data.path())),
    );
    Device { app, _data: data }
}

fn create(app: &App<Db>, name: &str, brief: &str) -> ProjectDetail {
    app.create_project(CreateProjectInput {
        name: name.to_owned(),
        brief: brief.to_owned(),
    })
    .expect("create project")
}

fn repo_path(repo: &tempfile::TempDir) -> String {
    repo.path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn attaching_a_folder_from_another_installation_loads_its_documents() {
    let repo = tempfile::tempdir().unwrap();

    let first = device("a");
    let project = create(&first.app, "kestrel", "A local event log.");
    first
        .app
        .add_note(
            &project.id,
            "Timing",
            "The timer fires twice inside one window.",
        )
        .expect("note");
    first
        .app
        .set_project_path(&project.id, Some(&repo_path(&repo)))
        .expect("attach repository");
    assert!(repo.path().join(".antik/brief.md").is_file());

    // The folder is pulled on another machine, whose store knows nothing about
    // the project that wrote it.
    let second = device("b");
    let fresh = create(&second.app, "kestrel", "");
    let loaded = second
        .app
        .set_project_path(&fresh.id, Some(&repo_path(&repo)))
        .expect("attach repository");

    assert_eq!(loaded.brief, "A local event log.");
    assert_eq!(loaded.notes.len(), 1);
    assert_eq!(
        loaded.notes[0].text,
        "The timer fires twice inside one window."
    );

    // The documents are claimed, so the next read finds nothing new.
    let again = second.app.get_project(&fresh.id).expect("read project");
    assert_eq!(again.notes.len(), 1);
    let brief = fs::read_to_string(repo.path().join(".antik/brief.md")).unwrap();
    assert!(
        brief.contains(&format!("project: {}", fresh.id)),
        "the brief is re-projected under this installation's project id: {brief}"
    );
}

#[test]
fn documents_that_arrive_on_disk_are_loaded_at_launch() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    let first = device("a");
    let project = create(&first.app, "kestrel", "A local event log.");
    first
        .app
        .add_note(&project.id, "Timing", "The timer fires twice.")
        .expect("note");
    let note_id = first
        .app
        .get_project(&project.id)
        .expect("read project")
        .notes[0]
        .id
        .clone();
    first
        .app
        .set_project_path(&project.id, Some(&repo_path(&source)))
        .expect("attach repository");

    // The machine that receives the pull has already attached an empty
    // repository, and the documents turn up afterwards.
    let second = device("b");
    let fresh = create(&second.app, "kestrel", "");
    second
        .app
        .set_project_path(&fresh.id, Some(&repo_path(&target)))
        .expect("attach repository");

    let pulled = target.path().join(".antik");
    fs::create_dir_all(pulled.join("notes")).unwrap();
    fs::copy(
        source.path().join(".antik/brief.md"),
        pulled.join("brief.md"),
    )
    .unwrap();
    fs::copy(
        source
            .path()
            .join(".antik/notes")
            .join(format!("{note_id}.md")),
        pulled.join("notes").join(format!("{note_id}.md")),
    )
    .unwrap();

    let boot = second.app.bootstrap().expect("launch");
    assert_eq!(boot.projects.len(), 1);
    let loaded = second.app.get_project(&fresh.id).expect("read project");
    assert_eq!(loaded.brief, "A local event log.");
    assert_eq!(loaded.notes.len(), 1);
    assert_eq!(loaded.notes[0].text, "The timer fires twice.");

    // A second launch adopts nothing twice.
    let boot = second.app.bootstrap().expect("second launch");
    assert_eq!(boot.projects.len(), 1);
    assert_eq!(
        second
            .app
            .get_project(&fresh.id)
            .expect("read project")
            .notes
            .len(),
        1
    );
}

#[test]
fn a_task_document_becomes_a_task_with_its_plan() {
    let repo = tempfile::tempdir().unwrap();

    let first = device("a");
    let project = create(&first.app, "kestrel", "A local event log.");
    let task = first
        .app
        .create_task(
            &project.id,
            CreateTaskInput {
                title: "Drain the queue".to_owned(),
                scope: "src/queue/**".to_owned(),
                prediction: "One worker is enough.".to_owned(),
            },
        )
        .expect("task");
    let section = first
        .app
        .add_plan_item(
            &task.id,
            PlanItemInput {
                kind: PlanItemKind::Section,
                parent_id: None,
                title: "Measurement".to_owned(),
                body: "Find the slow path.".to_owned(),
                check_id: None,
            },
        )
        .expect("section");
    let section_id = section
        .plan_items
        .iter()
        .find(|item| item.kind == PlanItemKind::Section)
        .expect("section item")
        .id
        .clone();
    first
        .app
        .add_plan_item(
            &task.id,
            PlanItemInput {
                kind: PlanItemKind::Subtask,
                parent_id: Some(section_id),
                title: "Time one flush".to_owned(),
                body: "Use the dev database.".to_owned(),
                check_id: None,
            },
        )
        .expect("subtask");
    first
        .app
        .add_plan_item(
            &task.id,
            PlanItemInput {
                kind: PlanItemKind::Dod,
                parent_id: None,
                title: "The queue drains under a second.".to_owned(),
                body: String::new(),
                check_id: None,
            },
        )
        .expect("definition of done");
    first
        .app
        .set_project_path(&project.id, Some(&repo_path(&repo)))
        .expect("attach repository");
    assert!(
        repo.path()
            .join(".antik/tasks")
            .join(format!("{}.md", task.id))
            .is_file()
    );

    // A second installation pulls the repository and attaches it. Its own
    // project keeps the task it was already on.
    let second = device("b");
    let fresh = create(&second.app, "kestrel", "");
    let loaded = second
        .app
        .set_project_path(&fresh.id, Some(&repo_path(&repo)))
        .expect("attach repository");

    assert_eq!(
        loaded.current_task_id, fresh.current_task_id,
        "an imported task does not take over the current pointer"
    );
    let imported = loaded
        .tasks
        .iter()
        .find(|entry| entry.title == "Drain the queue")
        .expect("the task document is loaded");
    assert!(matches!(imported.phase, Phase::Plan));
    assert_eq!(imported.scope, "src/queue/**");
    assert_eq!(imported.prediction, "One worker is enough.");
    let section = imported
        .plan_items
        .iter()
        .find(|item| item.kind == PlanItemKind::Section)
        .expect("the section is loaded");
    assert_eq!(section.title, "Measurement");
    assert_eq!(section.body, "Find the slow path.");
    assert!(imported.plan_items.iter().any(|item| {
        item.kind == PlanItemKind::Subtask
            && item.parent_id.as_deref() == Some(section.id.as_str())
            && item.title == "Time one flush"
            && item.body == "Use the dev database."
    }));
    assert!(imported.plan_items.iter().any(|item| {
        item.kind == PlanItemKind::Dod && item.title == "The queue drains under a second."
    }));

    // The document is claimed and re-projected under this project, once.
    let boot = second.app.bootstrap().expect("launch");
    assert_eq!(boot.projects.len(), 1);
    let doc = fs::read_to_string(
        repo.path()
            .join(".antik/tasks")
            .join(format!("{}.md", task.id)),
    )
    .unwrap();
    assert!(doc.contains(&format!("project: {}", fresh.id)));
    assert!(doc.contains("### 1. Measurement"));
    // The folder holds two task documents (the project's first task and this
    // one); both are imported once and neither comes back a second time.
    let tasks = second
        .app
        .get_project(&fresh.id)
        .expect("read project")
        .tasks;
    assert_eq!(
        tasks
            .iter()
            .filter(|entry| entry.title == "Drain the queue")
            .count(),
        1,
        "the imported task does not duplicate"
    );
}

#[test]
fn a_document_that_names_another_local_project_is_left_alone() {
    let repo = tempfile::tempdir().unwrap();
    let dev = device("c");

    let first = create(&dev.app, "kestrel", "A local event log.");
    dev.app
        .add_note(&first.id, "Timing", "The timer fires twice.")
        .expect("note");
    dev.app
        .set_project_path(&first.id, Some(&repo_path(&repo)))
        .expect("attach repository");

    // The second project attaches the same directory. The documents there name
    // a project this store already has, so they are not adopted a second time.
    let second = create(&dev.app, "petrel", "");
    let attached = dev
        .app
        .set_project_path(&second.id, Some(&repo_path(&repo)))
        .expect("attach repository");
    assert!(attached.notes.is_empty());
    assert_eq!(attached.brief, "");

    let owner = dev.app.get_project(&first.id).expect("read project");
    assert_eq!(owner.notes.len(), 1);
    assert_eq!(owner.brief, "A local event log.");
}
