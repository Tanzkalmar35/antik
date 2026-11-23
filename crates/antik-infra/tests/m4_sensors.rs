//! M4 exit tests: the exit status is the only result, a cancelled or timed-out
//! run writes nothing, and a drill keeps its failures.
//!
//! The runner here is scripted, so no test starts a process. The process edge
//! itself is covered by the unit tests in `antik-infra::runner`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use antik_app::{
    App,
    dto::{CheckInput, CheckResult, CreateDrillInput, CreateProjectInput, RunStatus},
    error::AppError,
    ports::{CancelToken, IdGen, RunOutput, RunRequest, RunStop, Runner},
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

struct SeqIds(AtomicU64);

impl IdGen for SeqIds {
    fn next(&self) -> String {
        let value = self.0.fetch_add(1, Ordering::SeqCst);
        format!("id-{value:04}")
    }
}

#[derive(Default)]
struct Scripted {
    outputs: Mutex<Vec<RunOutput>>,
    programs: Mutex<Vec<String>>,
    dirs: Mutex<Vec<std::path::PathBuf>>,
}

impl Scripted {
    fn new(outputs: Vec<RunOutput>) -> Arc<Self> {
        Arc::new(Self {
            outputs: Mutex::new(outputs),
            ..Self::default()
        })
    }
}

impl Runner for Scripted {
    fn run(&self, request: &RunRequest, _cancel: &CancelToken) -> RunOutput {
        self.programs.lock().unwrap().push(request.program.clone());
        self.dirs.lock().unwrap().push(request.cwd.clone());
        let mut outputs = self.outputs.lock().unwrap();
        if outputs.is_empty() {
            return exited(0, "script exhausted");
        }
        outputs.remove(0)
    }
}

fn exited(code: i32, stdout: &str) -> RunOutput {
    RunOutput {
        stop: RunStop::Exited(code),
        exit_code: Some(code),
        duration_ms: 12,
        stdout_tail: stdout.to_owned(),
        stderr_tail: String::new(),
        truncated: false,
    }
}

fn stopped(stop: RunStop) -> RunOutput {
    RunOutput {
        stop,
        exit_code: None,
        duration_ms: 5,
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        truncated: false,
    }
}

fn app(runner: Arc<Scripted>) -> (App<Db>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = App::with_runner(
        Arc::new(Db::open_in_memory().expect("open store")),
        Arc::new(FixedClock),
        Arc::new(SeqIds(AtomicU64::new(0))),
        Arc::new(MarkdownFiles::new(dir.path())),
        runner,
    );
    (app, dir)
}

/// A project with one task, which is what checks hang off.
fn project(app: &App<Db>) -> (String, String) {
    let project = app
        .create_project(CreateProjectInput {
            name: "kestrel".to_owned(),
            brief: "A local event log for one developer.".to_owned(),
        })
        .expect("create project");
    (project.id.clone(), project.tasks[0].id.clone())
}

fn unit_check() -> CheckInput {
    CheckInput {
        label: "unit".to_owned(),
        detail: "covers the cap and the delay".to_owned(),
        program: "cargo".to_owned(),
        args: "test --lib".to_owned(),
    }
}

fn property_check() -> CheckInput {
    CheckInput {
        label: "property".to_owned(),
        detail: "no sixth retry is attempted".to_owned(),
        program: "cargo".to_owned(),
        args: "test --test property".to_owned(),
    }
}

#[test]
fn a_run_writes_the_exit_status_and_nothing_else() {
    let runner = Scripted::new(vec![exited(0, "24 passed"), exited(1, "")]);
    let (app, _dir) = app(runner.clone());
    let (project_id, task_id) = project(&app);
    app.add_check(&task_id, unit_check()).expect("unit check");
    app.add_check(&task_id, property_check())
        .expect("property check");

    let start = app.start_checks(&task_id).expect("reserve run");
    let run = app
        .execute_checks(start, &CancelToken::new())
        .expect("run checks");

    assert_eq!(run.status, RunStatus::Done);
    assert_eq!(run.results.len(), 2);
    assert_eq!(run.results[0].result, CheckResult::Pass);
    assert_eq!(run.results[0].exit_code, Some(0));
    assert_eq!(run.results[1].result, CheckResult::Fail);
    assert_eq!(run.results[1].exit_code, Some(1));

    let task = app.get_task(&task_id).expect("task");
    let unit = task.checks.iter().find(|c| c.label == "unit").unwrap();
    assert_eq!(unit.result, CheckResult::Pass);
    assert_eq!(unit.command, "cargo test --lib");
    assert!(unit.evaluated_at.is_some());
    let property = task.checks.iter().find(|c| c.label == "property").unwrap();
    assert_eq!(property.result, CheckResult::Fail);

    // The runner ran inside the project's folder, not the app data root.
    assert_eq!(runner.programs.lock().unwrap().len(), 2);
    assert!(runner.dirs.lock().unwrap()[0].ends_with(&project_id));

    // The run is readable again, which is what the screen polls.
    let again = app.check_run(&run.id).expect("read run");
    assert_eq!(again.results.len(), 2);
    assert_eq!(again.status, RunStatus::Done);
}

