//! Attaching a repository. Checks and the read scope move to that directory,
//! and antik's documents move into `.antik` inside it. A path that cannot be
//! canonicalised, or is not a directory, is refused before anything changes.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use antik_app::App;
use antik_app::dto::CreateProjectInput;
use antik_app::error::AppError;
use antik_app::ports::IdGen;
use antik_domain::{Clock, Timestamp};
use antik_infra::{Db, MarkdownFiles};

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

struct Fixture {
    app: App<Db>,
    data: tempfile::TempDir,
    repo: tempfile::TempDir,
}

fn fixture() -> Fixture {
    let data = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let app = App::new(
        Arc::new(Db::open_in_memory().unwrap()),
        Arc::new(FixedClock),
        Arc::new(SeqIds(AtomicU64::new(0))),
        Arc::new(MarkdownFiles::new(data.path())),
    );
    Fixture { app, data, repo }
}

fn repo_string(fixture: &Fixture) -> String {
    fixture
        .repo
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn project(fixture: &Fixture) -> antik_app::dto::ProjectDetail {
    fixture
        .app
        .create_project(CreateProjectInput {
            name: "kestrel".to_owned(),
            brief: "A local event log.".to_owned(),
        })
        .expect("create project")
}

#[test]
fn attaching_a_repository_moves_the_documents_into_dot_antik() {
    let fixture = fixture();
    let project = project(&fixture);
    assert!(
        fixture
            .data
            .path()
            .join(&project.id)
            .join("brief.md")
            .is_file(),
        "before a repository is attached the documents live in app data"
    );

    let expected = repo_string(&fixture);
    let updated = fixture
        .app
        .set_project_path(&project.id, Some(&expected))
        .expect("attach repository");

    assert_eq!(updated.repo_path.as_deref(), Some(expected.as_str()));
    assert!(fixture.repo.path().join(".antik/brief.md").is_file());
    assert!(fixture.repo.path().join(".antik/tasks").is_dir());
}

#[test]
fn a_relative_or_missing_path_is_refused() {
    let fixture = fixture();
    let project = project(&fixture);

    assert!(matches!(
        fixture.app.set_project_path(&project.id, Some("code/repo")),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        fixture
            .app
            .set_project_path(&project.id, Some("/definitely/not/a/directory")),
        Err(AppError::Invalid(_))
    ));
    assert!(
        fixture
            .app
            .get_project(&project.id)
            .unwrap()
            .repo_path
            .is_none()
    );
}

#[test]
fn clearing_the_path_returns_the_documents_to_app_data() {
    let fixture = fixture();
    let project = project(&fixture);
    fixture
        .app
        .set_project_path(&project.id, Some(&repo_string(&fixture)))
        .expect("attach repository");

    let cleared = fixture
        .app
        .set_project_path(&project.id, None)
        .expect("clear path");

    assert!(cleared.repo_path.is_none());
    assert!(
        fixture
            .data
            .path()
            .join(&project.id)
            .join("brief.md")
            .is_file()
    );
}
