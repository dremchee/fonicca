//! Batch processing of library samples: one recipe (trim, normalize, fades,
//! format, renaming) applied to many files, in the background.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::audio::{self, Pcm};
use crate::editor::{self, Curve, Op};
use crate::editor::processing::{normalize_lufs, validate_lufs, LoudnessReport};
use crate::error::{Error, Result};
use crate::AppState;

pub const PROGRESS_EVENT: &str = "batch:progress";
pub const DONE_EVENT: &str = "batch:done";

/// Files longer than this are skipped (each is decoded whole).
const MAX_SECONDS: f64 = 30.0 * 60.0;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trim {
    /// Silence is quieter than this.
    pub threshold_db: f32,
    /// Kept before the first and after the last sound.
    pub pad_ms: f32,
}

/// Trim, channels, DC removal, peak normalization, fades, sample rate, then LUFS.
/// Finally written as WAV at `bit_depth` and named by `rename`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub trim: Option<Trim>,
    /// Peak level, dB.
    pub normalize: Option<f32>,
    /// An alternative to peak normalization, applied after fades and resampling.
    pub lufs: Option<Lufs>,
    #[serde(default)]
    pub dc_offset: bool,
    pub fade_in_ms: Option<f32>,
    pub fade_out_ms: Option<f32>,
    #[serde(default = "smooth")]
    pub curve: Curve,
    /// 1 or 2.
    pub channels: Option<usize>,
    pub sample_rate: Option<u32>,
    /// 16, 24 or 32 (float); the source's depth when unset.
    pub bit_depth: Option<u32>,
    /// File name template, see [`render_name`].
    pub rename: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Lufs { pub target: f64, pub ceiling: f64 }

impl Recipe {
    fn validate(&self) -> Result<()> {
        if self.normalize.is_some() && self.lufs.is_some() {
            return Err(Error::Other("choose either peak or LUFS normalization".into()));
        }
        if self.normalize.is_some_and(|p| !p.is_finite() || !(-96.0..=0.0).contains(&p)) {
            return Err(Error::Other("peak level must be between -96 and 0 dBFS".into()));
        }
        if let Some(lufs) = self.lufs { validate_lufs(lufs.target, lufs.ceiling)?; }
        Ok(())
    }
}

pub struct Processed { pub pcm: Pcm, pub loudness: Option<LoudnessReport> }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileLoudness {
    name: String,
    path: String,
    target: f64,
    #[serde(flatten)]
    report: LoudnessReport,
}

struct Written { path: PathBuf, loudness: Option<FileLoudness> }

