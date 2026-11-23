//! M5 exit checks: the model may phrase a question and offer advice. It cannot
//! score, store, approve, or see a file outside the read scope.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use antik_app::{
    App,
    dto::{
        CreateProjectInput, CreateTaskInput, EventKind, GateSource, GateStatus, ModelChoice,
        ModelHealth, PlanItemInput, PlanItemKind, ProjectDetail,
    },
    error::AppError,
    ports::{
        GateContext, GateDraftText, IdGen, Model, ModelAdvice, ModelError, ModelTarget, NoRunner,
        PlanContext, ProjectFiles, ScannedProject, ScopedFile,
    },
};
use antik_domain::scope::Pattern;
use antik_domain::{Clock, Timestamp};
use antik_infra::Db;

const MODEL_QUESTION: &str =
    "What does the attempt cap protect in the outbox, and what breaks if it is lifted?";

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

/// A read scope that hands the application one prepared file and records what
/// it was actually asked for.
struct Scope {
    files: Vec<ScopedFile>,
    asked: Mutex<Vec<String>>,
}

impl ProjectFiles for Scope {
    fn write_project(&self, _project: &ProjectDetail) -> Result<(), AppError> {
        Ok(())
    }

    fn scan(&self, _project: &ProjectDetail) -> Result<ScannedProject, AppError> {
        Ok(ScannedProject {
            brief: None,
            notes: Vec::new(),
        })
    }

    fn project_root(&self, _project: &ProjectDetail) -> Option<PathBuf> {
        None
    }

    fn sandbox_root(&self) -> Option<PathBuf> {
        None
    }

    fn read_scope(
        &self,
        _project: &ProjectDetail,
        fence: &[Pattern],
        _scope: &[Pattern],
    ) -> Vec<ScopedFile> {
        if fence.is_empty() {
            return Vec::new();
        }
        self.asked
            .lock()
            .unwrap()
            .extend(self.files.iter().map(|file| file.path.clone()));
        self.files
            .iter()
            .map(|file| ScopedFile {
                path: file.path.clone(),
                text: file.text.clone(),
                truncated: false,
            })
            .collect()
    }
}

struct Fake {
    reject: bool,
    seen: Mutex<Vec<String>>,
}

impl Model for Fake {
    fn draft_gate(
        &self,
        target: &ModelTarget,
        context: &GateContext,
    ) -> Result<GateDraftText, ModelError> {
        self.seen
            .lock()
            .unwrap()
            .extend(context.files.iter().map(|file| file.path.clone()));
        if self.reject {
            return Err(ModelError::new("the local model is not running"));
        }
        assert_eq!(target.model, "qwen3.5:9b");
        Ok(GateDraftText {
            question: MODEL_QUESTION.to_owned(),
            hint: "Start from the caller that has already given up.".to_owned(),
        })
    }

    fn critique_plan(
        &self,
        _target: &ModelTarget,
        context: &PlanContext,
    ) -> Result<ModelAdvice, ModelError> {
        if self.reject {
            return Err(ModelError::new("the local model is not running"));
        }
        Ok(ModelAdvice {
            points: vec![
                format!("The prediction for \"{}\" names no failure.", context.title),
                "Separate the cap from the delay: one is a request budget.".to_owned(),
            ],
        })
    }

    fn health(&self, target: &ModelTarget) -> ModelHealth {
        ModelHealth {
            configured: true,
            reachable: !self.reject,
            base_url: target.base_url.clone(),
            model: target.model.clone(),
            detail: if self.reject {
                "the local model is not running".to_owned()
            } else {
                "ready".to_owned()
            },
        }
    }
}

fn fixture(reject: bool) -> (App<Db>, Arc<Fake>, Arc<Scope>) {
    let scope = Arc::new(Scope {
        files: vec![ScopedFile {
            path: "src/queue/flush.rs".to_owned(),
            text: "fn flush() {}".to_owned(),
            truncated: false,
        }],
        asked: Mutex::new(Vec::new()),
    });
    let model = Arc::new(Fake {
        reject,
        seen: Mutex::new(Vec::new()),
    });
    let app = App::with_model(
        Arc::new(Db::open_in_memory().expect("open store")),
        Arc::new(FixedClock),
        Arc::new(SeqIds(AtomicU64::new(0))),
        scope.clone(),
        Arc::new(NoRunner),
        model.clone(),
    );
    (app, model, scope)
}

fn project(app: &App<Db>) -> ProjectDetail {
    app.create_project(CreateProjectInput {
        name: "kestrel".to_owned(),
        brief: "A local event log.".to_owned(),
    })
    .expect("create project")
}

