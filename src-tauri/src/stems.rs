//! One cancellable local separation job. Results are published only when all
//! stems have been written; a failed or cancelled job leaves the source intact.
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
}
impl Model {
    pub const ALL: [Self; 3] = [Self::Spleeter, Self::ScnetLarge, Self::ScnetXlIhf];
    pub fn id(self) -> &'static str {
        match self {
            Self::Spleeter => "spleeter",
            Self::ScnetLarge => "scnetLarge",
            Self::ScnetXlIhf => "scnetXlIhf",
        }
    }
}
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub model: Model,
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
    folder: String,
    model: Option<Model>,
) -> Result<()> {
    let model = model.unwrap_or_default();
    let path: String =
        state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT path FROM samples WHERE id=?1", [id], |r| r.get(0))?;
    if !Path::new(&folder).is_dir() {
        return Err(failure("choose an output folder"));
    }
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
        let state = app.state::<AppState>();
        let jobs = &state.stems;
        let result = run(&state, Path::new(&path), Path::new(&folder), model);
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
    let mut working = WorkingDir {
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
    if matches!(model, Model::ScnetLarge | Model::ScnetXlIhf) {
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
    let mut target = parent.join(format!("{stem} stems"));
    let mut index = 2;
    loop {
        match std::fs::create_dir(&target) {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                target = parent.join(format!("{stem} stems {index}"));
                index += 1;
            }
            Err(e) => return Err(e.into()),
        }
    }
    // Destination is reserved as an empty directory; a non-empty destination is never replaced.
    if let Err(e) = std::fs::rename(&working.path, &target) {
        let _ = std::fs::remove_dir(&target);
        return Err(e.into());
    }
    working.published = true;
    Ok(dsp::NAMES
        .iter()
        .map(|n| target.join(format!("{n}.wav")))
        .collect())
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
    Ok((
        paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        ids,
    ))
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
    let mut conn = crate::db::open(db_path)?;
    let mut ids = Vec::new();
    for (name, path) in dsp::NAMES.iter().zip(paths) {
        let id = conn.query_row(
            "SELECT id FROM samples WHERE path=?1",
            [path.to_string_lossy().as_ref()],
            |r| r.get(0),
        )?;
        let tag = crate::library::ensure_tag(&conn, &format!("stem:{name}"))?;
        crate::library::tag_samples(&mut conn, &[id], tag.id)?;
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
    #[ignore = "requires SCNET_MODELS and STEM_AUDIO; full file write and cancellation"]
    fn scnet_file_export_keeps_original_and_cleans_cancelled_work() {
        check_file_export(Model::ScnetLarge, "SCNET_MODELS")
    }
    #[test]
    #[ignore = "requires SCNET_XL_MODELS and STEM_AUDIO; XL full file write and cancellation"]
    fn xl_file_export_keeps_original_and_cleans_cancelled_work() {
        check_file_export(Model::ScnetXlIhf, "SCNET_XL_MODELS")
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
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    #[test]
    fn generated_stems_are_imported_and_tagged_without_duplicate_sources() {
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
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM sources", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        for (id, name) in ids.into_iter().zip(dsp::NAMES) {
            let tag:String=conn.query_row("SELECT t.name FROM tags t JOIN sample_tags st ON t.id=st.tag_id WHERE st.sample_id=?1",[id],|r|r.get(0)).unwrap();
            assert_eq!(tag, format!("stem:{name}"));
        }
    }
}
