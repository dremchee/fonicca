mod analysis;
mod analyzer;
mod audio;
mod category;
mod clap;
mod commands;
mod db;
mod duplicates;
mod editor;
mod editor_commands;
mod error;
mod library;
mod models;
mod player;
mod scan;
mod settings;
mod similar;
mod sync;
mod watch;

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub player: player::Player,
    pub scanner: Arc<watch::Scanner>,
    pub analyzer: Arc<analyzer::Analyzer>,
    pub models_dir: std::path::PathBuf,
    pub db_path: std::path::PathBuf,
    pub settings: settings::SettingsStore,
    pub similar: Mutex<similar::Similar>,
    /// The file open in the editor, if any.
    pub editor: Mutex<Option<editor::Session>>,
    pub model_downloading: std::sync::atomic::AtomicBool,
    pub duplicates_verifying: std::sync::atomic::AtomicBool,
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
            let analyzer = analyzer::Analyzer::start(db_path.clone(), move |p| {
                let _ = handle.emit(analyzer::PROGRESS_EVENT, p);
            });
            let models_dir = dir.join("models");
            if models::is_installed(&models_dir) {
                let analyzer = Arc::clone(&analyzer);
                let path = models::model_path(&models_dir);
                std::thread::spawn(move || {
                    if let Err(e) = analyzer.load_model(&path) {
                        eprintln!("{e}");
                    }
                });
            }
            let handle = app.handle().clone();
            let scan_analyzer = Arc::clone(&analyzer);
            let scanner = watch::Scanner::new(db_path.clone(), move |p| {
                if p.done && p.changes > 0 {
                    scan_analyzer.wake();
                }
                let _ = handle.emit(scan::PROGRESS_EVENT, p);
            });
            let watcher = watch::LibraryWatcher::new(Arc::clone(&scanner));
            let settings = settings::SettingsStore::load(dir.join("settings.json"));
            // Before the sources are registered, so a disabled watcher never starts watching.
            watcher.set_enabled(settings.get().watch_folders);
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
                analyzer,
                models_dir,
                db_path,
                settings,
                similar: Default::default(),
                editor: Mutex::new(None),
                model_downloading: Default::default(),
                duplicates_verifying: Default::default(),
                watcher,
            });
            let state = app.state::<AppState>();
            commands::apply_settings(app.handle(), &state, None, &state.settings.get())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            editor_commands::editor_open,
            editor_commands::editor_close,
            editor_commands::editor_apply,
            editor_commands::editor_undo,
            editor_commands::editor_redo,
            editor_commands::editor_peaks,
            editor_commands::editor_play,
            editor_commands::editor_save,
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
            commands::prioritize_analysis,
            commands::set_analysis_paused,
            commands::analysis_status,
            commands::sound_model_status,
            commands::download_sound_model,
            commands::play,
            commands::stop,
            commands::seek,
            commands::playback_position,
            commands::get_settings,
            commands::update_settings,
            commands::app_info,
            commands::delete_sound_model,
            commands::duplicate_count,
            commands::duplicate_groups,
            commands::verify_duplicates,
            commands::resolve_duplicates,
            commands::list_hidden,
            commands::unhide_samples,
            commands::preferred_folders,
            commands::set_preferred_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
