use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, OptionalExtension, ToSql};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::audio;
use crate::error::{Error, Result};
use crate::scan;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    id: i64,
    path: String,
    sample_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    id: i64,
    source_id: i64,
    path: String,
    name: String,
    ext: String,
    size: i64,
    duration: Option<f64>,
    sample_rate: Option<i64>,
    channels: Option<i64>,
    bit_depth: Option<i64>,
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

/// Builds the shared WHERE clause for sample listing and counting.
fn sample_filter(query: &Option<String>, source_id: Option<i64>) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(id) = source_id {
        clauses.push("source_id = ?");
        args.push(Box::new(id));
    }
    if let Some(q) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        // Every word must appear in the file name.
        for word in q.split_whitespace() {
            let escaped = word.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            clauses.push("name LIKE ? ESCAPE '\\'");
            args.push(Box::new(format!("%{escaped}%")));
        }
    }
    let sql = if clauses.is_empty() { String::new() } else { format!("WHERE {}", clauses.join(" AND ")) };
    (sql, args)
}

#[tauri::command]
pub fn count_samples(
    state: State<'_, AppState>,
    query: Option<String>,
    source_id: Option<i64>,
) -> Result<i64> {
    let (filter, args) = sample_filter(&query, source_id);
    let conn = state.db.lock().unwrap();
    let n = conn.query_row(
        &format!("SELECT COUNT(*) FROM samples {filter}"),
        rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())),
        |r| r.get(0),
    )?;
    Ok(n)
}

#[tauri::command]
pub fn list_samples(
    state: State<'_, AppState>,
    query: Option<String>,
    source_id: Option<i64>,
    offset: i64,
    limit: i64,
) -> Result<Vec<Sample>> {
    let (filter, mut args) = sample_filter(&query, source_id);
    args.push(Box::new(limit));
    args.push(Box::new(offset));
    let conn = state.db.lock().unwrap();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT id, source_id, path, name, ext, size, duration, sample_rate, channels, bit_depth
         FROM samples {filter}
         ORDER BY name COLLATE NOCASE, path LIMIT ? OFFSET ?"
    ))?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())), |r| {
            Ok(Sample {
                id: r.get(0)?,
                source_id: r.get(1)?,
                path: r.get(2)?,
                name: r.get(3)?,
                ext: r.get(4)?,
                size: r.get(5)?,
                duration: r.get(6)?,
                sample_rate: r.get(7)?,
                channels: r.get(8)?,
                bit_depth: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
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
