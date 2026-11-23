use std::sync::Arc;

use antik_app::{
    App,
    dto::{
        Bootstrap, CheckInput, ConceptReviewOutcome, ConceptStatus, ConceptSummary,
        CreateConceptInput, CreateDecisionInput, CreateDrillInput, CreateProjectInput,
        CreateQuestionInput, CreateTaskInput, Critique, DrillOutcome, DrillSummary, EventSummary,
        GateDetail, HandoffDetail, HandoffInput, HelpLevel, ModelChoice, ModelHealth,
        PlanChatReply, PlanDraftInput, PlanItemInput, PlanProposal, ProjectDetail, RecallOutcome,
        Review, RunDetail, SessionView, Settings, TaskDetail,
    },
    error::{AppError, AppErrorDto},
};
use antik_infra::Db;

use crate::jobs::JobManager;

type Services = App<Db>;

/// Wrap one synchronous use case in `spawn_blocking` so the async runtime is
/// never blocked by SQLite.
async fn run<T, F>(work: F) -> Result<T, AppErrorDto>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| AppErrorDto::internal(format!("blocking task failed: {error}")))?
        .map_err(AppErrorDto::from)
}

fn services(state: &tauri::State<'_, Services>) -> Services {
    state.inner().clone()
}

/// Everything the shell needs at launch, in one round trip.
#[tauri::command]
#[specta::specta]
pub async fn bootstrap(state: tauri::State<'_, Services>) -> Result<Bootstrap, AppErrorDto> {
    let app = services(&state);
    run(move || app.bootstrap()).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: tauri::State<'_, Services>,
    input: CreateProjectInput,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.create_project(input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_project(
    state: tauri::State<'_, Services>,
    id: String,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.get_project(&id)).await
}

/// Attaches a local repository to a project, or clears it with `null`.
#[tauri::command]
#[specta::specta]
pub async fn set_project_path(
    state: tauri::State<'_, Services>,
    project_id: String,
    path: Option<String>,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.set_project_path(&project_id, path.as_deref())).await
}

