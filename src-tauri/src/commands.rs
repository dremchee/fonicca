use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::duplicates;
use crate::error::{Error, Result};
use crate::library::{self, Collection, Filter, ImportReport, Sample, Tag, TagRef};
use crate::settings::{self, Settings, Theme};
use crate::similar::Ranking;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    id: i64,
    path: String,
    sample_count: i64,
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
pub fn source_folders(state: State<'_, AppState>, source_id: i64) -> Result<Vec<library::SourceFolder>> {
    library::source_folders(&state.db.lock().unwrap(), source_id)
}

#[tauri::command]
pub fn add_source(state: State<'_, AppState>, path: String) -> Result<i64> {
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
    if let Err(e) = state.watcher.watch(id, std::path::Path::new(&path)) {
        eprintln!("cannot watch {path}: {e}");
    }
    state.scanner.request(id);
    Ok(id)
}

#[tauri::command]
pub fn remove_source(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db.lock().unwrap().execute("DELETE FROM sources WHERE id = ?1", [id])?;
    state.watcher.unwatch(id);
    Ok(())
}

#[tauri::command]
pub fn rescan_source(state: State<'_, AppState>, id: i64) {
    state.scanner.request(id);
}

/// Similarity ranking for `filter.similar_to`, embedding the sample on demand.
fn ranking_for(state: &AppState, filter: &Filter) -> Result<Option<Arc<Ranking>>> {
    let Some(id) = filter.similar_to else { return Ok(None) };
    let conn = state.db.lock().unwrap();
    let mut similar = state.similar.lock().unwrap();
    if let Some(r) = similar.search(&conn, id)? {
        return Ok(Some(r));
    }
    if !state.analyzer.model_ready() {
        return Err(Error::Other("Download the sound model to find similar sounds".into()));
    }
    if state.analyzer.embed_now(&conn, id)? {
        if let Some(r) = similar.search(&conn, id)? {
            return Ok(Some(r));
        }
    }
    Err(Error::Other("This sample could not be analyzed".into()))
}

#[tauri::command]
pub fn count_samples(state: State<'_, AppState>, filter: Filter) -> Result<i64> {
    let ranking = ranking_for(&state, &filter)?;
    library::count_ranked(&state.db.lock().unwrap(), &filter, ranking.as_deref())
}

#[tauri::command]
pub fn list_samples(
    state: State<'_, AppState>,
    filter: Filter,
    offset: i64,
    limit: i64,
) -> Result<Vec<Sample>> {
    let ranking = ranking_for(&state, &filter)?;
    library::list_ranked(&state.db.lock().unwrap(), &filter, ranking.as_deref(), offset, limit)
}