fn smooth() -> Curve {
    Curve::Smooth
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Destination {
    /// Copies in one folder.
    Folder { path: String },
    /// Over the originals (WAV only, not renamed), each backed up first.
    InPlace,
}

// --- Processing -----------------------------------------------------------------

fn db_to_amp(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Cuts leading and trailing silence, keeping `pad_ms` around the sound. Audio
/// that is silent throughout stays as it is.
fn trim(pcm: &mut Pcm, t: Trim) {
    let ch = pcm.channels.max(1);
    let threshold = db_to_amp(t.threshold_db);
    let loud = |f: &[f32]| f.iter().any(|s| s.abs() > threshold);
    let frames: Vec<&[f32]> = pcm.samples.chunks(ch).collect();
    let (Some(first), Some(last)) = (frames.iter().position(|f| loud(f)), frames.iter().rposition(|f| loud(f))) else {
        return;
    };
    let pad = (t.pad_ms.max(0.0) / 1000.0 * pcm.sample_rate as f32) as usize;
    let start = first.saturating_sub(pad);
    let end = (last + 1 + pad).min(frames.len());
    editor::apply(pcm, &Op::Crop { start, end });
}

/// To `channels` channels: mono is copied to every channel, more are averaged down.
fn remix(pcm: &mut Pcm, channels: usize) {
    let (from, to) = (pcm.channels.max(1), channels.max(1));
    if from == to {
        return;
    }
    let samples = pcm
        .samples
        .chunks(from)
        .flat_map(|f| {
            let mixed = f.iter().sum::<f32>() / from as f32;
            (0..to).map(move |c| if from == 1 { f[0] } else if to == 1 { mixed } else { f[c.min(from - 1)] })
        })
        .collect();
    *pcm = Pcm { samples, channels: to, sample_rate: pcm.sample_rate };
}

pub(crate) fn resample(pcm: &mut Pcm, rate: u32) -> Result<()> {
    if pcm.sample_rate == rate || pcm.frames() < 2 {
        pcm.sample_rate = rate;
        return Ok(());
    }
    let err = |e: &dyn std::fmt::Display| Error::Audio(format!("resampling: {e}"));
    let ch = pcm.channels.max(1);
    let mut resampler =
        Fft::<f32>::new(pcm.sample_rate as usize, rate as usize, 1024, ch, FixedSync::Both).map_err(|e| err(&e))?;
    let input = InterleavedSlice::new(&pcm.samples, ch, pcm.frames()).map_err(|e| err(&e))?;
    let output = resampler.process_all(&input, pcm.frames(), None).map_err(|e| err(&e))?;
    *pcm = Pcm { samples: output.take_data(), channels: ch, sample_rate: rate };
    Ok(())
}

/// Applies the audio part of the recipe.
pub fn process(mut pcm: Pcm, r: &Recipe) -> Result<Processed> {
    r.validate()?;
    if let Some(t) = r.trim {
        trim(&mut pcm, t);
    }
    if let Some(ch) = r.channels {
        remix(&mut pcm, ch);
    }
    let frames = pcm.frames();
    if r.dc_offset {
        editor::apply(&mut pcm, &Op::DcOffset { start: 0, end: frames });
    }
    if let Some(peak_db) = r.normalize {
        editor::apply(&mut pcm, &Op::Normalize { start: 0, end: frames, peak_db });
    }
    let rate = pcm.sample_rate as f32;
    let ms = |v: f32| ((v.max(0.0) / 1000.0) * rate) as usize;
    if let Some(len) = r.fade_in_ms.map(ms).filter(|&n| n > 0) {
        editor::apply(&mut pcm, &Op::FadeIn { start: 0, end: len.min(frames), curve: r.curve });
    }
    if let Some(len) = r.fade_out_ms.map(ms).filter(|&n| n > 0) {
        editor::apply(&mut pcm, &Op::FadeOut { start: frames.saturating_sub(len), end: frames, curve: r.curve });
    }
    if let Some(rate) = r.sample_rate {
        resample(&mut pcm, rate)?;
    }
    let loudness = r.lufs.map(|l| normalize_lufs(&mut pcm, l.target, l.ceiling)).transpose()?;
    Ok(Processed { pcm, loudness })
}

// --- Naming ---------------------------------------------------------------------

/// What a file name template can use.
#[derive(Debug, Clone, Default)]
pub struct NameFields {
    pub name: String,
    pub category: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub kind: Option<String>,
}

/// Fills a template: `{name}`, `{n}` (position, zero-padded to the batch size),
/// `{category}`, `{bpm}`, `{key}`, `{kind}`. An unknown value takes one separator
/// next to it along, so no doubled or dangling ones are left; the rest of the
/// name stays as written. Characters a file name can't hold go.
pub fn render_name(template: &str, f: &NameFields, n: usize, total: usize) -> String {
    let width = total.to_string().len().max(2);
    let value = |token: &str| -> Option<String> {
        match token {
            "name" => Some(f.name.clone()),
            "n" => Some(format!("{n:0width$}")),
            "category" => f.category.clone(),
            "bpm" => f.bpm.map(|b| format!("{}", b.round() as i64)),
            "key" => f.key.as_ref().map(|k| k.replace(' ', "")),
            "kind" => f.kind.clone(),
            _ => None,
        }
        .filter(|v| !v.is_empty())
    };
    let is_sep = |c: char| matches!(c, '_' | '-' | ' ' | '.');
    let mut out = String::new();
    // Separators the template put right before the current position (only the
    // template's own, never a value's), and whether the next ones should go.
    let mut trailing_seps = 0;
    let mut drop_next_seps = false;
    let mut rest = template;
    let literal = |out: &mut String, text: &str, trailing: &mut usize, drop: &mut bool| {
        let text = if std::mem::take(drop) { text.trim_start_matches(is_sep) } else { text };
        out.push_str(text);
        *trailing = text.chars().rev().take_while(|&c| is_sep(c)).count();
    };
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else { break };
        literal(&mut out, &rest[..open], &mut trailing_seps, &mut drop_next_seps);
        match value(&rest[open + 1..open + close]) {
            Some(v) => {
                out.push_str(&v);
                trailing_seps = 0;
            }
            // An empty value takes the separator before it, or at the start of the
            // name the one after.
            None if trailing_seps > 0 => {
                for _ in 0..trailing_seps {
                    out.pop();
                }
                trailing_seps = 0;
            }
            None if out.chars().all(is_sep) => drop_next_seps = true,
            None => {}
        }
        rest = &rest[open + close + 1..];
    }
    literal(&mut out, rest, &mut trailing_seps, &mut drop_next_seps);
    let name: String = out.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '\0')).collect();
    let name = name.trim().to_string();
    if name.is_empty() { f.name.clone() } else { name }
}

