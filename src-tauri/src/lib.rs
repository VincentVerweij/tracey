use commands::AppState;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            db: std::sync::Mutex::new(conn),
        })
        .setup(|_app| {
            log::info!("Tracey starting up");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::preferences_get,
            commands::preferences_update,
            commands::health_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
