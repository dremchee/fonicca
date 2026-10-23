//! Tauri commands for the single-file editor. One file is open at a time.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

use crate::editor::{self, Op, Session, State as EditorState};
use crate::error::{Error, Result};
use crate::AppState;

fn with_session<T>(state: &AppState, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
    let mut guard = state.editor.lock().unwrap();
    let session = guard.as_mut().ok_or_else(|| Error::Other("no file is open in the editor".into()))?;
    f(session)
}

#[tauri::command]
pub async fn editor_open(state: State<'_, AppState>, id: i64) -> Result<EditorState> {
    let (path, bit_depth): (String, Option<u32>) = state.db.lock().unwrap().query_row(
        "SELECT path, bit_depth FROM samples WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let session = Session::open(id, Path::new(&path), bit_depth)?;
    let s = session.state();
    *state.editor.lock().unwrap() = Some(session);
    Ok(s)
}

#[tauri::command]
pub fn editor_close(state: State<'_, AppState>) {
    state.player.stop();
    state.editor.lock().unwrap().take();
}

#[tauri::command]
pub fn editor_apply(state: State<'_, AppState>, op: Op) -> Result<EditorState> {
    with_session(&state, |s| {
        s.apply(op);
        Ok(s.state())
    })
}

#[tauri::command]
pub fn editor_undo(state: State<'_, AppState>) -> Result<EditorState> {
    with_session(&state, |s| {
        s.undo();
        Ok(s.state())
    })
}

#[tauri::command]
pub fn editor_redo(state: State<'_, AppState>) -> Result<EditorState> {
    with_session(&state, |s| {
        s.redo();
        Ok(s.state())
    })
}

/// Waveform of `[start, end)` in `columns` min/max pairs (for zoomed views).
#[tauri::command]
pub fn editor_peaks(state: State<'_, AppState>, start: usize, end: usize, columns: usize) -> Result<Vec<i8>> {
    with_session(&state, |s| Ok(editor::peaks(s.audio(), start, end, columns.min(8192))))
}

#[tauri::command]
pub fn editor_play(state: State<'_, AppState>, start: usize, end: usize, looping: bool) -> Result<()> {
    with_session(&state, |s| state.player.play_buffer(s.audio(), start, end, looping))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    path: String,
    /// Where the previous version went when saving over the original.
    backup: Option<String>,
    state: EditorState,
}

/// Saves as a new WAV next to the original ("Kick (edit).wav"), or over the
/// original after copying it to the backups folder. Non-WAV originals are
/// always saved as a new file, since writing WAV would change their format.
#[tauri::command]
pub fn editor_save(state: State<'_, AppState>, as_new: bool) -> Result<Saved> {
    let backups = state.db_path.parent().map(|d| d.join("backups")).unwrap_or_else(|| PathBuf::from("backups"));
    let saved = with_session(&state, |s| {
        let is_wav = s.path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("wav"));
        let (target, backup) = if as_new || !is_wav {
            (editor::edited_path(&s.path), None)
        } else {
            std::fs::create_dir_all(&backups)?;
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let name = s.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let backup = backups.join(format!("{stamp}-{name}"));
            std::fs::copy(&s.path, &backup)?;
            (s.path.clone(), Some(backup))
        };
        // Through a temporary file so a failed write never truncates the target.
        let tmp = target.with_extension("wav.tmp");
        editor::write_wav(&tmp, s.audio(), s.bit_depth)?;
        std::fs::rename(&tmp, &target)?;
        if backup.is_some() {
            s.mark_saved();
        }
        Ok(Saved {
            path: target.to_string_lossy().into_owned(),
            backup: backup.map(|b| b.to_string_lossy().into_owned()),
            state: s.state(),
        })
    })?;
    // Pick up the new or changed file right away instead of waiting for the watcher.
    let sources: Vec<(i64, String)> = {
        let conn = state.db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, path FROM sources")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        rows
    };
    let saved_path = Path::new(&saved.path);
    let source = sources
        .into_iter()
        .filter(|(_, root)| saved_path.starts_with(root))
        .max_by_key(|(_, root)| root.len())
        .map(|(id, _)| id);
    if let Some(id) = source {
        state.scanner.request(id);
    }
    Ok(saved)
}
