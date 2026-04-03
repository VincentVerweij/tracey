use commands::AppState;
use commands::SyncState;
use commands::ClassificationState;

mod commands;
pub mod db;
mod models;
mod platform;
mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    commands::init_health();
    let conn = db::open().expect("DB init failed");

    use platform::windows::WindowsPlatformHooks;
    use std::sync::Arc;
    let platform: Arc<dyn platform::PlatformHooks + Send + Sync> = Arc::new(WindowsPlatformHooks);
    let sync_state = Arc::new(std::sync::Mutex::new(SyncState::default()));
    let sync_notify = Arc::new(tokio::sync::Notify::new());

    // Load persisted classification model and rules at startup
    let classification_state = {
        use services::classification::trainer;
        let model = trainer::load_model(&conn);
        let rules_json: Option<String> = conn
            .query_row("SELECT classification_rules_json FROM user_preferences LIMIT 1", [], |r| r.get(0))
            .ok()
            .flatten();
        let rules: Vec<services::classification::heuristic::HeuristicRule> = rules_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        Arc::new(std::sync::Mutex::new(ClassificationState {
            model,
            rules,
            sample_count_at_last_train: trainer::count_samples(&conn),
        }))
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            db: std::sync::Mutex::new(conn),
            platform,
            sync_state,
            sync_notify,
            classification_state,
        })
        .setup(|app| {
            log::info!("Tracey starting up");
            services::timer_tick::start_tick_loop(app.handle().clone());
            services::idle_service::start_idle_loop(app.handle().clone());
            services::screenshot_service::start_screenshot_loop(app.handle().clone());
            services::sync_service::start_sync_loop(app.handle().clone());
            services::activity_tracker::start_activity_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::preferences_get,
            commands::preferences_update,
            commands::health_get,
            commands::timer::timer_start,
            commands::timer::timer_stop,
            commands::timer::timer_get_active,
            commands::timer::time_entry_list,
            commands::timer::time_entry_autocomplete,
            commands::timer::time_entry_create_manual,
            commands::timer::time_entry_continue,
            commands::timer::time_entry_update,
            commands::timer::time_entry_delete,
            commands::idle::idle_get_status,
            commands::idle::idle_resolve,
            commands::hierarchy::client_list,
            commands::hierarchy::client_create,
            commands::hierarchy::client_update,
            commands::hierarchy::client_archive,
            commands::hierarchy::client_unarchive,
            commands::hierarchy::client_delete,
            commands::hierarchy::project_list,
            commands::hierarchy::project_create,
            commands::hierarchy::project_update,
            commands::hierarchy::project_archive,
            commands::hierarchy::project_unarchive,
            commands::hierarchy::project_delete,
            commands::hierarchy::task_list,
            commands::hierarchy::task_create,
            commands::hierarchy::task_update,
            commands::hierarchy::task_delete,
            commands::hierarchy::fuzzy_match_projects,
            commands::hierarchy::fuzzy_match_tasks,
            commands::screenshot::screenshot_list,
            commands::screenshot::screenshot_delete_expired,
            commands::activity::tag_list,
            commands::activity::tag_create,
            commands::activity::tag_delete,
            commands::sync::sync_configure,
            commands::sync::sync_get_status,
            commands::sync::sync_trigger,
            commands::data::data_delete_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
