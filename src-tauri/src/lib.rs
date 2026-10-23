mod audio;
mod commands;
mod db;
mod error;
mod library;
mod player;
mod scan;
mod watch;

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub player: player::Player,
    pub scanner: Arc<watch::Scanner>,
    pub watcher: watch::LibraryWatcher,
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
            db::prune_detached(&conn)?;
            let sources: Vec<(i64, String)> = conn
                .prepare("SELECT id, path FROM sources")?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;

            let handle = app.handle().clone();
            let scanner = watch::Scanner::new(db_path, move |p| {
                let _ = handle.emit(scan::PROGRESS_EVENT, p);
            });
            let watcher = watch::LibraryWatcher::new(Arc::clone(&scanner));
            for (id, path) in &sources {
                if let Err(e) = watcher.watch(*id, std::path::Path::new(path)) {
                    eprintln!("cannot watch {path}: {e}");
                }
                // Pick up changes made while the app was closed.
                scanner.request(*id);
            }
            app.manage(AppState {
                db: Mutex::new(conn),
                player: player::Player::new(),
                scanner,
                watcher,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sources,
            commands::add_source,
            commands::remove_source,
            commands::rescan_source,
            commands::count_samples,
            commands::list_samples,
            commands::sample_ids,
            commands::sample_paths,
            commands::set_favorite,
            commands::list_tags,
            commands::tag_samples,
            commands::untag_samples,
            commands::rename_tag,
            commands::delete_tag,
            commands::list_collections,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::add_to_collection,
            commands::remove_from_collection,
            commands::export_user_data,
            commands::import_user_data,
            commands::get_peaks,
            commands::play,
            commands::stop,
            commands::seek,
            commands::playback_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