#[test]
fn a_cancelled_run_writes_no_result_and_leaves_every_check_alone() {
    let runner = Scripted::new(vec![stopped(RunStop::Cancelled)]);
    let (app, _dir) = app(runner);
    let (_, task_id) = project(&app);
    app.add_check(&task_id, unit_check()).expect("check");

    let cancel = CancelToken::new();
    cancel.cancel();
    let start = app.start_checks(&task_id).expect("reserve run");
    let run = app.execute_checks(start, &cancel).expect("run checks");

    assert_eq!(run.status, RunStatus::Cancelled);
    assert!(run.results.is_empty(), "a cancelled run writes nothing");

    let task = app.get_task(&task_id).expect("task");
    assert_eq!(task.checks[0].result, CheckResult::Pending);
    assert!(task.checks[0].evaluated_at.is_none());
}

#[test]
fn a_timeout_fails_the_run_without_writing_a_verdict() {
    let runner = Scripted::new(vec![stopped(RunStop::TimedOut)]);
    let (app, _dir) = app(runner);
    let (_, task_id) = project(&app);
    app.add_check(&task_id, unit_check()).expect("check");

    let start = app.start_checks(&task_id).expect("reserve run");
    let run = app
        .execute_checks(start, &CancelToken::new())
        .expect("run checks");

    assert_eq!(run.status, RunStatus::Failed);
    assert!(run.results.is_empty());
    let task = app.get_task(&task_id).expect("task");
    assert_eq!(task.checks[0].result, CheckResult::Pending);
}

#[test]
fn a_program_outside_the_allowlist_is_refused_before_it_is_stored() {
    let runner = Scripted::new(Vec::new());
    let (app, _dir) = app(runner);
    let (_, task_id) = project(&app);

    let error = app
        .add_check(
            &task_id,
            CheckInput {
                label: "shell".to_owned(),
                detail: "not allowed".to_owned(),
                program: "sh".to_owned(),
                args: "-c whoami".to_owned(),
            },
        )
        .expect_err("a shell is not a check program");
    assert!(matches!(error, AppError::Invalid(_)));
    assert!(app.get_task(&task_id).expect("task").checks.is_empty());

    let piped = app.add_check(
        &task_id,
        CheckInput {
            label: "piped".to_owned(),
            detail: "no shell".to_owned(),
            program: "cargo".to_owned(),
            args: "test | head".to_owned(),
        },
    );
    assert!(piped.is_err(), "there is no shell to pipe into");
}

#[test]
fn a_check_is_authored_removed_and_logged() {
    let runner = Scripted::new(Vec::new());
    let (app, _dir) = app(runner);
    let (_, task_id) = project(&app);

    let task = app.add_check(&task_id, unit_check()).expect("add check");
    assert_eq!(task.checks.len(), 1);
    let check_id = task.checks[0].id.clone();

    let task = app.remove_check(&check_id).expect("remove check");
    assert!(task.checks.is_empty());

    let events = app.list_events(None, 50).expect("events");
    let kinds: Vec<_> = events.iter().map(|event| event.kind).collect();
    assert!(kinds.contains(&antik_app::dto::EventKind::CheckAdded));
    assert!(kinds.contains(&antik_app::dto::EventKind::CheckRemoved));
}

#[test]
fn a_drill_keeps_the_failure_it_produced() {
    let runner = Scripted::new(vec![exited(1, "assertion failed: attempts <= 5")]);
    let (app, _dir) = app(runner);
    let drill = app
        .add_drill(CreateDrillInput {
            skill: "concurrency".to_owned(),
            name: "cancel a token".to_owned(),
            command: "cargo test --test cancel".to_owned(),
        })
        .expect("add drill");

    let outcome = app.run_drill(&drill.id).expect("run drill");
    assert!(!outcome.passed, "exit 1 is not a pass");
    assert_eq!(outcome.exit_code, Some(1));
    assert_eq!(outcome.drill.attempts, 1);
    assert!(!outcome.drill.passed);
    assert!(outcome.output.contains("assertion failed"));

    // The attempt is in the ledger, and the run is in the job table.
    let drafts = app.bootstrap().expect("bootstrap");
    assert_eq!(drafts.drills[0].attempts, 1);
    assert!(!drafts.drills[0].passed);
}

#[test]
fn a_drill_needs_an_allowlisted_program() {
    let runner = Scripted::new(Vec::new());
    let (app, _dir) = app(runner);

    let shell = app.add_drill(CreateDrillInput {
        skill: "concurrency".to_owned(),
        name: "read the source".to_owned(),
        command: "sh -c 'cat src/lib.rs'".to_owned(),
    });
    assert!(shell.is_err(), "a drill cannot name a shell");

    let piped = app.add_drill(CreateDrillInput {
        skill: "concurrency".to_owned(),
        name: "piped".to_owned(),
        command: "cargo test | head".to_owned(),
    });
    assert!(piped.is_err(), "there is no shell to pipe into");
}
