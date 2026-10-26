//! One cancellable local separation job. Results are published only when all
//! stems have been written; a failed or cancelled job leaves the source intact.
mod demucs;
mod dsp;
pub(crate) mod models;
mod scnet;
use crate::{
    AppState,
    audio::{self, Pcm},
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, Emitter, Manager, State};
fn failure(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("stem separation: {e}"))
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(failure("cancelled"))
    } else {
        Ok(())
    }
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Model {
    #[default]
    Spleeter,
    ScnetLarge,
    ScnetXlIhf,
    Htdemucs,
}
impl Model {
    pub const ALL: [Self; 4] = [
        Self::Spleeter,
        Self::ScnetLarge,
        Self::ScnetXlIhf,
        Self::Htdemucs,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Spleeter => "spleeter",
            Self::ScnetLarge => "scnetLarge",
            Self::ScnetXlIhf => "scnetXlIhf",
            Self::Htdemucs => "htdemucs",
        }
    }
}
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub model: Model,
    /// Stable source identity, independent of the current library selection.
    pub source_id: Option<i64>,
    pub running: bool,
    pub stage: String,
    pub progress: f64,
    pub error: Option<String>,
    pub cancelled: bool,
    pub paths: Vec<String>,
    pub ids: Vec<i64>,
    pub name: String,
}
#[derive(Default)]
pub struct Jobs {
    running: AtomicBool,
    cancel: AtomicBool,
    status: Mutex<Progress>,
}
impl Jobs {
    pub fn is_running(&self) -> bool { self.running.load(Ordering::SeqCst) }

    pub fn downloading(&self, model: Model) -> bool {
        let status = self.status.lock().unwrap();
        status.running && status.stage == "downloading" && status.model == model
    }