#[tauri::command]
pub fn sample_ids(state: State<'_, AppState>, filter: Filter) -> Result<Vec<i64>> {
    let ranking = ranking_for(&state, &filter)?;
    library::ids_ranked(&state.db.lock().unwrap(), &filter, ranking.as_deref())
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

#[tauri::command]
pub fn list_collections(state: State<'_, AppState>) -> Result<Vec<Collection>> {
    library::list_collections(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn create_collection(state: State<'_, AppState>, name: String, ids: Vec<i64>) -> Result<i64> {
    let mut conn = state.db.lock().unwrap();
    let id = library::create_collection(&conn, &name)?;
    library::add_to_collection(&mut conn, &ids, id)?;
    Ok(id)
}

#[tauri::command]
pub fn rename_collection(state: State<'_, AppState>, id: i64, name: String) -> Result<()> {
    library::rename_collection(&state.db.lock().unwrap(), id, &name)
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, id: i64) -> Result<()> {
    library::delete_collection(&state.db.lock().unwrap(), id)
}

#[tauri::command]
pub fn add_to_collection(state: State<'_, AppState>, ids: Vec<i64>, collection_id: i64) -> Result<()> {
    library::add_to_collection(&mut state.db.lock().unwrap(), &ids, collection_id)
}

#[tauri::command]
pub fn remove_from_collection(state: State<'_, AppState>, ids: Vec<i64>, collection_id: i64) -> Result<()> {
    library::remove_from_collection(&mut state.db.lock().unwrap(), &ids, collection_id)
}

/// Writes favorites, tags and collections to a JSON file. Returns the number of samples.
#[tauri::command]
pub fn export_user_data(state: State<'_, AppState>, path: String) -> Result<usize> {
    let data = library::export_user_data(&state.db.lock().unwrap())?;
    let json = serde_json::to_vec_pretty(&data).map_err(|e| Error::Other(e.to_string()))?;
    std::fs::write(&path, json)?;
    Ok(data.samples.len())
}

#[tauri::command]
pub fn import_user_data(state: State<'_, AppState>, path: String) -> Result<ImportReport> {
    let bytes = std::fs::read(&path)?;
    let data: library::UserData =
        serde_json::from_slice(&bytes).map_err(|e| Error::Other(format!("invalid export file: {e}")))?;
    library::import_user_data(&mut state.db.lock().unwrap(), &data)
}

fn sample_path(state: &AppState, id: i64) -> Result<String> {
    Ok(state.db.lock().unwrap().query_row("SELECT path FROM samples WHERE id = ?1", [id], |r| r.get(0))?)
}

/// Title, artist, album, year and genre from the file's own tags; `None` for the many
/// samples that have none.
#[tauri::command]
pub async fn sample_tags(state: State<'_, AppState>, id: i64) -> Result<Option<crate::tags::TrackTags>> {
    let path = sample_path(&state, id)?;
    tauri::async_runtime::spawn_blocking(move || crate::tags::read(std::path::Path::new(&path)))
        .await
        .map_err(|e| Error::Other(e.to_string()))
}

/// Writes tags (and the cover) into the file itself and returns them as read back.
/// The library row keeps its id: a rescan matches the file by path.
#[tauri::command]
pub async fn update_sample_tags(
    state: State<'_, AppState>,
    id: i64,
    edit: crate::tags::TagEdit,
) -> Result<Option<crate::tags::TrackTags>> {
    let path = sample_path(&state, id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = std::path::Path::new(&path);
        crate::tags::write(path, edit)?;
        Ok(crate::tags::read(path))
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

/// Bytes of an image picked as a new cover, for its preview; errors on non-images.
#[tauri::command]
pub async fn cover_preview(path: String) -> Result<tauri::ipc::Response> {
    let pic = tauri::async_runtime::spawn_blocking(move || crate::tags::image_file(std::path::Path::new(&path)))
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
    Ok(tauri::ipc::Response::new(pic.into_data()))
}

/// Embedded cover art as image bytes; empty when the file has none.
#[tauri::command]
pub async fn sample_cover(state: State<'_, AppState>, id: i64) -> Result<tauri::ipc::Response> {
    let path = sample_path(&state, id)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || crate::tags::cover(std::path::Path::new(&path)))
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    Ok(tauri::ipc::Response::new(bytes.unwrap_or_default()))
}

#[tauri::command]
pub async fn sample_wave(
    state: State<'_, AppState>,
    id: i64,
    columns: usize,
    split: bool,
) -> Result<tauri::ipc::Response> {
    let (path, size, mtime, frames): (String, i64, i64, Option<f64>) = state.db.lock().unwrap().query_row(
        "SELECT path, size, mtime, duration * sample_rate FROM samples WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let key = (id, size, mtime);
    let overview = match state.overviews.get(key) {
        Some(o) => o,
        None => {
            // From the disk cache when it has it, otherwise decoded and stored there.
            let disk = state.overviews.disk();
            let o = tauri::async_runtime::spawn_blocking(move || {
                if let Some(o) = disk.as_ref().and_then(|d| d.load(key)) {
                    return Ok(o);
                }
                let o = crate::overview::Overview::compute(std::path::Path::new(&path), frames.map(|f| f as usize))?;
                if let Some(d) = &disk {
                    d.store(key, &o);
                }
                Ok::<_, Error>(o)
            })
            .await
            .map_err(|e| Error::Other(e.to_string()))??;
            let o = Arc::new(o);
            state.overviews.put(key, Arc::clone(&o));
            o
        }
    };
    Ok(tauri::ipc::Response::new(overview.render(columns.clamp(1, 8192), split)))
}

#[tauri::command]
pub fn prioritize_analysis(state: State<'_, AppState>, ids: Vec<i64>) {
    state.analyzer.prioritize(&ids);
}

#[tauri::command]
pub fn set_analysis_paused(state: State<'_, AppState>, paused: bool) {
    state.analyzer.set_paused(paused);
}

#[tauri::command]
pub fn analysis_status(state: State<'_, AppState>) -> Result<crate::analyzer::Progress> {
    let conn = state.db.lock().unwrap();
    Ok(state.analyzer.progress(&conn, Vec::new()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundModelStatus {
    installed: bool,
    ready: bool,
    downloading: bool,
    size: u64,
}

#[tauri::command]
pub fn sound_model_status(state: State<'_, AppState>) -> SoundModelStatus {
    SoundModelStatus {
        installed: crate::models::is_installed(&state.models_dir),
        ready: state.analyzer.model_ready(),
        downloading: state.model_downloading.load(std::sync::atomic::Ordering::SeqCst),
        size: crate::clap::MODEL_SIZE,
    }
}

#[tauri::command]
pub fn model_storage(state: State<'_, AppState>) -> Vec<crate::models::Storage> {
    crate::models::storage(
        &state.models_dir,
        state.model_downloading.load(std::sync::atomic::Ordering::SeqCst),
        |model| state.stems.downloading(model),
    )
}

/// Downloads the sound model in the background; progress and completion are
/// reported through `model-download` events.
#[tauri::command]
pub fn download_sound_model(app: tauri::AppHandle, state: State<'_, AppState>) {
    use std::sync::atomic::Ordering;
    use tauri::{Emitter, Manager};
    if state.model_downloading.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let emit = |p: crate::models::DownloadProgress| {
            let _ = app.emit(crate::models::DOWNLOAD_EVENT, p);
        };
        let result = crate::models::download(&state.models_dir, &emit)
            .and_then(|path| state.analyzer.load_model(&path));
        state.model_downloading.store(false, Ordering::SeqCst);
        emit(crate::models::DownloadProgress {
            received: 0,
            total: 0,
            done: true,
            error: result.err().map(|e| e.to_string()),
        });
    });
}

#[tauri::command]
/// Async so rendering a stretched loop happens off the main (UI) thread.
pub async fn play(
    state: State<'_, AppState>,
    id: i64,
    start: Option<f64>,
    sync: Option<crate::sync::Target>,
) -> Result<crate::sync::Adjust> {
    let (path, kind, bpm, key): (String, Option<String>, Option<f64>, Option<String>) =
        state.db.lock().unwrap().query_row(
            "SELECT path, kind, bpm, musical_key FROM samples WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    let adjust = match &sync {
        Some(target) => crate::sync::plan(kind.as_deref(), bpm, key.as_deref(), target),
        None => crate::sync::Adjust::NONE,
    };
    state.player.play(std::path::Path::new(&path), start.unwrap_or(0.0), adjust)
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) {
    state.player.stop();
}

#[tauri::command]
pub fn pause(state: State<'_, AppState>) -> Option<f64> {
    state.player.pause()
}

#[tauri::command]
pub fn resume(state: State<'_, AppState>) -> bool {
    state.player.resume()
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position: f64) -> Result<()> {
    state.player.seek(position)
}

/// Peak and RMS per channel of the sound playing, for the level meter.
#[tauri::command]
pub fn playback_levels(state: State<'_, AppState>) -> Option<crate::meter::Levels> {
    state.player.levels()
}

#[tauri::command]
pub fn playback_position(state: State<'_, AppState>) -> Option<f64> {
    state.player.position()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

/// Sent to every window with the new settings after they change.
pub const SETTINGS_EVENT: &str = "settings-changed";

/// Saves the full settings object and applies what changed. Returns what was stored
/// (out-of-range values clamped).
#[tauri::command]
pub fn update_settings(app: tauri::AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<Settings> {
    let (prev, next) = state.settings.replace(settings)?;
    apply_settings(&app, &state, Some(&prev), &next)?;
    // Editor windows keep their own copy of the settings.
    let _ = tauri::Emitter::emit(&app, SETTINGS_EVENT, &next);
    Ok(next)
}

/// Pushes settings into the running app; `prev: None` applies everything (startup).
pub fn apply_settings(app: &tauri::AppHandle, state: &AppState, prev: Option<&Settings>, next: &Settings) -> Result<()> {
    if prev.is_none_or(|p| p.theme != next.theme) {
        // The webview's prefers-color-scheme follows the window appearance.
        app.set_theme(match next.theme {
            Theme::System => None,
            Theme::Light => Some(tauri::Theme::Light),
            Theme::Dark => Some(tauri::Theme::Dark),
        });
    }
    if prev.is_none_or(|p| p.watch_folders != next.watch_folders) {
        state.watcher.set_enabled(next.watch_folders);
    }
    if prev.is_none_or(|p| p.volume != next.volume) {
        state.player.set_volume(next.volume);
    }
    if prev.is_none_or(|p| p.analysis_threads != next.analysis_threads) {
        state.analyzer.set_threads(next.analysis_threads.unwrap_or_else(settings::default_threads))?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    cores: usize,
    default_threads: usize,
    db_path: String,
    db_size: u64,
    models_dir: String,
    models_size: u64,
    settings_path: String,
}

/// Size of a file or of every file directly in a folder; 0 when missing.
fn disk_size(path: &std::path::Path) -> u64 {
    match std::fs::read_dir(path) {
        Ok(entries) => entries.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum(),
        Err(_) => std::fs::metadata(path).map_or(0, |m| m.len()),
    }
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, state: State<'_, AppState>) -> AppInfo {
    let db_path = &state.db_path;
    // WAL mode: recent writes live in the -wal file until a checkpoint.
    let wal = db_path.with_extension("db-wal");
    AppInfo {
        version: app.package_info().version.to_string(),
        cores: settings::cores(),
        default_threads: settings::default_threads(),
        db_path: db_path.to_string_lossy().into_owned(),
        db_size: disk_size(db_path) + disk_size(&wal),
        models_dir: state.models_dir.to_string_lossy().into_owned(),
        models_size: crate::models::disk_size(&state.models_dir),
        settings_path: state.settings.path().to_string_lossy().into_owned(),
    }
}

/// Unloads and deletes the sound model. Embeddings and categories found with it stay.
#[tauri::command]
pub fn delete_sound_model(state: State<'_, AppState>) -> Result<()> {
    if state.model_downloading.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(Error::Other("the sound model is downloading".into()));
    }
    state.analyzer.unload_model();
    let path = crate::models::model_path(&state.models_dir);
    match std::fs::remove_file(&path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

#[tauri::command]
pub fn duplicate_count(state: State<'_, AppState>) -> Result<i64> {
    duplicates::count(&state.db.lock().unwrap())
}

/// On its own connection: a few thousand rows, and the list stays responsive meanwhile.
#[tauri::command]
pub async fn duplicate_groups(state: State<'_, AppState>) -> Result<Vec<duplicates::Group>> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || duplicates::groups(&crate::db::open(&db_path)?))
        .await
        .map_err(|e| Error::Other(e.to_string()))?
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyDone {
    hashed: usize,
    error: Option<String>,
}

/// Hashes large duplicate candidates in the background; reports through `duplicates-verified`.
#[tauri::command]
pub fn verify_duplicates(app: tauri::AppHandle, state: State<'_, AppState>) {
    use std::sync::atomic::Ordering;
    use tauri::{Emitter, Manager};
    if state.duplicates_verifying.swap(true, Ordering::SeqCst) {
        return;
    }
    let db_path = state.db_path.clone();
    std::thread::spawn(move || {
        let result = duplicates::verify(&db_path);
        app.state::<AppState>().duplicates_verifying.store(false, Ordering::SeqCst);
        let (hashed, error) = match result {
            Ok(n) => (n, None),
            Err(e) => (0, Some(e.to_string())),
        };
        let _ = app.emit(duplicates::VERIFY_EVENT, VerifyDone { hashed, error });
    });
}

/// Moving thousands of files to the Trash takes a while: own connection, off the main thread.
#[tauri::command]
pub async fn resolve_duplicates(
    state: State<'_, AppState>,
    resolutions: Vec<duplicates::Resolution>,
    action: duplicates::Action,
) -> Result<duplicates::ResolveReport> {
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        duplicates::resolve(&mut crate::db::open(&db_path)?, &resolutions, action)
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

#[tauri::command]
pub fn list_hidden(state: State<'_, AppState>) -> Result<Vec<duplicates::Hidden>> {
    duplicates::list_hidden(&state.db.lock().unwrap())
}

/// Unhides the paths (all with `None`) and rescans their sources to bring the files back.
#[tauri::command]
pub fn unhide_samples(state: State<'_, AppState>, paths: Option<Vec<String>>) -> Result<()> {
    let sources = duplicates::unhide(&state.db.lock().unwrap(), paths.as_deref())?;
    for id in sources {
        state.scanner.request(id);
    }
    Ok(())
}

#[tauri::command]
pub fn preferred_folders(state: State<'_, AppState>) -> Result<Vec<String>> {
    duplicates::preferred_folders(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn set_preferred_folder(state: State<'_, AppState>, path: String, preferred: bool) -> Result<()> {
    duplicates::set_preferred_folder(&state.db.lock().unwrap(), &path, preferred)
}

/// Saves a theme to a file for sharing.
#[tauri::command]
pub fn export_theme(path: String, theme: settings::CustomTheme) -> Result<()> {
    settings::write_theme_file(std::path::Path::new(&path), &theme)
}

/// Reads a shared theme file; the theme comes back with a placeholder id.
#[tauri::command]
pub fn import_theme(path: String) -> Result<settings::CustomTheme> {
    settings::read_theme_file(std::path::Path::new(&path))
}