/// `dir/stem.wav`, or `stem 2.wav`, … when that is taken on disk or by this batch.
fn free_path(dir: &Path, stem: &str, taken: &Mutex<HashSet<PathBuf>>) -> PathBuf {
    let mut taken = taken.lock().unwrap();
    let path = (1..)
        .map(|i| dir.join(if i == 1 { format!("{stem}.wav") } else { format!("{stem} {i}.wav") }))
        .find(|p| !p.exists() && !taken.contains(p))
        .expect("some name is free");
    taken.insert(path.clone());
    path
}

// --- Running --------------------------------------------------------------------

struct Item {
    path: PathBuf,
    ext: String,
    bit_depth: Option<u32>,
    fields: NameFields,
}

fn items(conn: &rusqlite::Connection, ids: &[i64]) -> Result<Vec<Item>> {
    let mut stmt =
        conn.prepare("SELECT path, name, ext, bit_depth, bpm, musical_key, category, kind FROM samples WHERE id = ?1")?;
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        let item = stmt.query_row([id], |r| {
            Ok(Item {
                path: PathBuf::from(r.get::<_, String>(0)?),
                ext: r.get(2)?,
                bit_depth: r.get(3)?,
                fields: NameFields { name: r.get(1)?, bpm: r.get(4)?, key: r.get(5)?, category: r.get(6)?, kind: r.get(7)? },
            })
        });
        match item {
            Ok(i) => out.push(i),
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(out)
}

static CANCEL: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
    name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    name: String,
    error: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Done {
    written: usize,
    failed: Vec<Failure>,
    loudness: Vec<FileLoudness>,
    cancelled: bool,
    /// The output folder, when there is one.
    dir: Option<String>,
}

/// File names the template gives the first `ids`, for the dialog's preview.
#[tauri::command]
pub fn batch_preview(state: State<'_, AppState>, ids: Vec<i64>, template: String, total: usize) -> Result<Vec<String>> {
    Ok(items(&state.db.lock().unwrap(), &ids)?.iter().enumerate().map(|(i, it)| render_name(&template, &it.fields, i + 1, total)).collect())
}

/// Starts processing; progress and the result come as events.
#[tauri::command]
pub fn batch_run(app: AppHandle, state: State<'_, AppState>, ids: Vec<i64>, recipe: Recipe, destination: Destination) -> Result<()> {
    recipe.validate()?;
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(Error::Other("a batch is already running".into()));
    }
    CANCEL.store(false, Ordering::SeqCst);
    let items = match items(&state.db.lock().unwrap(), &ids) {
        Ok(i) => i,
        Err(e) => {
            RUNNING.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    let backups = state.db_path.parent().map(|d| d.join("backups")).unwrap_or_else(|| PathBuf::from("backups"));
    std::thread::spawn(move || {
        let done = run(&app, items, &recipe, &destination, &backups);
        RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit(DONE_EVENT, done);
    });
    Ok(())
}

#[tauri::command]
pub fn batch_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

fn run(app: &AppHandle, items: Vec<Item>, recipe: &Recipe, destination: &Destination, backups: &Path) -> Done {
    let total = items.len();
    let counter = AtomicUsize::new(0);
    let taken = Mutex::new(HashSet::new());
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let dir = match destination {
        Destination::Folder { path } => Some(PathBuf::from(path)),
        Destination::InPlace => None,
    };
    let results: Vec<std::result::Result<Written, Failure>> = items
        .par_iter()
        .enumerate()
        .filter(|_| !CANCEL.load(Ordering::Relaxed))
        .map(|(i, item)| {
            let result = process_one(item, i + 1, total, recipe, dir.as_deref(), backups, stamp, &taken)
                .map_err(|e| Failure { name: item.fields.name.clone(), error: e.to_string() });
            let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = app.emit(PROGRESS_EVENT, Progress { done, total, name: item.fields.name.clone() });
            result
        })
        .collect();

    let mut written = Vec::new();
    let mut failed = Vec::new();
    let mut loudness = Vec::new();
    for r in results {
        match r {
            Ok(file) => {
                written.push(file.path);
                if let Some(report) = file.loudness { loudness.push(report); }
            },
            Err(f) => failed.push(f),
        }
    }
    // Pick the new or changed files up right away; each library folder once.
    if let Some(state) = app.try_state::<AppState>() {
        let mut parents: Vec<&Path> = written.iter().filter_map(|p| p.parent()).collect();
        parents.sort();
        parents.dedup();
        for p in parents {
            let _ = crate::editor_commands::rescan_containing(&state, p);
        }
    }
    Done {
        written: written.len(),
        failed,
        loudness,
        cancelled: CANCEL.load(Ordering::SeqCst),
        dir: dir.map(|d| d.to_string_lossy().into_owned()),
    }
}

#[allow(clippy::too_many_arguments)]
fn process_one(
    item: &Item,
    n: usize,
    total: usize,
    recipe: &Recipe,
    dir: Option<&Path>,
    backups: &Path,
    stamp: u64,
    taken: &Mutex<HashSet<PathBuf>>,
) -> Result<Written> {
    let in_place = dir.is_none();
    if in_place && !item.ext.eq_ignore_ascii_case("wav") {
        return Err(Error::Other("only WAV files can be replaced; save to a folder instead".into()));
    }
    let pcm = audio::decode(&item.path, MAX_SECONDS + 1.0)?;
    if pcm.frames() as f64 > MAX_SECONDS * pcm.sample_rate as f64 {
        return Err(Error::Other("longer than 30 minutes".into()));
    }
    let processed = process(pcm, recipe)?;
    let pcm = processed.pcm;
    let bits = recipe.bit_depth.or(item.bit_depth);
    let target = match dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)?;
            let stem = match &recipe.rename {
                Some(t) if !t.trim().is_empty() => render_name(t, &item.fields, n, total),
                _ => item.fields.name.clone(),
            };
            free_path(dir, &stem, taken)
        }
        None => {
            std::fs::create_dir_all(backups)?;
            let file = item.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            std::fs::copy(&item.path, backups.join(format!("{stamp}-{file}")))?;
            item.path.clone()
        }
    };
    // Through a temporary file so a failed write never truncates the target.
    let tmp = target.with_extension("wav.tmp");
    editor::write_wav(&tmp, &pcm, bits, None)?;
    std::fs::rename(&tmp, &target)?;
    let loudness = processed.loudness.map(|report| FileLoudness {
        name: item.fields.name.clone(), path: target.to_string_lossy().into_owned(),
        target: recipe.lufs.unwrap().target, report,
    });
    Ok(Written { path: target, loudness })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> Recipe {
        Recipe {
            trim: None,
            normalize: None,
            lufs: None,
            dc_offset: false,
            fade_in_ms: None,
            fade_out_ms: None,
            curve: Curve::Linear,
            channels: None,
            sample_rate: None,
            bit_depth: None,
            rename: None,
        }
    }

    #[test]
    fn trims_remixes_normalizes_and_fades() {
        // Mono at 1 kHz: 100 ms of silence, 200 ms at 0.25, 100 ms of silence.
        let mut samples = vec![0.0f32; 100];
        samples.extend(std::iter::repeat_n(0.25, 200));
        samples.extend(std::iter::repeat_n(0.0, 100));
        let pcm = Pcm { samples, channels: 1, sample_rate: 1000 };
        let r = Recipe {
            trim: Some(Trim { threshold_db: -40.0, pad_ms: 10.0 }),
            channels: Some(2),
            normalize: Some(0.0),
            fade_in_ms: Some(20.0),
            ..recipe()
        };
        let out = process(pcm, &r).unwrap().pcm;
        assert_eq!((out.channels, out.frames()), (2, 220), "200 ms of sound and 10 ms either side");
        // Normalized to full scale, faded in from silence, both channels alike.
        let peak = out.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 1.0).abs() < 1e-4);
        assert_eq!(out.samples[0], 0.0);
        assert!(out.samples.chunks(2).all(|f| f[0] == f[1]));

        // Down to mono and to twice the rate.
        let stereo = Pcm { samples: [0.2f32, 0.4].repeat(1000), channels: 2, sample_rate: 1000 };
        let out = process(stereo, &Recipe { channels: Some(1), sample_rate: Some(2000), ..recipe() }).unwrap().pcm;
        assert_eq!((out.channels, out.sample_rate), (1, 2000));
        assert!((out.frames() as i64 - 2000).abs() <= 2, "{}", out.frames());
        assert!((out.samples[1000] - 0.3).abs() < 0.01);
    }

    #[test]
    fn silent_audio_is_not_trimmed_away() {
        let pcm = Pcm { samples: vec![0.0; 50], channels: 1, sample_rate: 1000 };
        let out = process(pcm, &Recipe { trim: Some(Trim { threshold_db: -60.0, pad_ms: 0.0 }), ..recipe() }).unwrap().pcm;
        assert_eq!(out.frames(), 50);
    }

    #[test]
    fn names_from_templates() {
        let f = NameFields {
            name: "Kick 01".into(),
            category: Some("kick".into()),
            bpm: Some(127.6),
            key: Some("A minor".into()),
            kind: Some("oneshot".into()),
        };
        assert_eq!(render_name("{category}_{bpm}_{key}_{n}", &f, 3, 120), "kick_128_Aminor_003");
        // Missing values leave no doubled or dangling separators.
        let bare = NameFields { name: "Pad".into(), ..Default::default() };
        assert_eq!(render_name("{category}_{bpm}_{name}_{n}", &bare, 1, 5), "Pad_01");
        assert_eq!(render_name("{key} - {name}", &bare, 1, 5), "Pad");
        // Nothing left: the original name. Path characters go.
        assert_eq!(render_name("{bpm}", &bare, 1, 5), "Pad");
        assert_eq!(render_name("a/b:{name}", &bare, 1, 5), "abPad");
        // The name's own spaces and dots stay.
        let dotted = NameFields { name: "06. Mandrill fills".into(), ..Default::default() };
        assert_eq!(render_name("{name}_{n}", &dotted, 1, 3), "06. Mandrill fills_01");
        assert_eq!(render_name("{key}_{name}", &dotted, 1, 3), "06. Mandrill fills");
        // Unknown tokens vanish, an unclosed brace stays literal.
        assert_eq!(render_name("{name}{oops}_x{", &bare, 1, 5), "Pad_x{");
    }

    #[test]
    fn writes_copies_or_replaces_with_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Kick.wav");
        let mut samples = vec![0.0f32; 100];
        samples.extend(std::iter::repeat_n(0.5, 100));
        editor::write_wav(&src, &Pcm { samples, channels: 1, sample_rate: 1000 }, Some(16), None).unwrap();
        let item = Item {
            path: src.clone(),
            ext: "wav".into(),
            bit_depth: Some(16),
            fields: NameFields { name: "Kick".into(), category: Some("kick".into()), ..Default::default() },
        };
        let trimmed = Recipe { trim: Some(Trim { threshold_db: -40.0, pad_ms: 0.0 }), ..recipe() };
        let taken = Mutex::new(HashSet::new());
        let backups = dir.path().join("backups");

        // A renamed copy in another folder, at 24 bits; the original untouched.
        let out = dir.path().join("out");
        let r = Recipe { rename: Some("{category}_{n}".into()), bit_depth: Some(24), ..trimmed.clone() };
        let written = process_one(&item, 2, 10, &r, Some(&out), &backups, 1, &taken).unwrap().path;
        assert_eq!(written, out.join("kick_02.wav"));
        let spec = hound::WavReader::open(&written).unwrap();
        assert_eq!((spec.spec().bits_per_sample, spec.len()), (24, 100));
        assert_eq!(hound::WavReader::open(&src).unwrap().len(), 200);

        // In place: same path, trimmed, the original kept in backups.
        let written = process_one(&item, 1, 1, &trimmed, None, &backups, 7, &taken).unwrap().path;
        assert_eq!(written, src);
        assert_eq!(hound::WavReader::open(&src).unwrap().len(), 100);
        assert_eq!(hound::WavReader::open(backups.join("7-Kick.wav")).unwrap().len(), 200);

        // Anything but WAV can't be replaced.
        let mp3 = Item { ext: "mp3".into(), ..item };
        assert!(process_one(&mp3, 1, 1, &trimmed, None, &backups, 8, &taken).is_err());
    }

    #[test]
    fn reads_names_from_the_library() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO sources (id, path, added_at) VALUES (1, '/lib', 0)", []).unwrap();
        conn.execute(
            "INSERT INTO samples (id, source_id, path, name, ext, size, mtime, bit_depth, bpm, musical_key, category, kind)
             VALUES (7, 1, '/lib/Kick.wav', 'Kick', 'wav', 0, 0, 24, 120.0, 'A minor', 'kick', 'oneshot')",
            [],
        )
        .unwrap();
        // Unknown ids are left out.
        let found = items(&conn, &[7, 99]).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].bit_depth, Some(24));
        assert_eq!(render_name("{category}_{bpm}_{key}_{kind}", &found[0].fields, 1, 1), "kick_120_Aminor_oneshot");
    }

    #[test]
    fn free_names_skip_taken_ones() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Kick.wav"), b"").unwrap();
        let taken = Mutex::new(HashSet::new());
        assert_eq!(free_path(dir.path(), "Kick", &taken).file_name().unwrap(), "Kick 2.wav");
        assert_eq!(free_path(dir.path(), "Kick", &taken).file_name().unwrap(), "Kick 3.wav");
    }
    fn sine(seconds: f64, amplitude: f32) -> Pcm {
        let samples = (0..(seconds * 48000.0) as usize)
            .map(|i| amplitude * (std::f32::consts::TAU * 440.0 * i as f32 / 48000.0).sin()).collect();
        Pcm { samples, channels: 1, sample_rate: 48000 }
    }

    fn measured(pcm: &Pcm) -> f64 {
        let mut meter = ebur128::EbuR128::new(pcm.channels as u32, pcm.sample_rate, ebur128::Mode::I).unwrap();
        meter.add_frames_f32(&pcm.samples).unwrap();
        meter.loudness_global().unwrap()
    }

    #[test]
    fn old_recipes_keep_peak_normalization_and_default_new_fields_off() {
        let r: Recipe = serde_json::from_str(r#"{"normalize":-6.0}"#).unwrap();
        assert!(!r.dc_offset && r.lufs.is_none());
        let output = process(sine(1.0, 0.1), &r).unwrap();
        assert!(output.loudness.is_none());
        let peak = output.pcm.samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!((20.0 * peak.log10() + 6.0).abs() < 1e-3);
    }

    #[test]
    fn dc_removal_is_per_channel_and_can_run_without_normalization() {
        let input = Pcm { samples: vec![0.5, -0.5, 0.7, -0.3], channels: 2, sample_rate: 48000 };
        let output = process(input, &Recipe { dc_offset: true, ..recipe() }).unwrap();
        assert!(output.loudness.is_none());
        for (actual, expected) in output.pcm.samples.iter().zip([-0.1f32, -0.1, 0.1, 0.1]) {
            assert!((*actual - expected).abs() < 1e-6);
        }
        assert_eq!(output.pcm.frames(), 2);
    }

    #[test]
    fn lufs_is_measured_on_final_audio_after_fades_channels_and_resampling() {
        let r = Recipe {
            lufs: Some(Lufs { target: -18.0, ceiling: -1.0 }),
            channels: Some(2), sample_rate: Some(44100),
            fade_in_ms: Some(500.0), fade_out_ms: Some(500.0), ..recipe()
        };
        let output = process(sine(2.0, 0.05), &r).unwrap();
        let report = output.loudness.unwrap();
        assert!(!report.limited);
        assert_eq!((output.pcm.channels, output.pcm.sample_rate), (2, 44100));
        assert!((measured(&output.pcm) + 18.0).abs() < 0.1);
        assert!((measured(&output.pcm) - report.after).abs() < 1e-6);
        let peak = output.pcm.samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(20.0 * peak.log10() <= -1.0 + 1e-4);
    }

    #[test]
    fn written_files_report_peak_limited_loudness_and_preserve_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Transient.wav");
        let mut input = sine(1.0, 0.005);
        input.samples[24000] = 1.0;
        editor::write_wav(&src, &input, Some(32), None).unwrap();
        let original = std::fs::read(&src).unwrap();
        let item = Item { path: src.clone(), ext: "wav".into(), bit_depth: Some(32), fields: NameFields { name: "Transient".into(), ..Default::default() } };
        let out = dir.path().join("out");
        let written = process_one(&item, 1, 1, &Recipe { lufs: Some(Lufs { target: -5.0, ceiling: -1.0 }), bit_depth: Some(24), ..recipe() },
            Some(&out), &dir.path().join("backups"), 1, &Mutex::new(HashSet::new())).unwrap();
        let report = written.loudness.unwrap();
        assert!(report.report.limited);
        assert!(report.report.after < report.target);
        let saved = audio::decode(&written.path, 10.0).unwrap();
        assert!((measured(&saved) - report.report.after).abs() < 0.05);
        let peak = saved.samples.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        assert!(20.0 * peak.log10() <= -1.0 + 1e-3);
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["name"], "Transient");
        assert_eq!(json["limited"], true);
        assert_eq!(json["target"], -5.0);
        assert_eq!(json["path"], written.path.to_string_lossy().as_ref());
        assert_eq!(std::fs::read(&src).unwrap(), original);
    }

    #[test]
    fn unmeasurable_lufs_and_invalid_recipes_do_not_replace_files() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        let r = Recipe { lufs: Some(Lufs { target: -14.0, ceiling: -0.3 }), ..recipe() };
        for (name, input) in [("Short", sine(0.1, 0.2)), ("Silent", sine(1.0, 0.0))] {
            let path = dir.path().join(format!("{name}.wav"));
            editor::write_wav(&path, &input, Some(32), None).unwrap();
            let original = std::fs::read(&path).unwrap();
            let item = Item { path: path.clone(), ext: "wav".into(), bit_depth: Some(32), fields: NameFields { name: name.into(), ..Default::default() } };
            assert!(process_one(&item, 1, 1, &r, None, &backups, 1, &Mutex::new(HashSet::new())).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
        assert!(!backups.exists());
        assert!(Recipe { normalize: Some(0.0), ..r.clone() }.validate().is_err());
        assert!(Recipe { lufs: Some(Lufs { target: -4.0, ceiling: -0.3 }), ..recipe() }.validate().is_err());
        assert!(process(sine(1.0, 0.2), &r).is_ok(), "an unmeasurable file does not poison subsequent processing");
    }

}
