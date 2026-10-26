//! Query the native state again immediately before an updater can exit the app.
use std::sync::atomic::Ordering;
use serde::Serialize;
use tauri::{AppHandle, State};
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    configured: bool,
    supported: bool,
    development: bool,
    editors_open: bool,
    busy: bool,
}

#[tauri::command]
pub fn update_status(app: AppHandle, state: State<'_, AppState>) -> UpdateStatus {
    let configured = app.config().plugins.0.get("updater")
        .and_then(|v| v.get("endpoints")).and_then(|v| v.as_array())
        .is_some_and(|endpoints| !endpoints.is_empty());
    UpdateStatus {
        configured,
        // Debian installations are managed by the package manager, not the updater.
        supported: !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some(),
        development: cfg!(debug_assertions),
        // Require editors to be saved and closed; this also covers edits in other windows.
        editors_open: !state.editor.lock().unwrap().is_empty(),
        busy: state.stems.is_running() || state.apollo.is_running()
            || state.basic_pitch.is_running() || crate::batch::is_running()
            || state.model_operations.is_busy()
            || state.model_downloading.load(Ordering::SeqCst)
            || state.duplicates_verifying.load(Ordering::SeqCst),
    }
}
