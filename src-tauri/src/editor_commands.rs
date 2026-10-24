//! Tauri commands for the editor. Each file opens in its own window
//! ("editor-<sample id>"); sessions are keyed by window label.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder, Window};

use crate::editor::{self, Op, Session, State as EditorState};
use crate::error::{Error, Result};
use crate::AppState;

pub const WINDOW_PREFIX: &str = "editor-";

fn with_session<T>(state: &AppState, window: &Window, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
    let mut sessions = state.editor.lock().unwrap();
    let session =
        sessions.get_mut(window.label()).ok_or_else(|| Error::Other("no file is open in this window".into()))?;
    f(session)
}

/// Opens (or brings to front) the editor window for a sample.
#[tauri::command]
pub fn open_editor_window(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<()> {
    let label = format!("{WINDOW_PREFIX}{id}");
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        window.set_focus()?;
        return Ok(());
    }
    let name: String = state.db.lock().unwrap().query_row("SELECT name FROM samples WHERE id = ?1", [id], |r| r.get(0))?;
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(format!("index.html?editor={id}").into()))
        .title(format!("{name} — Edit"))
        .inner_size(1100.0, 560.0)
        .min_inner_size(640.0, 360.0)
        .visible(false);
    // Same look as the main window: traffic lights over the app's own header.
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    builder.build()?;
    crate::show_window_fallback(&app, &label);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    state: EditorState,
    name: String,
    ext: String,
    path: String,
    bpm: Option<f64>,
}

/// Loads the sample into this window's session.
#[tauri::command]
pub async fn editor_open(state: State<'_, AppState>, window: Window, id: i64) -> Result<Opened> {
    let (path, name, ext, bit_depth, bpm): (String, String, String, Option<u32>, Option<f64>) =
        state.db.lock().unwrap().query_row(
            "SELECT path, name, ext, bit_depth, bpm FROM samples WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    let session = Session::open(id, Path::new(&path), bit_depth)?;
    let s = session.state();
    state.editor.lock().unwrap().insert(window.label().to_string(), session);
    Ok(Opened { state: s, name, ext, path, bpm })
}

/// Frees a window's session (also done when the window is destroyed).
pub fn release(app: &AppHandle, label: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.editor.lock().unwrap().remove(label);
    }
}

#[tauri::command]
pub fn editor_apply(state: State<'_, AppState>, window: Window, op: Op) -> Result<EditorState> {
    with_session(&state, &window, |s| {
        s.apply(op);
        Ok(s.state())
    })
}

#[tauri::command]
pub fn editor_undo(state: State<'_, AppState>, window: Window) -> Result<EditorState> {
    with_session(&state, &window, |s| {
        s.undo();
        Ok(s.state())
    })
}

#[tauri::command]
pub fn editor_redo(state: State<'_, AppState>, window: Window) -> Result<EditorState> {
    with_session(&state, &window, |s| {
        s.redo();
        Ok(s.state())
    })
}

/// Waveform data for `[start, end)` (see `editor::wave`), as raw bytes.
#[tauri::command]
pub fn editor_wave(
    state: State<'_, AppState>,
    window: Window,
    start: usize,
    end: usize,
    columns: usize,
    split: bool,
    bands: bool,
) -> Result<tauri::ipc::Response> {
    with_session(&state, &window, |s| {
        Ok(tauri::ipc::Response::new(s.wave(start, end, columns.clamp(1, 8192), split, bands)))
    })
}

/// Moves each frame to the nearest zero crossing within `radius` frames.
#[tauri::command]
pub fn editor_zero_snap(state: State<'_, AppState>, window: Window, frames: Vec<usize>, radius: usize) -> Result<Vec<usize>> {
    let radius = radius.min(48_000);
    with_session(&state, &window, |s| Ok(frames.iter().map(|&f| editor::zero_crossing(s.audio(), f, radius)).collect()))
}

#[tauri::command]
pub fn editor_play(state: State<'_, AppState>, window: Window, start: usize, end: usize, looping: bool) -> Result<()> {
    with_session(&state, &window, |s| state.player.play_buffer(s.audio(), start, end, looping))
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
pub fn editor_save(state: State<'_, AppState>, window: Window, as_new: bool) -> Result<Saved> {
    let backups = state.db_path.parent().map(|d| d.join("backups")).unwrap_or_else(|| PathBuf::from("backups"));
    let saved = with_session(&state, &window, |s| {
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
        editor::write_wav(&tmp, s.audio(), s.bit_depth, s.loop_points())?;
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
    rescan_containing(&state, Path::new(&saved.path))?;
    Ok(saved)
}

/// Rescans the library folder that holds `path` right away instead of waiting for
/// the watcher. False when no library folder holds it.
fn rescan_containing(state: &AppState, path: &Path) -> Result<bool> {
    let sources: Vec<(i64, String)> = {
        let conn = state.db.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, path FROM sources")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        rows
    };
    let source = sources
        .into_iter()
        .filter(|(_, root)| path.starts_with(root))
        .max_by_key(|(_, root)| root.len())
        .map(|(id, _)| id);
    if let Some(id) = source {
        state.scanner.request(id);
    }
    Ok(source.is_some())
}

/// Slices of `[start, end)` of this window's audio (see `editor::slices`).
#[tauri::command]
pub fn editor_slices(
    state: State<'_, AppState>,
    window: Window,
    start: usize,
    end: usize,
    by: editor::SliceBy,
    zero: bool,
) -> Result<Vec<editor::Span>> {
    with_session(&state, &window, |s| Ok(editor::slices(s.audio(), start, end, by, zero)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedSlices {
    dir: String,
    count: usize,
    /// Whether the folder is inside the library, so the slices show up there.
    in_library: bool,
}

/// Writes each slice as a WAV ("Kick 01.wav", …) into a new folder next to the
/// file ("Kick slices"), at the file's bit depth, and adds them to the library.
#[tauri::command]
pub fn editor_export_slices(state: State<'_, AppState>, window: Window, spans: Vec<editor::Span>) -> Result<ExportedSlices> {
    if spans.is_empty() {
        return Err(Error::Other("nothing to export".into()));
    }
    let dir = with_session(&state, &window, |s| {
        let parent = s.path.parent().unwrap_or(Path::new("."));
        let stem = s.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let dir = (1..)
            .map(|n| parent.join(if n == 1 { format!("{stem} slices") } else { format!("{stem} slices {n}") }))
            .find(|d| !d.exists())
            .expect("some name is free");
        std::fs::create_dir_all(&dir)?;
        let audio = s.audio();
        let ch = audio.channels.max(1);
        let width = spans.len().to_string().len().max(2);
        for (i, span) in spans.iter().enumerate() {
            let (a, b) = (span.start.min(audio.frames()), span.end.min(audio.frames()));
            let pcm = crate::audio::Pcm {
                samples: audio.samples[a * ch..b.max(a) * ch].to_vec(),
                channels: audio.channels,
                sample_rate: audio.sample_rate,
            };
            editor::write_wav(&dir.join(format!("{stem} {:0width$}.wav", i + 1)), &pcm, s.bit_depth, None)?;
        }
        Ok(dir)
    })?;
    let in_library = rescan_containing(&state, &dir)?;
    Ok(ExportedSlices { dir: dir.to_string_lossy().into_owned(), count: spans.len(), in_library })
}