/// A second batch, moved to verify, ready for a gate.
fn verify_task(app: &App<Db>, project_id: &str) -> String {
    let task = app
        .create_task(
            project_id,
            CreateTaskInput {
                title: "Retry policy for the outbox".to_owned(),
                scope: "src/queue/flush.rs".to_owned(),
                prediction: String::new(),
            },
        )
        .expect("create task");
    app.write_prediction(
        &task.id,
        "Five attempts is enough because the delay doubles each time.",
    )
    .expect("prediction");
    app.add_plan_item(
        &task.id,
        PlanItemInput {
            kind: PlanItemKind::Dod,
            parent_id: None,
            title: "A sixth attempt is refused".to_owned(),
            body: String::new(),
            check_id: None,
        },
    )
    .expect("definition of done");
    app.approve_plan(&task.id).expect("approve plan");
    app.close_implement(&task.id).expect("close implement");
    task.id
}

#[test]
fn the_model_phrases_the_question_and_the_app_still_scores_it() {
    let (app, model, _scope) = fixture(false);
    let project = project(&app);
    app.add_allowlist_path("src/**").expect("allowlist");
    let task_id = verify_task(&app, &project.id);

    let task = app.draft_gate(&task_id).expect("draft gate");
    let gate = task.gates[0].clone();
    assert_eq!(gate.drafted_by, GateSource::Model);
    assert_eq!(gate.question, MODEL_QUESTION);
    assert_eq!(
        model.seen.lock().unwrap().as_slice(),
        ["src/queue/flush.rs"],
        "the model saw exactly what the scope handed it"
    );

    // The rubric came from the app, so a real answer still scores the same way.
    let answered = app
        .answer_gate(
            &gate.id,
            "The outbox cap protects the request budget: without it a sixth wait outlives the \
             caller that started it, and the work goes to the wrong place.",
        )
        .expect("answer");
    assert_eq!(answered.gates[0].status, GateStatus::Passed);
    assert_eq!(answered.gates[0].drafted_by, GateSource::Model);
}

#[test]
fn a_dead_model_fails_open_and_the_log_says_so() {
    let (app, _model, _scope) = fixture(true);
    let project = project(&app);
    let task_id = verify_task(&app, &project.id);

    let task = app.draft_gate(&task_id).expect("the gate still exists");
    assert_eq!(task.gates[0].drafted_by, GateSource::App);
    assert!(task.gates[0].question.contains("naive way"));
    assert!(!task.gates[0].hint.is_empty());

    let events = app.list_events(None, 100).expect("events");
    assert!(
        events
            .iter()
            .any(|event| event.kind == EventKind::GateDrafted
                && event.summary.contains("model unavailable")),
        "the bypass is in the log"
    );
}

#[test]
fn advice_needs_a_prediction_and_writes_nothing() {
    let (app, _model, _scope) = fixture(false);
    let project = project(&app);
    let task = app
        .create_task(
            &project.id,
            CreateTaskInput {
                title: "Retry policy for the outbox".to_owned(),
                scope: "src/queue/flush.rs".to_owned(),
                prediction: String::new(),
            },
        )
        .expect("create task");

    assert!(matches!(
        app.critique_plan(&task.id).unwrap_err(),
        AppError::Invalid(_)
    ));

    app.write_prediction(&task.id, "Five attempts is enough.")
        .expect("prediction");
    let before = app.list_events(None, 100).expect("events").len();
    let critique = app.critique_plan(&task.id).expect("critique");
    assert_eq!(critique.points.len(), 2);
    assert_eq!(critique.model, "qwen3.5:9b");
    assert_eq!(
        app.list_events(None, 100).expect("events").len(),
        before,
        "advice is not a mutation"
    );
}

#[test]
fn with_the_model_off_the_gate_still_runs() {
    let (app, _model, _scope) = fixture(false);
    let project = project(&app);
    let task_id = verify_task(&app, &project.id);

    app.set_model(ModelChoice::None).expect("model off");
    let health = app.model_health();
    assert!(!health.configured);
    assert!(!health.reachable);
    assert!(matches!(
        app.critique_plan(&task_id).unwrap_err(),
        AppError::ModelUnavailable(_)
    ));

    let task = app.draft_gate(&task_id).expect("draft gate");
    assert_eq!(task.gates[0].drafted_by, GateSource::App);

    // And with the local choice, health reports the fake endpoint as ready.
    app.set_model(ModelChoice::Local).expect("model on");
    assert!(app.model_health().reachable);
}

#[test]
fn the_endpoint_must_be_on_this_machine() {
    let (app, _model, _scope) = fixture(false);
    assert!(matches!(
        app.set_model_endpoint("http://example.com:11434", "qwen3:14b"),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        app.set_model_endpoint("https://127.0.0.1:11434", "qwen3:14b"),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        app.set_model_endpoint("http://127.0.0.1:11434", "two words"),
        Err(AppError::Invalid(_))
    ));

    let settings = app
        .set_model_endpoint("http://127.0.0.1:11434/", "qwen3:8b")
        .expect("a loopback endpoint");
    assert_eq!(settings.model_url, "http://127.0.0.1:11434");
    assert_eq!(settings.model_name, "qwen3:8b");
}
