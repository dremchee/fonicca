mod app_icon;
mod analysis;
mod apollo;
mod analyzer;
mod audio;
mod batch;
mod category;
mod clap;
mod commands;
mod db;
mod duplicates;
mod editor;
mod editor_commands;
mod error;
mod library;
mod meter;
mod models;
mod natural_sort;
mod overview;
mod player;
mod scan;
mod settings;
mod stems;
mod basic_pitch;
mod similar;
mod sync;
mod tags;
mod watch;
mod updates;

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub apollo: apollo::Jobs,
    pub stems: stems::Jobs,
    pub basic_pitch: basic_pitch::Jobs,
    pub db: Mutex<Connection>,
    pub player: player::Player,
    pub scanner: Arc<watch::Scanner>,
    pub analyzer: Arc<analyzer::Analyzer>,
    pub models_dir: std::path::PathBuf,
    pub db_path: std::path::PathBuf,
    pub settings: settings::SettingsStore,
    pub similar: Mutex<similar::Similar>,
    /// Waveform overviews of recently previewed samples.
    pub overviews: overview::Cache,
    /// Files open in editor windows, by window label.
    pub editor: Mutex<std::collections::HashMap<String, editor::Session>>,
    pub model_operations: models::Operations,
    pub model_downloading: std::sync::atomic::AtomicBool,
    pub duplicates_verifying: std::sync::atomic::AtomicBool,
    pub watcher: watch::LibraryWatcher,
}

/// Windows start hidden and the page shows itself once styled, so there is no white
/// flash. Should the page fail before that, show the window anyway after a moment.
pub(crate) fn show_window_fallback(app: &tauri::AppHandle, label: &str) {
    let app = app.clone();
    let label = label.to_owned();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if let Some(window) = app.get_webview_window(&label) {
            if !window.is_visible().unwrap_or(true) {
                let _ = window.show();
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
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
            let player = player::Player::new();
            app.manage(AppState {
                stems: Default::default(),
                basic_pitch: Default::default(),
                apollo: Default::default(),
                db: Mutex::new(conn),
                player,
                scanner,
                analyzer,
                models_dir,
                db_path,
                settings,
                similar: Default::default(),
                overviews: overview::Cache::with_disk(app.path().app_cache_dir()?.join("waveforms")),
                editor: Default::default(),
                model_operations: Default::default(),
                model_downloading: Default::default(),
                duplicates_verifying: Default::default(),
                watcher,
            });
            let state = app.state::<AppState>();
            commands::apply_settings(app.handle(), &state, None, &state.settings.get())?;
            show_window_fallback(app.handle(), "main");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::ThemeChanged(_)) {
                app_icon::refresh(window.app_handle());
            }
            if matches!(event, tauri::WindowEvent::Destroyed)
                && window.label().starts_with(editor_commands::WINDOW_PREFIX)
            {
                editor_commands::release(window.app_handle(), window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            basic_pitch::basic_pitch_status,
            basic_pitch::basic_pitch_start,
            basic_pitch::basic_pitch_cancel,
            updates::update_status,
            editor_commands::open_editor_window,
            editor_commands::editor_open,
            editor_commands::editor_apply,
            editor_commands::editor_process,
            editor_commands::editor_undo,
            editor_commands::editor_redo,
            editor_commands::editor_wave,
            editor_commands::editor_spectrogram,
            batch::batch_preview,
            batch::batch_run,
            batch::batch_cancel,
            editor_commands::editor_copy,
            editor_commands::editor_clipboard,
            editor_commands::editor_paste,
            editor_commands::editor_zero_snap,
            editor_commands::editor_slices,
            editor_commands::editor_export_slices,
            editor_commands::editor_play,
            editor_commands::editor_update_playback,
            editor_commands::editor_save,
            commands::list_sources,
            commands::source_folders,
            commands::add_source,
            commands::remove_source,
            commands::rescan_source,
            commands::count_samples,
            commands::list_samples,
            commands::sample_ids,
            commands::sample_paths,
            commands::sample_tags,
            commands::sample_cover,
            commands::update_sample_tags,
            commands::cover_preview,
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
            commands::sample_wave,
            commands::prioritize_analysis,
            commands::set_analysis_paused,
            commands::analysis_status,
            commands::sound_model_status,
            commands::model_storage,
            commands::download_model,
            commands::delete_model,
            commands::download_sound_model,
            apollo::apollo_status,
            apollo::apollo_start,
            apollo::apollo_cancel,
            stems::stem_status,
            stems::stem_start,
            stems::stem_cancel,
            commands::play,
            commands::stop,
            commands::pause,
            commands::resume,
            commands::seek,
            commands::playback_position,
            commands::playback_levels,
            commands::get_settings,
            commands::update_settings,
            commands::app_info,
            commands::delete_sound_model,
            commands::export_theme,
            commands::import_theme,
            commands::duplicate_count,
            commands::duplicate_groups,
            commands::verify_duplicates,
            commands::resolve_duplicates,
            commands::list_hidden,
            commands::unhide_samples,
            commands::preferred_folders,
            commands::set_preferred_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Tauri applies its development icon just before Ready. Override it afterwards.
            if matches!(event, tauri::RunEvent::Ready) {
                app_icon::refresh(app);
            }
        });
}
