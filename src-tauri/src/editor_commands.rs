//! Tauri commands for embedded and standalone editor sessions, keyed by window label.
use crate::audio::AudioRead;

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, Window};

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

/// Snapshot edited audio for transcription; inference never holds the editor lock.
pub(crate) fn transcription_audio(state: &AppState, window: &Window, range: editor::Span) -> Result<crate::audio::Pcm> {
    with_session(state, window, |session| {
        let pcm = session.audio();
        if range.start >= range.end || range.end > pcm.frames()
            || (range.end-range.start) as f64 / pcm.sample_rate as f64 > 600.0 {
            return Err(Error::Other("select a non-empty fragment up to 10 minutes".into()));
        }
        Ok(crate::audio::Pcm { samples: pcm.range(range.start * pcm.channels..range.end * pcm.channels).into_owned(),
            channels: pcm.channels, sample_rate: pcm.sample_rate })
    })
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
pub async fn editor_open(state: State<'_, AppState>, window: Window, id: i64, discard_existing: Option<bool>) -> Result<Opened> {
    // Keep the current session intact until the replacement has decoded successfully.
    if !discard_existing.unwrap_or(false) && state.editor.lock().unwrap().get(window.label()).is_some_and(|s| s.state().dirty) {
        return Err(Error::Other("editor has unsaved changes".into()));
    }
    let (path, name, ext, bit_depth, bpm): (String, String, String, Option<u32>, Option<f64>) =
        state.db.lock().unwrap().query_row(
            "SELECT path, name, ext, bit_depth, bpm FROM samples WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    let decode_path = path.clone();
    let session = tauri::async_runtime::spawn_blocking(move || Session::open(id, Path::new(&decode_path), bit_depth))
        .await.map_err(|e| Error::Other(e.to_string()))??;
    let s = session.state();
    let mut sessions = state.editor.lock().unwrap();
    if !discard_existing.unwrap_or(false) && sessions.get(window.label()).is_some_and(|s| s.state().dirty) {
        return Err(Error::Other("editor has unsaved changes".into()));
    }
    sessions.insert(window.label().to_string(), session);
    Ok(Opened { state: s, name, ext, path, bpm })
}

/// Close an embedded session only after its edits have been saved or discarded.
#[tauri::command]
pub fn editor_close(state: State<'_, AppState>, window: Window, discard: bool) -> Result<()> {
    let mut sessions = state.editor.lock().unwrap();
    if !discard && sessions.get(window.label()).is_some_and(|s| s.state().dirty) {
        return Err(Error::Other("editor has unsaved changes".into()));
    }
    sessions.remove(window.label());
    Ok(())
}

/// Frees a window's session (also done when the window is destroyed).
pub fn release(app: &AppHandle, label: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.editor.lock().unwrap().remove(label);
    }
}

#[tauri::command]
pub async fn editor_apply(state: State<'_, AppState>, window: Window, op: Op) -> Result<EditorState> {
    let (mut audio, revision) = with_session(&state, &window, |s| Ok((s.audio().clone(), s.revision())))?;
    let (audio, op) = tauri::async_runtime::spawn_blocking(move || -> Result<_> {
        audio.try_apply(&op)?;
        Ok((audio, op))
    }).await.map_err(|e| Error::Other(format!("audio processing failed: {e}")))??;
    with_session(&state, &window, |s| {
        s.apply_rendered(revision, op, audio)?;
        Ok(s.state())
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Processed {
    state: EditorState,
    start: usize,
    end: usize,
    loudness: Option<editor::processing::LoudnessReport>,
}

/// Snapshot under the lock; copy, process and render the output on the worker.
#[tauri::command]
pub async fn editor_process(
    state: State<'_, AppState>, window: Window, start: usize, end: usize,
    processing: editor::processing::Processing,
) -> Result<Processed> {
    let snapshot = with_session(&state, &window, |s| editor::processing::Snapshot::new(s, start, end))?;
    let output = tauri::async_runtime::spawn_blocking(move || snapshot.render(processing))
        .await.map_err(|e| Error::Other(format!("audio processing failed: {e}")))??;
    with_session(&state, &window, |s| {
        let editor::processing::RenderedSnapshot { audio, revision, rendered } = output;
        s.apply_rendered(revision, rendered.op, audio)?;
        Ok(Processed { state: s.state(), start, end: rendered.end, loudness: rendered.loudness })
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
pub async fn editor_wave(
    state: State<'_, AppState>, window: Window,
    start: usize, end: usize, columns: usize, split: bool, bands: bool,
) -> Result<tauri::ipc::Response> {
    let (audio, revision) = with_session(&state, &window, |s| Ok((s.audio().clone(), s.audio_revision())))?;
    let (bytes, elapsed) = tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let bytes = audio.wave(start, end, columns.clamp(1, 16384), split, bands);
        (bytes, started.elapsed().as_micros() as u64)
    }).await.map_err(|e| Error::Other(format!("waveform generation failed: {e}")))?;
    if let Ok(sessions) = state.editor.try_lock() {
        if let Some(session) = sessions.get(window.label()) { session.record_wave_time(revision, elapsed); }
    }
    // A stale request is harmless: the frontend compares its revision before
    // displaying it, and unchanged source caches remain useful in undo history.
    Ok(tauri::ipc::Response::new(bytes))
}

/// Build the analysis once per audio revision without holding the session lock.
#[tauri::command]
pub async fn editor_spectrogram(
    state: State<'_, AppState>, window: Window, start: usize, end: usize, columns: usize,
) -> Result<tauri::ipc::Response> {
    let (cached, pcm, revision) = with_session(&state, &window, |s| {
        let cached = s.spectrogram();
        let pcm = if cached.is_none() {
            let audio = s.audio();
            Some(audio.clone())
        } else { None };
        Ok((cached, pcm, s.audio_revision()))
    })?;
    let (spectrum, bytes) = tauri::async_runtime::spawn_blocking(move || -> Result<_> {
        let spectrum = match cached {
            Some(cached) => cached,
            None => std::sync::Arc::new(editor::spectrogram::Spectrogram::build(&pcm.unwrap())?),
        };
        let bytes = spectrum.range(start, end, columns);
        Ok((spectrum, bytes))
    }).await.map_err(|e| Error::Other(format!("spectrogram analysis failed: {e}")))??;
    with_session(&state, &window, |s| s.cache_spectrogram(revision, spectrum))?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Copies `[start, end)` to the clipboard shared by all editor windows.
#[tauri::command]
pub async fn editor_copy(state: State<'_, AppState>, window: Window, start: usize, end: usize) -> Result<Option<editor::clipboard::ClipInfo>> {
    let audio = with_session(&state, &window, |s| Ok(s.audio().clone()))?;
    tauri::async_runtime::spawn_blocking(move || {
        let frames = end.min(audio.frames()).saturating_sub(start.min(audio.frames()));
        editor::memory::check_samples(frames.saturating_mul(audio.channels), editor::memory::pcm_budget())?;
        Ok(editor::clipboard::copy(&audio, start, end).then(editor::clipboard::info).flatten())
    }).await.map_err(|e| Error::Other(format!("copying audio failed: {e}")))?
}

#[tauri::command]
pub fn editor_clipboard() -> Option<editor::clipboard::ClipInfo> {
    editor::clipboard::info()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pasted {
    state: EditorState,
    /// Where the pasted audio is now.
    start: usize,
    end: usize,
}

/// Replaces `[start, end)` with the clipboard (inserts when they are equal), in
/// this file's channel count and sample rate. One edit, undone as one.
#[tauri::command]
pub async fn editor_paste(state: State<'_, AppState>, window: Window, start: usize, end: usize) -> Result<Pasted> {
    let snapshot = with_session(&state, &window, |s| {
        let clip = editor::clipboard::snapshot()
            .ok_or_else(|| Error::Other("the clipboard is empty".into()))?;
        Ok(editor::paste::Snapshot::new(s, clip, start, end))
    })?;
    let output = tauri::async_runtime::spawn_blocking(move || snapshot.render())
        .await.map_err(|e| Error::Other(format!("pasting audio failed: {e}")))??;
    with_session(&state, &window, |s| {
        let editor::paste::Rendered { audio, op, revision, start, end } = output;
        s.apply_rendered(revision, op, audio)?;
        Ok(Pasted { state: s.state(), start, end })
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

#[tauri::command]
pub fn editor_update_playback(state: State<'_, AppState>, window: Window, start: usize, end: usize, looping: bool) -> Result<bool> {
    with_session(&state, &window, |s| state.player.update_buffer_range(s.audio(), start, end, looping))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    path: String,
    backup: Option<String>,
    state: EditorState,
    name: String,
    ext: String,
    bpm: Option<f64>,
    in_library: bool,
    warning: Option<String>,
    file_id: Option<i64>,
}

/// Saves the current buffer, or exports a selection without changing the session.
#[tauri::command]
pub fn editor_save(state: State<'_, AppState>, window: Window, mode: editor::saving::SaveMode, destination: Option<String>, range: Option<editor::Span>) -> Result<Saved> {
    let backups = state.db_path.parent().map(|d| d.join("backups")).unwrap_or_else(|| PathBuf::from("backups"));
    let saved = with_session(&state, &window, |s| {
        let output = editor::saving::save_session(s, &backups, mode, destination.as_deref().map(Path::new), range)?;
        // Index just the saved file, rather than rescanning a potentially large source.
        let indexed = index_saved(&state.db.lock().unwrap(), &output.path);
        let (id, warning) = match indexed { Ok(id) => (id, None), Err(error) => (None, Some(error.to_string())) };
        if mode != editor::saving::SaveMode::Selection {
            if let Some(id) = id { s.sample_id = id; }
        }
        let bpm = state.db.lock().unwrap().query_row("SELECT bpm FROM samples WHERE id=?1", [s.sample_id], |r| r.get(0)).ok().flatten();
        Ok(Saved {
            path: output.path.to_string_lossy().into_owned(),
            backup: output.backup.map(|b| b.to_string_lossy().into_owned()),
            state: s.state(),
            name: output.path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
            ext: "wav".into(), bpm, in_library: id.is_some(), warning, file_id: id,
        })
    })?;
    if let Some(id) = saved.file_id { let _ = window.emit("editor:saved", id); }
    Ok(saved)
}

/// Import/update only files under a library source. Save As outside the library
/// never adds a whole unrelated folder to the user's sources.
fn index_saved(conn: &rusqlite::Connection, path: &Path) -> Result<Option<i64>> {
    let mut stmt = conn.prepare("SELECT id,path FROM sources")?;
    let sources = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let Some((source, _)) = sources.into_iter().filter(|(_, root)| path.starts_with(root)).max_by_key(|(_, root)| root.len()) else { return Ok(None) };
    let info = crate::audio::probe(path)?;
    let meta = std::fs::metadata(path)?;
    let mtime = meta.modified()?.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let fingerprint = crate::audio::fingerprint(path, meta.len()).ok();
    conn.execute("INSERT INTO samples(source_id,path,name,ext,size,mtime,duration,sample_rate,channels,bit_depth,fingerprint)
        VALUES(?1,?2,?3,'wav',?4,?5,?6,?7,?8,?9,?10)
        ON CONFLICT(path) DO UPDATE SET size=excluded.size,mtime=excluded.mtime,duration=excluded.duration,
        sample_rate=excluded.sample_rate,channels=excluded.channels,bit_depth=excluded.bit_depth,fingerprint=excluded.fingerprint,
        peaks=NULL,analysis_version=NULL,embedding_version=NULL,content_hash=NULL",
        rusqlite::params![source,path.to_string_lossy(),path.file_stem().unwrap_or_default().to_string_lossy(),meta.len() as i64,mtime,
            info.duration,info.sample_rate,info.channels,info.bit_depth,fingerprint])?;
    Ok(Some(conn.query_row("SELECT id FROM samples WHERE path=?1", [path.to_string_lossy().as_ref()], |r| r.get(0))?))
}

#[cfg(test)]
mod save_index_tests {
    use super::*;
    #[test]
    fn saved_files_get_a_stable_library_id_and_overwrites_invalidate_cached_audio() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open(&dir.path().join("library.db")).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO sources(path,added_at) VALUES(?1,0)", [dir.path().to_str().unwrap()]).unwrap();
        let path = dir.path().join("Saved.wav");
        editor::write_wav(&path, &crate::audio::Pcm { samples: vec![0.25; 100], channels: 1, sample_rate: 1000 }, Some(16), None).unwrap();
        let id = index_saved(&conn, &path).unwrap().unwrap();
        conn.execute("UPDATE samples SET peaks=X'01', analysis_version=1 WHERE id=?1", [id]).unwrap();
        assert_eq!(index_saved(&conn, &path).unwrap(), Some(id));
        let count: i64 = conn.query_row("SELECT count(*) FROM samples", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
        assert!(conn.query_row("SELECT peaks IS NULL AND analysis_version IS NULL FROM samples WHERE id=?1", [id], |r| r.get::<_, bool>(0)).unwrap());
        let outside = tempfile::tempdir().unwrap();
        assert_eq!(index_saved(&conn, &outside.path().join("Elsewhere.wav")).unwrap(), None);
        assert_eq!(conn.query_row("SELECT count(*) FROM sources", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }
}

/// Rescans the library folder that holds `path` right away instead of waiting for
/// the watcher. False when no library folder holds it.
pub(crate) fn rescan_containing(state: &AppState, path: &Path) -> Result<bool> {
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
        let width = spans.len().to_string().len().max(2);
        for (i, span) in spans.iter().enumerate() {
            let (a, b) = (span.start.min(audio.frames()), span.end.min(audio.frames()));
            let pcm = audio.slice_frames(a,b.max(a));
            editor::write_wav(&dir.join(format!("{stem} {:0width$}.wav", i + 1)), &pcm, s.bit_depth, None)?;
        }
        Ok(dir)
    })?;
    let in_library = rescan_containing(&state, &dir)?;
    Ok(ExportedSlices { dir: dir.to_string_lossy().into_owned(), count: spans.len(), in_library })
}
