pub mod commands;
pub mod jobs;

use std::sync::Arc;

use antik_app::App as AppServices;
use antik_infra::{Db, MarkdownFiles, Ollama, ProcessRunner, SystemClock, SystemIds};
use tauri::Manager;

/// One registry for commands and their types. Bindings are exported from it,
/// so a command that is added here cannot drift from the TypeScript side.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::bootstrap,
        commands::create_project,
        commands::get_project,
        commands::set_project_path,
        commands::set_current_task,
        commands::update_brief,
        commands::add_note,
        commands::delete_note,
        commands::delete_task,
        commands::create_task,
        commands::get_task,
        commands::write_prediction,
        commands::approve_plan,
        commands::add_plan_item,
        commands::update_plan_item,
        commands::set_plan_item_status,
        commands::move_plan_item,
        commands::delete_plan_item,
        commands::send_plan_message,
        commands::suggest_plan,
        commands::accept_plan_proposal,
        commands::add_task_note,
        commands::record_attempt,
        commands::close_implement,
        commands::write_handoff,
        commands::start_session,
        commands::submit_recall,
        commands::get_handoff,
        commands::close_session,
        commands::list_events,
        commands::review,
        commands::export_store,
        commands::draft_gate,
        commands::list_gates,
        commands::answer_gate,
        commands::use_hint,
        commands::override_gate,
        commands::list_concepts,
        commands::create_concept,
        commands::attach_concept,
        commands::detach_concept,
        commands::review_concept,
        commands::set_help,
        commands::set_model,
        commands::add_allowlist_path,
        commands::remove_allowlist_path,
        commands::set_model_endpoint,
        commands::model_health,
        commands::critique_plan,
        commands::add_decision,
        commands::add_question,
        commands::answer_question,
        commands::add_check,
        commands::remove_check,
        commands::get_run,
        commands::run_checks,
        commands::cancel_run,
        commands::run_drill,
        commands::add_drill,
    ])
}

pub fn export_bindings(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let builder = specta_builder();
    builder.export(specta_typescript::Typescript::default(), path)?;
    Ok(())
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    antik_infra::logging::init(&data_dir.join("logs"))?;
    let db = Db::open(data_dir.join("antik.db"))?;
    let projects_dir = data_dir.join("projects");
    std::fs::create_dir_all(&projects_dir)?;
    let services = AppServices::with_model(
        Arc::new(db),
        Arc::new(SystemClock),
        Arc::new(SystemIds),
        Arc::new(MarkdownFiles::new(projects_dir)),
        Arc::new(ProcessRunner::new()),
        Arc::new(Ollama::new()),
    );
    app.manage(services);
    app.manage(Arc::new(jobs::JobManager::new()));
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    export_bindings("../src/bindings.ts").expect("failed to export TypeScript bindings");

    let builder = specta_builder();

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(setup_app)
        .run(tauri::generate_context!())
        .expect("error while running antik");
}

#[cfg(test)]
mod binding_tests {
    #[test]
    fn bindings_export() {
        super::export_bindings("../src/bindings.ts").expect("export bindings");
    }
}
