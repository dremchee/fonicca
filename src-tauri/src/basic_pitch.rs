//! Local audio-to-MIDI with Spotify Basic Pitch's pinned ONNX model.
mod midi;
mod notes;
pub(crate) mod input;
use input::prepare;
use crate::{
    AppState,
    audio::{self, Pcm},
    error::{Error, Result},
    stems::models,
};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Tensor,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, Emitter, Manager, State, Window};
const RATE: u32 = 22050;
const WINDOW: usize = 43844;
const OUTPUT_FRAMES: usize = 172;
const TRIM: usize = 15;
const KEPT: usize = OUTPUT_FRAMES - TRIM * 2;
const STRIDE: usize = WINDOW - TRIM * 2 * 256;
const KEYS: usize = 88;
fn failure(e: impl std::fmt::Display) -> Error {
    Error::Other(format!("Basic Pitch: {e}"))
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(failure("cancelled"))
    } else {
        Ok(())
    }
}
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    source_id: Option<i64>,
    range: Option<crate::editor::Span>,
    pub running: bool,
    stage: String,
    progress: f64,
    error: Option<String>,
    cancelled: bool,
    path: Option<String>,
    note_count: usize,
    name: String,
}
#[derive(Default)]
pub struct Jobs {
    running: AtomicBool,
    cancel: AtomicBool,
    status: Mutex<Progress>,
}
impl Jobs {
    pub fn is_running(&self) -> bool { self.running.load(Ordering::SeqCst) }

