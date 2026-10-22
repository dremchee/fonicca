use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::audio;
use crate::error::{Error, Result};
use crate::library::{self, Filter, Sample, Tag, TagRef};
use crate::scan;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    id: i64,
    path: String,
    sample_count: i64,
}

pub fn spawn_scan(app: &AppHandle, state: &AppState, source_id: i64) {
    let app = app.clone();
    let db_path = state.db_path.clone();
    std::thread::spawn(move || {
        scan::scan_source(&db_path, source_id, |p| {
            let _ = app.emit(scan::PROGRESS_EVENT, p);
        })
    });
}

#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> Result<Vec<Source>> {
    let conn = state.db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT s.id, s.path, COUNT(x.id) FROM sources s
         LEFT JOIN samples x ON x.source_id = s.id
         GROUP BY s.id ORDER BY s.path COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| Ok(Source { id: r.get(0)?, path: r.get(1)?, sample_count: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn add_source(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<i64> {
    if !PathBuf::from(&path).is_dir() {
        return Err(Error::Other(format!("not a folder: {path}")));
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let id: i64 = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "INSERT INTO sources (path, added_at) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET path = excluded.path RETURNING id",
            params![path, now],
            |r| r.get(0),
        )?
    };
    spawn_scan(&app, &state, id);
    Ok(id)
}

#[tauri::command]
pub fn remove_source(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db.lock().unwrap().execute("DELETE FROM sources WHERE id = ?1", [id])?;
    Ok(())
}

#[tauri::command]
pub fn rescan_source(app: AppHandle, state: State<'_, AppState>, id: i64) {
    spawn_scan(&app, &state, id);
}

#[tauri::command]
pub fn count_samples(state: State<'_, AppState>, filter: Filter) -> Result<i64> {
    library::count(&state.db.lock().unwrap(), &filter)
}

#[tauri::command]
pub fn list_samples(
    state: State<'_, AppState>,
    filter: Filter,
    offset: i64,
    limit: i64,
) -> Result<Vec<Sample>> {
    library::list(&state.db.lock().unwrap(), &filter, offset, limit)
}

#[tauri::command]
pub fn sample_ids(state: State<'_, AppState>, filter: Filter) -> Result<Vec<i64>> {
    library::ids(&state.db.lock().unwrap(), &filter)
}

#[tauri::command]
pub fn sample_paths(state: State<'_, AppState>, ids: Vec<i64>) -> Result<Vec<String>> {
    library::paths(&state.db.lock().unwrap(), &ids)
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, ids: Vec<i64>, favorite: bool) -> Result<()> {
    library::set_favorite(&mut state.db.lock().unwrap(), &ids, favorite)
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Result<Vec<Tag>> {
    library::list_tags(&state.db.lock().unwrap())
}

/// Adds a tag (created on first use) to the given samples.
#[tauri::command]
pub fn tag_samples(state: State<'_, AppState>, ids: Vec<i64>, name: String) -> Result<TagRef> {
    let mut conn = state.db.lock().unwrap();
    let tag = library::ensure_tag(&conn, &name)?;
    library::tag_samples(&mut conn, &ids, tag.id)?;
    Ok(tag)
}

#[tauri::command]
pub fn untag_samples(state: State<'_, AppState>, ids: Vec<i64>, tag_id: i64) -> Result<()> {
    library::untag_samples(&mut state.db.lock().unwrap(), &ids, tag_id)
}

#[tauri::command]
pub fn rename_tag(state: State<'_, AppState>, id: i64, name: String) -> Result<()> {
    library::rename_tag(&state.db.lock().unwrap(), id, &name)
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, id: i64) -> Result<()> {
    library::delete_tag(&state.db.lock().unwrap(), id)
}

fn sample_path(state: &AppState, id: i64) -> Result<PathBuf> {
    let conn = state.db.lock().unwrap();
    let path: String = conn
        .query_row("SELECT path FROM samples WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Other(format!("sample {id} not found")))?;
    Ok(PathBuf::from(path))
}

/// Returns cached waveform peaks, decoding the file on first request.
#[tauri::command]
pub async fn get_peaks(state: State<'_, AppState>, id: i64) -> Result<Vec<i8>> {
    let cached: Option<Vec<u8>> = state
        .db
        .lock()
        .unwrap()
        .query_row("SELECT peaks FROM samples WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .flatten();
    if let Some(bytes) = cached {
        return Ok(bytes.into_iter().map(|b| b as i8).collect());
    }

    let path = sample_path(&state, id)?;
    let peaks = tauri::async_runtime::spawn_blocking(move || audio::compute_peaks(&path))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    let bytes: Vec<u8> = peaks.iter().map(|&p| p as u8).collect();
    state.db.lock().unwrap().execute("UPDATE samples SET peaks = ?1 WHERE id = ?2", params![bytes, id])?;
    Ok(peaks)
}

#[tauri::command]
pub fn play(state: State<'_, AppState>, id: i64, start: Option<f64>) -> Result<()> {
    let path = sample_path(&state, id)?;
    state.player.play(&path, start.unwrap_or(0.0))
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) {
    state.player.stop();
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position: f64) -> Result<()> {
    state.player.seek(position)
}

#[tauri::command]
pub fn playback_position(state: State<'_, AppState>) -> Option<f64> {
    state.player.position()
}
