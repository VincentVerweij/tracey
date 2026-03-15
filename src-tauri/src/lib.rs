use tauri::Manager;

mod commands;
mod db;
mod models;
mod platform;
mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            env_logger::init();
            log::info!("Tracey starting up");
            // DB initialization happens here in T008
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Commands registered here as they are implemented
            // T013: commands::preferences::preferences_get,
            // T013: commands::preferences::preferences_update,
            // T014: commands::health::health_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