/// Points a project at another of its batches. Navigating to a batch does not
/// change the current one; this command is the explicit switch.
#[tauri::command]
#[specta::specta]
pub async fn set_current_task(
    state: tauri::State<'_, Services>,
    project_id: String,
    task_id: String,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.set_current_task(&project_id, &task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_brief(
    state: tauri::State<'_, Services>,
    id: String,
    body: String,
    expected_revision: Option<u32>,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.update_brief(&id, &body, expected_revision)).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_note(
    state: tauri::State<'_, Services>,
    project_id: String,
    title: String,
    text: String,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_note(&project_id, &title, &text)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_note(
    state: tauri::State<'_, Services>,
    project_id: String,
    note_id: String,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.delete_note(&project_id, &note_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_task(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.delete_task(&task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_task(
    state: tauri::State<'_, Services>,
    project_id: String,
    input: CreateTaskInput,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.create_task(&project_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_task(
    state: tauri::State<'_, Services>,
    id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.get_task(&id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn write_prediction(
    state: tauri::State<'_, Services>,
    task_id: String,
    text: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.write_prediction(&task_id, &text)).await
}

#[tauri::command]
#[specta::specta]
pub async fn approve_plan(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.approve_plan(&task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_plan_item(
    state: tauri::State<'_, Services>,
    task_id: String,
    input: PlanItemInput,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_plan_item(&task_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_plan_item(
    state: tauri::State<'_, Services>,
    item_id: String,
    title: String,
    body: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.update_plan_item(&item_id, &title, &body)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_plan_item_status(
    state: tauri::State<'_, Services>,
    item_id: String,
    done: bool,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.set_plan_item_status(&item_id, done)).await
}

#[tauri::command]
#[specta::specta]
pub async fn move_plan_item(
    state: tauri::State<'_, Services>,
    item_id: String,
    direction: i32,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.move_plan_item(&item_id, direction)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_plan_item(
    state: tauri::State<'_, Services>,
    item_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.delete_plan_item(&item_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_plan_message(
    state: tauri::State<'_, Services>,
    task_id: String,
    text: String,
) -> Result<PlanChatReply, AppErrorDto> {
    let app = services(&state);
    run(move || app.send_plan_message(&task_id, &text)).await
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_plan(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<PlanProposal, AppErrorDto> {
    let app = services(&state);
    run(move || app.suggest_plan(&task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn accept_plan_proposal(
    state: tauri::State<'_, Services>,
    task_id: String,
    input: PlanDraftInput,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.accept_plan_proposal(&task_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_task_note(
    state: tauri::State<'_, Services>,
    task_id: String,
    text: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_task_note(&task_id, &text)).await
}

#[tauri::command]
#[specta::specta]
pub async fn record_attempt(
    state: tauri::State<'_, Services>,
    task_id: String,
    text: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.record_attempt(&task_id, &text)).await
}

#[tauri::command]
#[specta::specta]
pub async fn close_implement(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.close_implement(&task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn write_handoff(
    state: tauri::State<'_, Services>,
    task_id: String,
    input: HandoffInput,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.write_handoff(&task_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn start_session(
    state: tauri::State<'_, Services>,
    project_id: String,
) -> Result<SessionView, AppErrorDto> {
    let app = services(&state);
    run(move || app.start_session(&project_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn submit_recall(
    state: tauri::State<'_, Services>,
    project_id: String,
    text: String,
) -> Result<RecallOutcome, AppErrorDto> {
    let app = services(&state);
    run(move || app.submit_recall(&project_id, &text)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_handoff(
    state: tauri::State<'_, Services>,
    project_id: String,
) -> Result<HandoffDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.get_handoff(&project_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn close_session(
    state: tauri::State<'_, Services>,
    project_id: String,
) -> Result<(), AppErrorDto> {
    let app = services(&state);
    run(move || app.close_session(&project_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_events(
    state: tauri::State<'_, Services>,
    project_id: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<EventSummary>, AppErrorDto> {
    let app = services(&state);
    run(move || app.list_events(project_id.as_deref(), limit.unwrap_or(100))).await
}

#[tauri::command]
#[specta::specta]
pub async fn review(state: tauri::State<'_, Services>, days: u32) -> Result<Review, AppErrorDto> {
    let app = services(&state);
    run(move || app.review(days)).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_store(state: tauri::State<'_, Services>) -> Result<String, AppErrorDto> {
    let app = services(&state);
    run(move || app.export_store()).await
}

#[tauri::command]
#[specta::specta]
pub async fn draft_gate(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.draft_gate(&task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_gates(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<Vec<GateDetail>, AppErrorDto> {
    let app = services(&state);
    run(move || app.list_gates(&task_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn answer_gate(
    state: tauri::State<'_, Services>,
    gate_id: String,
    answer: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.answer_gate(&gate_id, &answer)).await
}

#[tauri::command]
#[specta::specta]
pub async fn use_hint(
    state: tauri::State<'_, Services>,
    gate_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.use_hint(&gate_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn override_gate(
    state: tauri::State<'_, Services>,
    gate_id: String,
    reason: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.override_gate(&gate_id, &reason)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_concepts(
    state: tauri::State<'_, Services>,
    status: Option<ConceptStatus>,
) -> Result<Vec<ConceptSummary>, AppErrorDto> {
    let app = services(&state);
    run(move || app.list_concepts(status)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_concept(
    state: tauri::State<'_, Services>,
    input: CreateConceptInput,
) -> Result<ConceptSummary, AppErrorDto> {
    let app = services(&state);
    run(move || app.create_concept(input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn attach_concept(
    state: tauri::State<'_, Services>,
    task_id: String,
    concept_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.attach_concept(&task_id, &concept_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn detach_concept(
    state: tauri::State<'_, Services>,
    task_id: String,
    concept_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.detach_concept(&task_id, &concept_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn review_concept(
    state: tauri::State<'_, Services>,
    concept_id: String,
    answer: String,
) -> Result<ConceptReviewOutcome, AppErrorDto> {
    let app = services(&state);
    run(move || app.review_concept(&concept_id, &answer)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_help(
    state: tauri::State<'_, Services>,
    concept_id: String,
    help: HelpLevel,
) -> Result<ConceptSummary, AppErrorDto> {
    let app = services(&state);
    run(move || app.set_help(&concept_id, help)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_model(
    state: tauri::State<'_, Services>,
    model: ModelChoice,
) -> Result<Settings, AppErrorDto> {
    let app = services(&state);
    run(move || app.set_model(model)).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_allowlist_path(
    state: tauri::State<'_, Services>,
    path: String,
) -> Result<Settings, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_allowlist_path(&path)).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_allowlist_path(
    state: tauri::State<'_, Services>,
    path: String,
) -> Result<Settings, AppErrorDto> {
    let app = services(&state);
    run(move || app.remove_allowlist_path(&path)).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_model_endpoint(
    state: tauri::State<'_, Services>,
    base_url: String,
    name: String,
) -> Result<Settings, AppErrorDto> {
    let app = services(&state);
    run(move || app.set_model_endpoint(&base_url, &name)).await
}

/// Never throws: "off" and "unreachable" are states the Settings screen shows.
#[tauri::command]
#[specta::specta]
pub async fn model_health(state: tauri::State<'_, Services>) -> Result<ModelHealth, AppErrorDto> {
    let app = services(&state);
    run(move || Ok(app.model_health())).await
}

#[tauri::command]
#[specta::specta]
pub async fn critique_plan(
    state: tauri::State<'_, Services>,
    task_id: String,
) -> Result<Critique, AppErrorDto> {
    let app = services(&state);
    run(move || app.critique_plan(&task_id)).await
}

// ---- Decisions and questions ---------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn add_decision(
    state: tauri::State<'_, Services>,
    project_id: String,
    input: CreateDecisionInput,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_decision(&project_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_question(
    state: tauri::State<'_, Services>,
    project_id: String,
    input: CreateQuestionInput,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_question(&project_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn answer_question(
    state: tauri::State<'_, Services>,
    question_id: String,
    answer: String,
) -> Result<ProjectDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.answer_question(&question_id, &answer)).await
}

// ---- Sensors -------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn add_check(
    state: tauri::State<'_, Services>,
    task_id: String,
    input: CheckInput,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_check(&task_id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_check(
    state: tauri::State<'_, Services>,
    check_id: String,
) -> Result<TaskDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.remove_check(&check_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_run(
    state: tauri::State<'_, Services>,
    run_id: String,
) -> Result<RunDetail, AppErrorDto> {
    let app = services(&state);
    run(move || app.check_run(&run_id)).await
}

/// Reserves the run, then returns its id immediately. The worker owns the
/// process slot, finishes the run in storage, and announces it on
/// `antik://job-finished`; `get_run` is the polling fallback.
#[tauri::command]
#[specta::specta]
pub async fn run_checks(
    state: tauri::State<'_, Services>,
    jobs: tauri::State<'_, Arc<JobManager>>,
    app: tauri::AppHandle,
    task_id: String,
) -> Result<String, AppErrorDto> {
    let services = services(&state);
    let manager = Arc::clone(jobs.inner());
    let permit = manager.try_slot().map_err(AppErrorDto::from)?;
    let worker = services.clone();
    let start = run(move || services.start_checks(&task_id)).await?;
    let run_id = start.run_id.clone();
    let cancel = manager.register(&run_id);
    let id = run_id.clone();

    tauri::async_runtime::spawn(async move {
        let outcome =
            tauri::async_runtime::spawn_blocking(move || worker.execute_checks(start, &cancel))
                .await;
        manager.finish(&id);
        drop(permit);

        use tauri::Emitter;
        let payload = match outcome {
            Ok(Ok(detail)) => serde_json::json!({
                "runId": id,
                "status": detail.status,
                "results": detail.results.len(),
            }),
            Ok(Err(error)) => serde_json::json!({
                "runId": id,
                "status": "failed",
                "message": error.to_string(),
            }),
            Err(error) => serde_json::json!({
                "runId": id,
                "status": "failed",
                "message": error.to_string(),
            }),
        };
        let _ = app.emit("antik://job-finished", payload);
    });

    Ok(run_id)
}

/// The signal only. The worker notices it, kills the process group, and marks
/// the run cancelled; nothing is written by this call.
#[tauri::command]
#[specta::specta]
pub fn cancel_run(
    jobs: tauri::State<'_, Arc<JobManager>>,
    run_id: String,
) -> Result<bool, AppErrorDto> {
    Ok(jobs.inner().cancel(&run_id))
}

#[tauri::command]
#[specta::specta]
pub async fn add_drill(
    state: tauri::State<'_, Services>,
    input: CreateDrillInput,
) -> Result<DrillSummary, AppErrorDto> {
    let app = services(&state);
    run(move || app.add_drill(input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn run_drill(
    state: tauri::State<'_, Services>,
    jobs: tauri::State<'_, Arc<JobManager>>,
    drill_id: String,
) -> Result<DrillOutcome, AppErrorDto> {
    let services = services(&state);
    // A drill is short, but it still takes a process slot so it cannot run
    // alongside two full suites.
    let _permit = jobs.inner().try_slot().map_err(AppErrorDto::from)?;
    run(move || services.run_drill(&drill_id)).await
}