    fn update(&self, stage: &str, progress: f64) {
        let mut state = self.status.lock().unwrap();
        state.stage = stage.into();
        state.progress = progress.clamp(0.0, 1.0);
    }
    pub fn downloading(&self) -> bool {
        let s = self.status.lock().unwrap();
        s.running && s.stage == "downloading"
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
pub fn basic_pitch_status(state: State<'_, AppState>) -> Status {
    Status {
        progress: state.basic_pitch.status.lock().unwrap().clone(),
        model_ready: models::basic_pitch_installed(&state.models_dir),
        download_bytes: models::BASIC_PITCH_TOTAL,
    }
}
#[tauri::command]
pub fn basic_pitch_cancel(state: State<'_, AppState>) {
    state.basic_pitch.cancel.store(true, Ordering::Relaxed);
}
enum Source {
    File(PathBuf),
    Editor(crate::editor::blocks::BlockAudio),
}
#[tauri::command]
pub fn basic_pitch_start(
    app: AppHandle,
    state: State<'_, AppState>,
    window: Window,
    id: i64,
    range: Option<crate::editor::Span>,
    path: String,
    bpm: Option<f64>,
) -> Result<()> {
    let target = PathBuf::from(path);
    midi::validate_path(&target)?;
    let (source_path, name, source_bpm): (String, String, Option<f64>) =
        state.db.lock().unwrap().query_row(
            "SELECT path,name,bpm FROM samples WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
    let source = if let Some(range) = range {
        Source::Editor(crate::editor_commands::transcription_audio(
            &state, &window, range,
        )?)
    } else {
        Source::File(PathBuf::from(&source_path))
    };
    if target == Path::new(&source_path) {
        return Err(failure("choose a new MIDI file"));
    }
    let lease = state
        .model_operations
        .acquire("basicPitch", crate::models::Operation::Using)?;
    if state
        .basic_pitch
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(failure("a transcription is already running"));
    }
    state.basic_pitch.cancel.store(false, Ordering::Relaxed);
    *state.basic_pitch.status.lock().unwrap() = Progress {
        source_id: Some(id),
        range,
        running: true,
        stage: "preparing".into(),
        name: name.clone(),
        ..Default::default()
    };
    std::thread::spawn(move || {
        let _lease = lease;
        let state = app.state::<AppState>();
        let jobs = &state.basic_pitch;
        let result = (|| {
            check(&jobs.cancel)?;
            let pcm = match source {
                Source::Editor(audio) => input::prepare_snapshot(&audio)?,
                Source::File(path) => {
                    if audio::probe(&path)?.duration.is_some_and(|d| d > 600.0) {
                        return Err(failure("use audio up to 10 minutes"));
                    }
                    prepare(audio::decode(&path, 600.1)?)?
                }
            };
            check(&jobs.cancel)?;
            jobs.update("downloading", 0.0);
            let dir = models::ensure_basic_pitch(&state.models_dir, &jobs.cancel, |n| {
                jobs.update("downloading", n as f64 / models::BASIC_PITCH_TOTAL as f64)
            })?;
            jobs.update("transcribing", 0.0);
            let prediction = predict(&dir, &pcm, &jobs.cancel, |p| jobs.update("transcribing", p))?;
            jobs.update("notes", 1.0);
            let notes = notes::decode(
                &prediction.frames,
                &prediction.onsets,
                prediction.duration,
                &jobs.cancel,
            )?;
            if notes.is_empty() {
                return Err(failure(
                    "no notes detected; try an isolated melodic instrument or a longer fragment",
                ));
            }
            let bytes = midi::encode(&notes, bpm.or(source_bpm), &name)?;
            jobs.update("saving", 1.0);
            midi::save(&target, &bytes, &jobs.cancel)?;
            Ok(notes.len())
        })();
        let mut progress = jobs.status.lock().unwrap();
        progress.running = false;
        match result {
            Ok(count) => {
                progress.stage = "done".into();
                progress.progress = 1.0;
                progress.note_count = count;
                progress.path = Some(target.to_string_lossy().into_owned());
            }
            Err(error) => {
                progress.cancelled = jobs.cancel.load(Ordering::Relaxed);
                progress.stage = if progress.cancelled {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                if !progress.cancelled {
                    progress.error = Some(error.to_string());
                }
            }
        }
        jobs.running.store(false, Ordering::SeqCst);
        let _ = app.emit("basic-pitch:changed", progress.clone());
    });
    Ok(())
}
#[derive(Serialize)]
struct Prediction {
    frames: Vec<f32>,
    onsets: Vec<f32>,
    duration: f64,
}
fn predict(
    dir: &Path,
    pcm: &Pcm,
    cancel: &AtomicBool,
    progress: impl Fn(f64),
) -> Result<Prediction> {
    check(cancel)?;
    if pcm.channels != 1 || pcm.sample_rate != RATE {
        return Err(failure("invalid transcription input"));
    }
    let mut session = Session::builder()
        .map_err(failure)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(failure)?
        .with_intra_threads(2)
        .map_err(failure)?
        .with_intra_op_spinning(false)
        .map_err(failure)?
        .commit_from_file(dir.join(models::BASIC_PITCH_FILE))
        .map_err(failure)?;
    let n = pcm.frames();
    let padded = n + TRIM * 256;
    let count = padded.div_ceil(STRIDE);
    let expected_frames = n * KEPT / STRIDE;
    let mut frames = Vec::with_capacity(count * KEPT * KEYS);
    let mut onsets = Vec::with_capacity(count * KEPT * KEYS);
    for (chunk, offset) in (0..padded).step_by(STRIDE).enumerate() {
        check(cancel)?;
        let mut input = vec![0.0; WINDOW];
        for (i, v) in input.iter_mut().enumerate() {
            let source = offset as isize + i as isize - (TRIM * 256) as isize;
            if source >= 0 && (source as usize) < n {
                *v = pcm.samples[source as usize];
            }
        }
        let input = Tensor::from_array(([1usize, WINDOW, 1], input)).map_err(failure)?;
        let output = session
            .run(ort::inputs!["serving_default_input_2:0"=>input])
            .map_err(failure)?;
        check(cancel)?;
        for (name, target) in [
            ("StatefulPartitionedCall:1", &mut frames),
            ("StatefulPartitionedCall:2", &mut onsets),
        ] {
            let (shape, values) = output[name].try_extract_tensor::<f32>().map_err(failure)?;
            if shape.as_ref() != [1, OUTPUT_FRAMES as i64, KEYS as i64]
                || values.iter().any(|v| !v.is_finite())
            {
                return Err(failure("invalid Basic Pitch model output"));
            }
            target.extend_from_slice(&values[TRIM * KEYS..(OUTPUT_FRAMES - TRIM) * KEYS]);
        }
        progress((chunk + 1) as f64 / count as f64);
    }
    frames.truncate(expected_frames * KEYS);
    onsets.truncate(expected_frames * KEYS);
    Ok(Prediction {
        frames,
        onsets,
        duration: n as f64 / RATE as f64,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_audio_and_cancelled_predictions_are_rejected() {
        let bad = Pcm {
            samples: vec![f32::NAN],
            channels: 1,
            sample_rate: RATE,
        };
        assert!(prepare(bad).is_err());
        let empty = Pcm {
            samples: vec![],
            channels: 1,
            sample_rate: RATE,
        };
        assert!(prepare(empty).is_err());
        let mono = Pcm {
            samples: vec![0.0; 100],
            channels: 1,
            sample_rate: RATE,
        };
        assert!(predict(Path::new("/missing"), &mono, &AtomicBool::new(true), |_| {}).is_err());
    }
    #[test]
    #[ignore = "requires BASIC_PITCH_MODELS and BASIC_PITCH_AUDIO; optional BASIC_PITCH_OUTPUT"]
    fn real_model_inference() {
        let dir = PathBuf::from(std::env::var("BASIC_PITCH_MODELS").unwrap());
        let source = PathBuf::from(std::env::var("BASIC_PITCH_AUDIO").unwrap());
        let original = std::fs::read(&source).unwrap();
        let pcm = prepare(audio::decode(&source, 600.0).unwrap()).unwrap();
        let start = std::time::Instant::now();
        let prediction = predict(&dir, &pcm, &AtomicBool::new(false), |_| {}).unwrap();
        let notes = notes::decode(
            &prediction.frames,
            &prediction.onsets,
            prediction.duration,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(!notes.is_empty());
        assert!(
            notes
                .iter()
                .all(|n| n.end <= prediction.duration && n.end > n.start)
        );
        eprintln!(
            "Basic Pitch: {} notes, {}s audio in {:?}",
            notes.len(),
            prediction.duration,
            start.elapsed()
        );
        let bytes = midi::encode(&notes, Some(120.0), "Basic Pitch test").unwrap();
        if let Ok(output) = std::env::var("BASIC_PITCH_OUTPUT") {
            let output = PathBuf::from(output);
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(
                output.join("predictions.json"),
                serde_json::to_vec(&prediction).unwrap(),
            )
            .unwrap();
            std::fs::write(
                output.join("notes.json"),
                serde_json::to_vec(&notes).unwrap(),
            )
            .unwrap();
            crate::editor::write_wav(&output.join("input.wav"), &pcm, Some(32), None).unwrap();
            midi::save(&output.join("notes.mid"), &bytes, &AtomicBool::new(false)).unwrap();
        }
        assert_eq!(std::fs::read(source).unwrap(), original);
    }
}
