use commands::AppState;
use commands::SyncState;

mod commands;
mod db;
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

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            db: std::sync::Mutex::new(conn),
            platform,
            sync_state,
            sync_notify,
        })
        .setup(|app| {
            log::info!("Tracey starting up");
            services::timer_tick::start_tick_loop(app.handle().clone());
            services::idle_service::start_idle_loop(app.handle().clone());
            services::screenshot_service::start_screenshot_loop(app.handle().clone());
            services::sync_service::start_sync_loop(app.handle().clone());
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
