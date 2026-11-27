//! One cancellable local separation job. Results are published only when all
//! stems have been written; a failed or cancelled job leaves the source intact.
mod demucs;
mod dsp;
pub(crate) mod models;
mod scnet;
mod vocal;
use crate::{
    AppState,
    audio::{self, Pcm},
    editor::encoding::{self, Format},
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
    MelRoformer,
    BsRoformer,
    Mdx23c,
}
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode { Two, #[default] Four }
impl Mode {
    fn names(self) -> &'static [&'static str] {
        match self { Self::Two => &["vocals", "instrumental"], Self::Four => &dsp::NAMES }
    }
    fn validate(self, model: Model) -> Result<()> {
        let expected = match self { Self::Two => 2, Self::Four => 4 };
        if models::tracks(model) != expected {
            return Err(failure("this model does not support the selected track mode"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct SeparationOptions {
    model: Model,
    mode: Mode,
    format: Format,
}

#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub model: Model,
    pub mode: Mode,
    pub format: Format,
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
    mode: Option<Mode>,
    format: Option<Format>,
) -> Result<()> {
    let model = model.unwrap_or_default();
    let mode = mode.unwrap_or_default();
    mode.validate(model)?;
    if state.db.lock().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM sample_stems WHERE child_id=?1)", [id], |r| r.get::<_, bool>(0))? {
        return Err(failure("select the original track to separate stems"));
    }
    let path: String =
        state
            .db
            .lock()
            .unwrap()
            .query_row("SELECT path FROM samples WHERE id=?1", [id], |r| r.get(0))?;
    let format = format.unwrap_or_else(|| Format::for_source(Path::new(&path)));
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
        mode,
        format,
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
    let options = SeparationOptions { model, mode, format };
    std::thread::spawn(move || {
        let (_model_lease, _processing_lease) = (model_lease, processing_lease);
        let state = app.state::<AppState>();
        let jobs = &state.stems;
        let result = run(&state, Path::new(&path), &folder, options);
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
#[cfg(test)]
fn separate_test_wav(
    model_dir: &Path,
    source: &Path,
    parent: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
) -> Result<Vec<PathBuf>> {
    separate_to_folder(SeparationOptions::default(), model_dir, source, parent, cancel, |_, value| progress(value))
}
fn separate_to_folder(
    options: SeparationOptions,
    model_dir: &Path,
    source: &Path,
    parent: &Path,
    cancel: &AtomicBool,
    update: impl Fn(&str, f64),
) -> Result<Vec<PathBuf>> {
    let SeparationOptions { model, mode, format } = options;
    let progress = |value| update("separating", value);
    mode.validate(model)?;
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
    let working = tempfile::Builder::new().prefix(".fonicca-stems-").tempdir_in(parent)?;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: dsp::RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let names = mode.names();
    let mut writers = names
        .iter()
        .map(|n| {
            hound::WavWriter::create(working.path().join(format!("{n}.wav")), spec).map_err(failure)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut written = 0;
    let mut emit = |stems: &[Vec<f32>]| {
        write_block(&mut writers, mode, &pcm, &mut written, stems)
    };
    if matches!(model, Model::MelRoformer | Model::BsRoformer | Model::Mdx23c) {
        vocal::Separator::load(model_dir, model)?.separate(&pcm, cancel, &progress, &mut emit)?;
    } else if model == Model::Htdemucs {
        demucs::Separator::load(model_dir)?.separate(&pcm, cancel, &progress, &mut emit)?;
    } else if matches!(model, Model::ScnetLarge | Model::ScnetXlIhf) {
        scnet::Separator::load_for(model_dir, model)?.separate(&pcm, cancel, &progress, &mut emit)?;
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
            let mut output = Vec::with_capacity(4);
            for (i, mut samples) in stems.into_iter().enumerate() {
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
                output.push(samples[begin..finish].to_vec());
                tails[i] = samples[finish..((end - left + 4096).min(right - left)) * 2].to_vec();
            }
            emit(&output)?;
            progress(end as f64 / pcm.frames() as f64);
        }
    }
    if written != pcm.frames() { return Err(failure("incomplete stem output")); }
    for writer in writers {
        writer.finalize().map_err(failure)?;
    }
    check(cancel)?;
    drop(pcm);
    encode_stems(working.path(), names, format, cancel, |value| update("encoding", value))?;
    check(cancel)?;
    publish_stems_named(working.path(), parent, &stem, names, format)
}

fn encode_stems(
    working: &Path, names: &[&str], format: Format, cancel: &AtomicBool, progress: impl Fn(f64),
) -> Result<()> {
    if format == Format::Wav {
        return Ok(());
    }
    progress(0.0);
    for (index, name) in names.iter().enumerate() {
        check(cancel)?;
        let source = working.join(format!("{name}.wav"));
        let target = working.join(format!("{name}.{}", format.extension()));
        encoding::transcode(&source, &target, format, &|| cancel.load(Ordering::Relaxed))?;
        std::fs::remove_file(source)?;
        progress((index + 1) as f64 / names.len() as f64);
    }
    Ok(())
}

// All backends emit interleaved stereo, vocals first. Derive the two-stem
// residual after overlap-add, preserving the source mix without normalization.
fn write_block<W: std::io::Write + std::io::Seek>(
    writers: &mut [hound::WavWriter<W>], mode: Mode, pcm: &Pcm,
    written: &mut usize, stems: &[Vec<f32>],
) -> Result<()> {
    let vocals = stems.first().ok_or_else(|| failure("missing vocals"))?;
    let frames = vocals.len() / 2;
    if vocals.len() % 2 != 0 || *written + frames > pcm.frames()
        || stems.iter().any(|s| s.len() != vocals.len() || s.iter().any(|v| !v.is_finite()))
        || writers.len() != mode.names().len() || (mode == Mode::Four && stems.len() != 4) {
        return Err(failure("invalid stem output"));
    }
    if mode == Mode::Two {
        for (i, &vocal) in vocals.iter().enumerate() {
            writers[0].write_sample(vocal).map_err(failure)?;
            let source = pcm.samples[(*written + i / 2) * pcm.channels + (i % 2).min(pcm.channels - 1)];
            writers[1].write_sample(source - vocal).map_err(failure)?;
        }
    } else {
        for (writer, stem) in writers.iter_mut().zip(stems) {
            for &sample in stem { writer.write_sample(sample).map_err(failure)?; }
        }
    }
    *written += frames;
    Ok(())
}

/// Publish one track per directory, never reusing an existing destination.
/// Repeated separations choose a new numbered directory for the complete set.
#[cfg(test)]
fn publish_stems(working: &Path, parent: &Path, name: &str) -> Result<Vec<PathBuf>> {
    publish_stems_named(working, parent, name, &dsp::NAMES, Format::Wav)
}
fn publish_stems_named(working: &Path, parent: &Path, name: &str, names: &[&str], format: Format) -> Result<Vec<PathBuf>> {
    let extension = format.extension();
    for version in 1.. {
        let suffix = if version == 1 { String::new() } else { format!(" {version}") };
        let folder = parent.join(format!("{name}{suffix}"));
        match std::fs::create_dir(&folder) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
        let paths: Vec<_> = names.iter()
            .map(|kind| folder.join(format!("{name} - {kind}.{extension}"))).collect();
        let mut published = Vec::new();
        let result: std::io::Result<()> = (|| {
            for (kind, target) in names.iter().zip(&paths) {
                // create_new is supported on removable drives too. Never claim
                // or clean up a destination that belonged to another process.
                let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(target)?;
                published.push(target.clone());
                let mut input = std::fs::File::open(working.join(format!("{kind}.{extension}")))?;
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
    options: SeparationOptions,
) -> Result<(Vec<String>, Vec<i64>)> {
    let jobs = &state.stems;
    jobs.update("downloading", 0.0);
    let model_dir = models::ensure_for(&state.models_dir, options.model, &jobs.cancel, |received| {
        jobs.update("downloading", received as f64 / models::total(options.model) as f64)
    })?;
    jobs.update("separating", 0.0);
    let paths =
        separate_to_folder(options, &model_dir, source, parent, &jobs.cancel, |stage, p| {
            jobs.update(stage, p)
        })?;
    jobs.update("importing", 1.0);
    let output_paths: Vec<_> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    jobs.status.lock().unwrap().paths = output_paths.clone();
    let ids = import(state, &paths)?;
    let conn = crate::db::open(&state.db_path)?;
    let parent_id: i64 = conn.query_row("SELECT id FROM samples WHERE path=?1", [source.to_string_lossy().as_ref()], |r| r.get(0))?;
    link_stems_named(&conn, parent_id, &ids, options.mode.names())?;
    Ok((output_paths, ids))
}
#[cfg(test)]
fn link_stems(conn: &rusqlite::Connection, parent_id: i64, ids: &[i64]) -> Result<()> {
    link_stems_named(conn, parent_id, ids, &dsp::NAMES)
}
fn link_stems_named(conn: &rusqlite::Connection, parent_id: i64, ids: &[i64], names: &[&str]) -> Result<()> {
    if ids.len() != names.len() { return Err(failure("stem identity mismatch")); }
    let tx = conn.unchecked_transaction()?;
    for (position, (id, kind)) in ids.iter().zip(names).enumerate() {
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
    #[ignore = "requires VOCAL_MODELS and STEM_AUDIO; real two-WAV export and cancellation"]
    fn real_two_track_file_export_preserves_source_and_residual() {
        let models = PathBuf::from(std::env::var("VOCAL_MODELS").unwrap());
        let source = PathBuf::from(std::env::var("STEM_AUDIO").unwrap());
        let original = std::fs::read(&source).unwrap();
        let mut pcm = audio::decode(&source,600.0).unwrap();
        crate::batch::resample(&mut pcm,dsp::RATE).unwrap();
        for model in [Model::MelRoformer,Model::Mdx23c] {
            let dir = tempfile::tempdir().unwrap();
            let model_dir = &models;
            let options = SeparationOptions { model, mode: Mode::Two, ..Default::default() };
            let paths = separate_to_folder(options, model_dir, &source, dir.path(), &AtomicBool::new(false), |_, _| {}).unwrap();
            assert_eq!(paths.len(),2);
            assert_eq!(std::fs::read_dir(paths[0].parent().unwrap()).unwrap().count(),2);
            let vocals = audio::decode(&paths[0],600.0).unwrap();
            let instrumental = audio::decode(&paths[1],600.0).unwrap();
            assert_eq!(vocals.frames(),pcm.frames()); assert_eq!(instrumental.frames(),pcm.frames());
            for i in 0..vocals.samples.len() {
                let original = pcm.samples[(i/2)*pcm.channels+(i%2).min(pcm.channels-1)];
                assert!((vocals.samples[i]+instrumental.samples[i]-original).abs()<2e-6);
            }
            let cancel = AtomicBool::new(true);
            assert!(separate_to_folder(options, model_dir, &source, dir.path(), &cancel, |_, _| cancel.store(true, Ordering::Relaxed)).is_err());
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(),1);
            assert_eq!(std::fs::read(&source).unwrap(),original);
            eprintln!("{model:?}: two WAVs, exact residual, original preserved, cancelled files removed");
        }
    }
    #[test]
    fn two_track_output_is_vocals_and_exact_source_residual() {
        for channels in [1, 2] {
            let pcm = Pcm { samples: (0..12*channels).map(|i| i as f32 * 0.06 - 0.3).collect(), channels, sample_rate: dsp::RATE };
            let spec = hound::WavSpec { channels: 2, sample_rate: dsp::RATE, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
            let dir = tempfile::tempdir().unwrap();
            let files = [dir.path().join("vocals.wav"), dir.path().join("instrumental.wav")];
            let mut writers = files.iter().map(|path| hound::WavWriter::create(path,spec).unwrap()).collect::<Vec<_>>();
            let mut written = 0;
            for count in [4, 8] {
                let vocal = vec![0.125; count*2];
                write_block(&mut writers,Mode::Two,&pcm,&mut written,&[vocal,vec![999.0;count*2]]).unwrap();
            }
            assert_eq!(written, 12);
            for writer in writers { writer.finalize().unwrap(); }
            let read = |path: &Path| hound::WavReader::open(path).unwrap().samples::<f32>().map(|v| v.unwrap()).collect::<Vec<_>>();
            let vocal = read(&files[0]); let instrumental = read(&files[1]);
            assert_eq!(vocal.len(),24);
            for i in 0..24 {
                let source = pcm.samples[(i/2)*channels + (i%2).min(channels-1)];
                assert_eq!(vocal[i],0.125);
                assert!((vocal[i] + instrumental[i] - source).abs() < 1e-6);
            }
            // Published two-track files have their own kinds and never overwrite a previous set.
            let work = tempfile::tempdir().unwrap(); let parent = tempfile::tempdir().unwrap();
            for name in Mode::Two.names() { std::fs::write(work.path().join(format!("{name}.wav")),name).unwrap(); }
            let paths = publish_stems_named(work.path(),parent.path(),"Song",Mode::Two.names(),Format::Wav).unwrap();
            assert_eq!(paths.len(),2);
            assert!(paths[1].ends_with("Song - instrumental.wav"));
        }
    }
    #[test]
    fn vocal_only_models_reject_four_track_requests() {
        for model in Model::ALL {
            assert_eq!(serde_json::to_string(&model).unwrap(), format!("\"{}\"", model.id()));
        }
        for model in [Model::MelRoformer,Model::BsRoformer,Model::Mdx23c] {
            assert!(Mode::Four.validate(model).is_err());
            assert!(Mode::Two.validate(model).is_ok());
        }
        for model in [Model::ScnetLarge,Model::ScnetXlIhf,Model::Spleeter,Model::Htdemucs] {
            assert!(Mode::Two.validate(model).is_err());
            assert!(Mode::Four.validate(model).is_ok());
        }
    }
    #[test]
    fn progress_keeps_source_identity_across_stages() {
        let jobs = Jobs::default();
        *jobs.status.lock().unwrap() = Progress { source_id: Some(42), format: Format::Flac, running: true, ..Default::default() };
        for stage in ["downloading", "separating", "encoding", "importing"] {
            jobs.update(stage, 0.45);
            let status = jobs.status.lock().unwrap();
            assert_eq!(status.source_id, Some(42));
            let json = serde_json::to_value(&*status).unwrap();
            assert_eq!(json["sourceId"], 42);
            assert_eq!(json["format"], "flac");
        }
    }

    #[test]
    fn converted_stems_publish_and_import_as_the_requested_format() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("stems.sqlite");
        crate::db::migrate(&mut crate::db::open(&db_path).unwrap()).unwrap();
        let names = Mode::Two.names();
        let cancel = AtomicBool::new(false);
        let spec = hound::WavSpec {
            channels: 2, sample_rate: dsp::RATE, bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        for format in [Format::Mp3, Format::Ogg, Format::Flac, Format::Aiff, Format::M4a, Format::Caf] {
            let work = tempfile::tempdir().unwrap();
            for name in names {
                let mut writer = hound::WavWriter::create(work.path().join(format!("{name}.wav")), spec).unwrap();
                for i in 0..4097 {
                    let value = (i as f32 * 0.05).sin() * 0.4;
                    writer.write_sample(value).unwrap();
                    writer.write_sample(-value).unwrap();
                }
                writer.finalize().unwrap();
            }
            encode_stems(work.path(), names, format, &cancel, |_| {}).unwrap();
            let paths = publish_stems_named(work.path(), dir.path(), "Song", names, format).unwrap();
            for (path, name) in paths.iter().zip(names) {
                assert!(path.ends_with(format!("Song - {name}.{}", format.extension())));
                let decoded = audio::decode(path, f64::INFINITY).unwrap();
                assert_eq!(decoded.channels, 2);
                assert_eq!(decoded.sample_rate, dsp::RATE);
                if matches!(format, Format::Mp3 | Format::Ogg) {
                    // The library reader includes codec delay and packet padding.
                    assert!((decoded.frames() as isize - 4097).unsigned_abs() < 2304, "{format:?}");
                } else {
                    assert_eq!(decoded.frames(), 4097, "{format:?}");
                }
            }
            let (_, ids) = import_files(&db_path, &paths).unwrap();
            assert_eq!(ids.len(), names.len());
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
        let format = Format::for_source(&source);
        let options = SeparationOptions { model, mode: Mode::Four, format };
        let start = std::time::Instant::now();
        let paths = separate_to_folder(
            options,
            &models,
            &source,
            dir.path(),
            &AtomicBool::new(false),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(paths.len(), 4);
        for path in &paths {
            assert_eq!(path.extension().unwrap(), format.extension());
            let stem = audio::decode(path, 600.0).unwrap();
            let expected = (pcm.frames() as f64 * dsp::RATE as f64 / pcm.sample_rate as f64).round() as usize;
            if matches!(format, Format::Mp3 | Format::Ogg) {
                assert!(stem.frames().abs_diff(expected) < 2304);
            } else {
                assert_eq!(stem.frames(), expected);
            }
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
        let cancel_stage = if format == Format::Wav { "separating" } else { "encoding" };
        assert!(
            separate_to_folder(options, &models, &source, dir.path(), &cancel, |stage, _| {
                if stage == cancel_stage {
                    cancel.store(true, Ordering::Relaxed);
                }
            }).is_err()
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
            separate_test_wav(
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
        let source = dir.path().join("source.wav");
        let pcm = Pcm { samples: vec![0.2; 4410], channels: 1, sample_rate: dsp::RATE };
        crate::editor::write_wav(&source, &pcm, Some(32), None).unwrap();
        // Model loading fails after opening the temporary stem writers.
        let missing_model = dir.path().join("missing-model");
        assert!(separate_test_wav(&missing_model, &source, dir.path(), &AtomicBool::new(false), |_| {}).is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(source.is_file());
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
        let paths = separate_test_wav(
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
            separate_test_wav(&models, &source, dir.path(), &cancel, |_| cancel
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
