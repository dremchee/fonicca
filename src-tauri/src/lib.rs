mod audio;
mod commands;
mod db;
mod error;
mod player;
mod scan;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub db_path: PathBuf,
    pub player: player::Player,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db_path = dir.join("library.db");
            let mut conn = db::open(&db_path)?;
            db::migrate(&mut conn)?;
            let source_ids: Vec<i64> = conn
                .prepare("SELECT id FROM sources")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            app.manage(AppState { db: Mutex::new(conn), db_path, player: player::Player::new() });

            // Pick up changes made while the app was closed.
            let state = app.state::<AppState>();
            for id in source_ids {
                commands::spawn_scan(app.handle(), &state, id);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sources,
            commands::add_source,
            commands::remove_source,
            commands::rescan_source,
            commands::count_samples,
            commands::list_samples,
            commands::get_peaks,
            commands::play,
            commands::stop,
            commands::seek,
            commands::playback_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