    fn update(&self, stage: &str, progress: f64) {
        let mut s = self.status.lock().unwrap();
        s.stage = stage.into();
        s.progress = progress.clamp(0.0, 1.0);
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    progress: Progress,
    model_ready: bool,
    download_bytes: u64,
}
#[tauri::command]
pub fn stem_status(state: State<'_, AppState>, model: Option<Model>) -> Status {
    let progress = state.stems.status.lock().unwrap().clone();
    let model = if progress.running {
        progress.model
    } else {
        model.unwrap_or_default()
    };
    Status {
        progress,
        model_ready: models::installed_for(&state.models_dir, model),
        download_bytes: models::total(model),
    }
}
#[tauri::command]
pub fn stem_cancel(state: State<'_, AppState>) {
    state.stems.cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
pub fn stem_start(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    model: Option<Model>,
) -> Result<()> {
    let model = model.unwrap_or_default();
    if state.db.lock().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM sample_stems WHERE child_id=?1)", [id], |r| r.get::<_, bool>(0))? {
        return Err(failure("select the original track to separate stems"));
    }
    let path: String =
        state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT path FROM samples WHERE id=?1", [id], |r| r.get(0))?;
    let folder = state.settings.get().stems_folder.map(PathBuf::from)
        .unwrap_or_else(|| Path::new(&path).parent().unwrap_or(Path::new(".")).to_owned());
    if !folder.is_dir() {
        return Err(failure("choose an output folder"));
    }
    let model_lease = state.model_operations.acquire(model.id(), crate::models::Operation::Using)?;
    let processing_lease = state.model_operations.acquire("audio-processing", crate::models::Operation::Using)?;
    if state
        .stems
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(failure("a separation job is already running"));
    }
    state.stems.cancel.store(false, Ordering::Relaxed);
    *state.stems.status.lock().unwrap() = Progress {
        model,
        source_id: Some(id),
        running: true,
        stage: "preparing".into(),
        name: Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let (_model_lease, _processing_lease) = (model_lease, processing_lease);
        let state = app.state::<AppState>();
        let jobs = &state.stems;
        let result = run(&state, Path::new(&path), &folder, model);
        let mut status = jobs.status.lock().unwrap();
        status.running = false;
        match result {
            Ok((paths, ids)) => {
                status.paths = paths;
                status.ids = ids;
                status.stage = "done".into();
                status.progress = 1.0;
            }
            Err(e) => {
                status.cancelled = jobs.cancel.load(Ordering::Relaxed);
                status.stage = if status.cancelled {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                if !status.cancelled {
                    status.error = Some(e.to_string());
                }
            }
        }
        jobs.running.store(false, Ordering::SeqCst);
        let _ = app.emit("stems:changed", status.clone());
    });
    Ok(())
}
struct WorkingDir {
    path: PathBuf,
    published: bool,
}
impl Drop for WorkingDir {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
#[cfg(test)]
fn separate_to_folder(
    model_dir: &Path,
    source: &Path,
    parent: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
) -> Result<Vec<PathBuf>> {
    separate_to_folder_with_model(Model::Spleeter, model_dir, source, parent, cancel, progress)
}
fn separate_to_folder_with_model(
    model: Model,
    model_dir: &Path,
    source: &Path,
    parent: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
) -> Result<Vec<PathBuf>> {
    let info = audio::probe(source)?;
    if info.duration.is_some_and(|d| d > 600.0) || info.channels.is_some_and(|c| c > 2) {
        return Err(failure("use a mono or stereo file up to 10 minutes"));
    }
    let mut pcm = audio::decode(source, 600.1)?;
    if pcm.frames() == 0 || pcm.channels == 0 || pcm.samples.iter().any(|s| !s.is_finite()) {
        return Err(failure("invalid source audio"));
    }
    if pcm.frames() as f64 / pcm.sample_rate as f64 > 600.0 {
        return Err(failure("source exceeds 10 minutes"));
    }
    check(cancel)?;
    crate::batch::resample(&mut pcm, dsp::RATE)?;
    check(cancel)?;
    let stem = source
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .take(80)
        .collect::<String>();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(failure)?
        .as_nanos();
    let working_path = parent.join(format!(".fonicca-stems-{stamp}"));
    std::fs::create_dir(&working_path)?;
    let working = WorkingDir {
        path: working_path,
        published: false,
    };
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: dsp::RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writers = dsp::NAMES
        .iter()
        .map(|n| {
            hound::WavWriter::create(working.path.join(format!("{n}.wav")), spec).map_err(failure)
        })
        .collect::<Result<Vec<_>>>()?;
    if model == Model::Htdemucs {
        let mut separator = demucs::Separator::load(model_dir)?;
        separator.separate(&pcm, cancel, &progress, |stems| {
            for (writer, stem) in writers.iter_mut().zip(stems) {
                for sample in stem {
                    writer.write_sample(*sample).map_err(failure)?;
                }
            }
            Ok(())
        })?;
    } else if matches!(model, Model::ScnetLarge | Model::ScnetXlIhf) {
        let mut separator = scnet::Separator::load_for(model_dir, model)?;
        separator.separate(&pcm, cancel, &progress, |stems| {
            for (writer, stem) in writers.iter_mut().zip(stems) {
                for sample in stem {
                    writer.write_sample(*sample).map_err(failure)?;
                }
            }
            Ok(())
        })?;
    } else {
        let mut separator = dsp::Separator::load(model_dir)?;
        const BLOCK: usize = 8 * dsp::RATE as usize;
        const CONTEXT: usize = dsp::RATE as usize;
        let mut tails: Vec<Vec<f32>> = vec![Vec::new(); 4];
        for start in (0..pcm.frames()).step_by(BLOCK) {
            check(cancel)?;
            let end = (start + BLOCK).min(pcm.frames());
            let left = start.saturating_sub(CONTEXT);
            let right = (end + CONTEXT).min(pcm.frames());
            let block = Pcm {
                samples: pcm.samples[left * pcm.channels..right * pcm.channels].to_vec(),
                channels: pcm.channels,
                sample_rate: dsp::RATE,
            };
            let stems = separator.separate(&block, || check(cancel))?;
            for (i, (writer, mut samples)) in writers.iter_mut().zip(stems).enumerate() {
                let begin = (start - left) * 2;
                let finish = (end - left) * 2;
                let blend = (tails[i].len() / 2).min((end - start).min(4096));
                for frame in 0..blend {
                    let weight = frame as f32 / blend as f32;
                    for ch in 0..2 {
                        let at = frame * 2 + ch;
                        samples[begin + at] =
                            tails[i][at] * (1.0 - weight) + samples[begin + at] * weight;
                    }
                }
                for sample in &samples[begin..finish] {
                    writer.write_sample(*sample).map_err(failure)?;
                }
                tails[i] = samples[finish..((end - left + 4096).min(right - left)) * 2].to_vec();
            }
            progress(end as f64 / pcm.frames() as f64);
        }
    }
    for writer in writers {
        writer.finalize().map_err(failure)?;
    }
    check(cancel)?;
    publish_stems(&working.path, parent, &stem)
}

/// Publish one track per directory, never reusing an existing destination.
/// Repeated separations choose a new numbered directory for the complete set.
fn publish_stems(working: &Path, parent: &Path, name: &str) -> Result<Vec<PathBuf>> {
    for version in 1.. {
        let suffix = if version == 1 { String::new() } else { format!(" {version}") };
        let folder = parent.join(format!("{name}{suffix}"));
        match std::fs::create_dir(&folder) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
        let paths: Vec<_> = dsp::NAMES.iter()
            .map(|kind| folder.join(format!("{name} - {kind}.wav"))).collect();
        let mut published = Vec::new();
        let result: std::io::Result<()> = (|| {
            for (kind, target) in dsp::NAMES.iter().zip(&paths) {
                // create_new is supported on removable drives too. Never claim
                // or clean up a destination that belonged to another process.
                let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(target)?;
                published.push(target.clone());
                let mut input = std::fs::File::open(working.join(format!("{kind}.wav")))?;
                std::io::copy(&mut input, &mut output)?;
                output.sync_all()?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => return Ok(paths),
            Err(e) => {
                for path in published { let _ = std::fs::remove_file(path); }
                let _ = std::fs::remove_dir(&folder);
                return Err(e.into());
            }
        }
    }
    unreachable!()
}

fn run(
    state: &AppState,
    source: &Path,
    parent: &Path,
    model: Model,
) -> Result<(Vec<String>, Vec<i64>)> {
    let jobs = &state.stems;
    jobs.update("downloading", 0.0);
    let model_dir = models::ensure_for(&state.models_dir, model, &jobs.cancel, |received| {
        jobs.update("downloading", received as f64 / models::total(model) as f64)
    })?;
    jobs.update("separating", 0.0);
    let paths =
        separate_to_folder_with_model(model, &model_dir, source, parent, &jobs.cancel, |p| {
            jobs.update("separating", p)
        })?;
    jobs.update("importing", 1.0);
    state.stems.status.lock().unwrap().paths = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let ids = import(state, &paths)?;
    let conn = crate::db::open(&state.db_path)?;
    let parent_id: i64 = conn.query_row("SELECT id FROM samples WHERE path=?1", [source.to_string_lossy().as_ref()], |r| r.get(0))?;
    link_stems(&conn, parent_id, &ids)?;
    Ok((
        paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        ids,
    ))
}
fn link_stems(conn: &rusqlite::Connection, parent_id: i64, ids: &[i64]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (position, (id, kind)) in ids.iter().zip(dsp::NAMES).enumerate() {
        tx.execute("INSERT INTO sample_stems(child_id,parent_id,kind,position) VALUES(?1,?2,?3,?4)
          ON CONFLICT(child_id) DO UPDATE SET parent_id=excluded.parent_id,kind=excluded.kind,position=excluded.position",
          rusqlite::params![id,parent_id,kind,position as i64])?;
    }
    tx.commit()?;
    Ok(())
}
fn import_files(db_path: &Path, paths: &[PathBuf]) -> Result<(i64, Vec<i64>)> {
    let folder = paths[0]
        .parent()
        .ok_or_else(|| failure("missing output directory"))?;
    let source_id = {
        let conn = crate::db::open(db_path)?;
        let mut stmt = conn.prepare("SELECT id,path FROM sources")?;
        let sources = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if let Some((id, _)) = sources
            .into_iter()
            .filter(|(_, root)| folder.starts_with(root))
            .max_by_key(|(_, p)| p.len())
        {
            id
        } else {
            conn.execute(
                "INSERT INTO sources(path,added_at) VALUES(?1,strftime('%s','now'))",
                [folder.to_string_lossy().as_ref()],
            )?;
            let id = conn.last_insert_rowid();
            id
        }
    };
    crate::scan::scan_source(db_path, source_id, |_| {});
    let conn = crate::db::open(db_path)?;
    let mut ids = Vec::new();
    for path in paths {
        let id = conn.query_row(
            "SELECT id FROM samples WHERE path=?1",
            [path.to_string_lossy().as_ref()],
            |r| r.get(0),
        )?;
        ids.push(id);
    }
    Ok((source_id, ids))
}
fn import(state: &AppState, paths: &[PathBuf]) -> Result<Vec<i64>> {
    let (source_id, ids) = import_files(&state.db_path, paths)?;
    let root: String = state.db.lock().unwrap().query_row(
        "SELECT path FROM sources WHERE id=?1",
        [source_id],
        |r| r.get(0),
    )?;
    let _ = state.watcher.watch(source_id, Path::new(&root));
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_keeps_source_identity_across_stages() {
        let jobs = Jobs::default();
        *jobs.status.lock().unwrap() = Progress { source_id: Some(42), running: true, ..Default::default() };
        for stage in ["downloading", "separating", "importing"] {
            jobs.update(stage, 0.45);
            let status = jobs.status.lock().unwrap();
            assert_eq!(status.source_id, Some(42));
            let json = serde_json::to_value(&*status).unwrap();
            assert_eq!(json["sourceId"], 42);
        }
    }
    #[test]
    fn stems_are_grouped_in_a_track_folder_without_overwriting_an_existing_set() {
        let dir = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        for name in dsp::NAMES { std::fs::write(work.path().join(format!("{name}.wav")), name).unwrap(); }
        let existing_folder = dir.path().join("Song");
        std::fs::create_dir(&existing_folder).unwrap();
        let existing = existing_folder.join("Song - drums.wav");
        std::fs::write(&existing, "keep me").unwrap();
        let paths = publish_stems(work.path(), dir.path(), "Song").unwrap();
        assert_eq!(std::fs::read_to_string(existing).unwrap(), "keep me");
        assert!(!dir.path().join("Song - vocals.wav").exists());
        for (path, kind) in paths.iter().zip(dsp::NAMES) {
            assert_eq!(path.parent(), Some(dir.path().join("Song 2").as_path()));
            assert_eq!(path.file_name().unwrap().to_str().unwrap(), format!("Song - {kind}.wav"));
            assert_eq!(std::fs::read_to_string(path).unwrap(), kind);
        }
    }

    #[test]
    fn first_separation_creates_a_folder_named_after_the_track() {
        let dir = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        for kind in dsp::NAMES {
            std::fs::write(work.path().join(format!("{kind}.wav")), kind).unwrap();
        }
        let paths = publish_stems(work.path(), dir.path(), "John Lennon - Hold On").unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        for path in paths {
            assert_eq!(path.parent(), Some(dir.path().join("John Lennon - Hold On").as_path()));
        }
    }

    #[test]
    fn failed_publication_removes_only_its_own_partial_files() {
        let dir = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        std::fs::write(work.path().join("vocals.wav"), "vocals").unwrap();
        std::fs::write(dir.path().join("original.wav"), "source").unwrap();
        assert!(publish_stems(work.path(), dir.path(), "Song").is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_to_string(dir.path().join("original.wav")).unwrap(), "source");
    }
    #[test]
    #[ignore = "requires SCNET_MODELS and STEM_AUDIO; full file write and cancellation"]
    fn scnet_file_export_keeps_original_and_cleans_cancelled_work() {
        check_file_export(Model::ScnetLarge, "SCNET_MODELS")
    }
    #[test]
    #[ignore = "requires SCNET_XL_MODELS and STEM_AUDIO; XL full file write and cancellation"]
    fn xl_file_export_keeps_original_and_cleans_cancelled_work() {
        check_file_export(Model::ScnetXlIhf, "SCNET_XL_MODELS")
    }
    #[test]
    #[ignore = "requires DEMUCS_MODELS and STEM_AUDIO; full file integration"]
    fn htdemucs_file_export_keeps_original_and_cleans_cancelled_work() {
        check_file_export(Model::Htdemucs, "DEMUCS_MODELS")
    }
    fn check_file_export(model: Model, model_env: &str) {
        let dir = tempfile::tempdir().unwrap();
        let source = PathBuf::from(std::env::var("STEM_AUDIO").unwrap());
        let models = PathBuf::from(std::env::var(model_env).unwrap());
        let original = std::fs::read(&source).unwrap();
        let pcm = audio::decode(&source, 600.0).unwrap();
        let start = std::time::Instant::now();
        let paths = separate_to_folder_with_model(
            model,
            &models,
            &source,
            dir.path(),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(paths.len(), 4);
        for path in &paths {
            let stem = audio::decode(path, 600.0).unwrap();
            assert_eq!(stem.frames(), pcm.frames());
            assert_eq!(stem.channels, 2);
            assert_eq!(stem.sample_rate, dsp::RATE);
            assert!(stem.samples.iter().all(|v| v.is_finite()));
        }
        eprintln!(
            "{model:?} file export {}s in {:?}",
            pcm.frames() as f64 / pcm.sample_rate as f64,
            start.elapsed()
        );
        let cancel = AtomicBool::new(false);
        assert!(
            separate_to_folder_with_model(model, &models, &source, dir.path(), &cancel, |_| cancel
                .store(true, Ordering::Relaxed))
            .is_err()
        );
        assert_eq!(std::fs::read(source).unwrap(), original);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(paths[0].parent().unwrap()).unwrap().count(), 4);
    }
    #[test]
    fn cancelled_processing_does_not_create_output_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        crate::editor::write_wav(
            &source,
            &Pcm {
                samples: vec![0.2; 4410],
                channels: 1,
                sample_rate: 44100,
            },
            Some(32),
            None,
        )
        .unwrap();
        let before = std::fs::read(&source).unwrap();
        assert!(
            separate_to_folder(
                Path::new("/missing-models"),
                &source,
                dir.path(),
                &AtomicBool::new(true),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&source).unwrap(), before);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn working_directories_are_cleaned_after_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".working");
        std::fs::create_dir(&path).unwrap();
        {
            let _guard = WorkingDir {
                path: path.clone(),
                published: false,
            };
            std::fs::write(path.join("partial.wav"), b"partial").unwrap();
        }
        assert!(!path.exists());
    }
    #[test]
    #[ignore = "requires STEM_MODELS and STEM_AUDIO; full file integration"]
    fn full_file_keeps_length_alignment_and_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let source = PathBuf::from(std::env::var("STEM_AUDIO").unwrap());
        let models = PathBuf::from(std::env::var("STEM_MODELS").unwrap());
        let bytes = std::fs::read(&source).unwrap();
        let mut original = audio::decode(&source, 600.0).unwrap();
        crate::batch::resample(&mut original, dsp::RATE).unwrap();
        let start = std::time::Instant::now();
        let paths = separate_to_folder(
            &models,
            &source,
            dir.path(),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        let mut sum = vec![0.0f32; original.frames() * 2];
        for path in &paths {
            let stem = audio::decode(path, 600.0).unwrap();
            assert_eq!(stem.frames(), original.frames());
            for (a, b) in sum.iter_mut().zip(stem.samples) {
                *a += b;
            }
        }
        let max = sum
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (s - original.samples
                    [(i / 2) * original.channels + (i % 2).min(original.channels - 1)])
                .abs()
            })
            .fold(0.0f32, f32::max);
        eprintln!(
            "full file {:.2}s in {:?}, reconstruction error {max}",
            original.frames() as f64 / dsp::RATE as f64,
            start.elapsed()
        );
        assert!(max < 1e-4);
        let cancel = AtomicBool::new(false);
        assert!(
            separate_to_folder(&models, &source, dir.path(), &cancel, |_| cancel
                .store(true, Ordering::Relaxed))
            .is_err()
        );
        assert_eq!(std::fs::read(source).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(paths[0].parent().unwrap()).unwrap().count(), 4);
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    #[test]
    fn generated_stems_keep_grouping_without_automatic_tags_or_duplicate_sources() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        let mut conn = crate::db::open(&db_path).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        drop(conn);
        let folder = dir.path().join("results");
        std::fs::create_dir(&folder).unwrap();
        let paths = dsp::NAMES.map(|name| folder.join(format!("{name}.wav")));
        for path in &paths {
            crate::editor::write_wav(
                path,
                &Pcm {
                    samples: vec![0.1; 4410],
                    channels: 1,
                    sample_rate: 44100,
                },
                Some(32),
                None,
            )
            .unwrap();
        }
        let (_, ids) = import_files(&db_path, &paths).unwrap();
        assert_eq!(ids.len(), 4);
        let (_, again) = import_files(&db_path, &paths).unwrap();
        assert_eq!(ids, again);
        let conn = crate::db::open(&db_path).unwrap();
        let source_id: i64 = conn.query_row("SELECT source_id FROM samples WHERE id=?1",[ids[0]],|r|r.get(0)).unwrap();
        conn.execute("INSERT INTO samples(source_id,path,name,ext,size,mtime) VALUES(?1,'/original/song.wav','song','wav',0,0)",[source_id]).unwrap();
        let parent_id = conn.last_insert_rowid();
        link_stems(&conn, parent_id, &ids).unwrap();
        drop(conn);
        let conn = crate::db::open(&db_path).unwrap();
        let grouped = crate::library::Filter { group_stems: true, expanded_stems: vec![parent_id], ..Default::default() };
        let rows = crate::library::list(&conn, &grouped, 0, 10).unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].stem_count, 4);
        assert!(rows[1..].iter().all(|s| s.stem_parent_id==Some(parent_id)));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM sources", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM sample_tags", [], |r| r.get::<_, i64>(0)).unwrap(), 0);

        // Re-importing must preserve tags the user assigned themselves.
        let tag = crate::library::ensure_tag(&conn, "My project").unwrap();
        let mut conn = conn;
        crate::library::tag_samples(&mut conn, &[ids[0]], tag.id).unwrap();
        drop(conn);
        import_files(&db_path, &paths).unwrap();
        let conn = crate::db::open(&db_path).unwrap();
        let saved: String = conn.query_row(
            "SELECT t.name FROM tags t JOIN sample_tags st ON t.id=st.tag_id WHERE st.sample_id=?1",
            [ids[0]], |r| r.get(0),
        ).unwrap();
        assert_eq!(saved, "My project");
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }
}
