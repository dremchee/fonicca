//! Per-sample audio analysis: loudness, one-shot/loop classification, BPM and key.

use std::path::Path;
use std::sync::LazyLock;

use ebur128::{EbuR128, Mode};
use regex::Regex;
use serde::Serialize;

use crate::audio;
use crate::error::Result;

/// Bump to re-analyze the whole library after changing the algorithms below.
pub const ANALYSIS_VERSION: i64 = 1;

/// Only the beginning of long files is decoded; enough for tempo, key and loudness.
const MAX_SECONDS: f64 = 45.0;
/// Tempo detection cost grows with length (~2.5 s per 2 minutes); 30 s is plenty.
const DETECT_SECONDS: f64 = 30.0;

/// Up to `DETECT_SECONDS` of audio, taken after the intro of long files.
fn detection_window(mono: &[f32], sample_rate: u32) -> &[f32] {
    let window = (DETECT_SECONDS * sample_rate as f64) as usize;
    if mono.len() <= window {
        return mono;
    }
    let start = (mono.len() - window).min(mono.len() / 4);
    &mono[start..start + window]
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    OneShot,
    Loop,
    /// Songs, stems and long recordings.
    Long,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::OneShot => "oneshot",
            Kind::Loop => "loop",
            Kind::Long => "long",
        }
    }
}

/// Where a BPM or key value came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Name,
    Audio,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Name => "name",
            Source::Audio => "audio",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub peak_db: Option<f64>,
    pub lufs: Option<f64>,
    pub kind: Kind,
    pub bpm: Option<(f64, Source)>,
    pub key: Option<(String, Source)>,
}

// --- Name parsing ------------------------------------------------------------

static BPM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[^\d.])(\d{2,3}(?:[.,]\d)?)\s*[-_ ]?bpm|bpm\s*[-_ ]?(\d{2,3}(?:[.,]\d)?)").unwrap()
});

/// BPM written in the file name, e.g. "Pad 120bpm", "Loop 94.0 BPM", "TOMS bpm85".
pub fn bpm_from_name(name: &str) -> Option<f64> {
    let caps = BPM_RE.captures(name)?;
    let raw = caps.get(1).or_else(|| caps.get(2))?.as_str().replace(',', ".");
    let bpm: f64 = raw.parse().ok()?;
    (50.0..=250.0).contains(&bpm).then_some(bpm)
}

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

static KEY_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([A-Ga-g])(#|♯|b|♭|sharp|flat)?(m|min|minor|maj|major|M)?$").unwrap()
});

fn note_index(letter: char) -> i32 {
    match letter.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    }
}

fn key_name(pitch: i32, minor: bool) -> String {
    format!("{}{}", NOTES[pitch.rem_euclid(12) as usize], if minor { "m" } else { "" })
}

/// Key written in the file name, e.g. "Bass Am 120bpm", "Pad_F#min", "Lead key C".
/// A bare note letter only counts after the word "key", since "Loop A" or
/// "Kick B" usually name variations rather than keys.
pub fn key_from_name(name: &str) -> Option<String> {
    let tokens: Vec<&str> =
        name.split(|c: char| c.is_whitespace() || "_-.()[]{},".contains(c)).filter(|t| !t.is_empty()).collect();
    for (i, token) in tokens.iter().enumerate() {
        let Some(c) = KEY_TOKEN_RE.captures(token) else { continue };
        let letter = c[1].chars().next().unwrap();
        let accidental = c.get(2).map(|m| m.as_str());
        let mode = c.get(3).map(|m| m.as_str());
        let after_key_word = i > 0 && tokens[i - 1].eq_ignore_ascii_case("key");
        if accidental.is_none() && mode.is_none() && !after_key_word {
            continue;
        }
        // Lowercase letters only with a marker ("am", "c#"), to skip words like "a" or "be".
        if letter.is_ascii_lowercase() && accidental.is_none() && mode.is_none() {
            continue;
        }
        // "Bb"/"Eb" are flats, but a lone lowercase "b" after a lowercase letter
        // ("db") is ambiguous with words; regex already limits tokens to 1–3 parts.
        let shift = match accidental {
            Some("#" | "♯" | "sharp") => 1,
            Some("b" | "♭" | "flat") => -1,
            _ => 0,
        };
        let minor = matches!(mode, Some("m" | "min" | "minor"));
        return Some(key_name(note_index(letter) + shift, minor));
    }
    None
}

static LOOP_HINT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bloops?\b|\blp\b|bpm").unwrap());
static ONESHOT_HINT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)one[\s_-]?shot|\bhit\b|\bshot\b").unwrap());

// --- Signal analysis ---------------------------------------------------------

fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

/// Envelope features used to tell one-shots from loops.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Envelope {
    /// Average level of the first and last quarter relative to the peak level.
    pub head_level: f32,
    pub tail_level: f32,
    /// Number of distinct attacks (transients).
    pub onsets: usize,
    /// Position of the last attack as a fraction of the length.
    pub last_onset: f32,
}

pub fn envelope(mono: &[f32], sample_rate: u32) -> Envelope {
    let frame = (sample_rate as usize / 100).max(1); // 10 ms
    let env: Vec<f32> = mono.chunks(frame).map(rms).collect();
    if env.len() < 8 {
        return Envelope::default();
    }
    let max = env.iter().copied().fold(0.0f32, f32::max);
    if max <= 1e-6 {
        return Envelope::default();
    }

    let quarter = (env.len() / 4).max(1);
    let mean = |x: &[f32]| x.iter().sum::<f32>() / x.len() as f32 / max;
    let head_level = mean(&env[..quarter]);
    let tail_level = mean(&env[env.len() - quarter..]);

    // An attack: a frame much louder than the previous 50 ms, at least 80 ms after the last one.
    // A sound that starts loud counts as an attack at 0.
    let starts_loud = env[0] > max * 0.15;
    let (mut onsets, mut last, mut last_onset) = (starts_loud as usize, starts_loud.then_some(0usize), 0usize);
    for i in 1..env.len() {
        let from = i.saturating_sub(5);
        let before = env[from..i].iter().sum::<f32>() / (i - from) as f32;
        let rising = env[i] > before * 1.8 + max * 0.02 && env[i] > max * 0.15;
        if rising && last.is_none_or(|l| i - l >= 8) {
            onsets += 1;
            last = Some(i);
            last_onset = i;
        }
    }
    Envelope { head_level, tail_level, onsets, last_onset: last_onset as f32 / env.len() as f32 }
}

pub fn classify(name: &str, duration: f64, env: Envelope) -> Kind {
    if ONESHOT_HINT_RE.is_match(name) {
        return Kind::OneShot;
    }
    if LOOP_HINT_RE.is_match(name) {
        return if duration > 60.0 { Kind::Long } else { Kind::Loop };
    }
    if duration < 1.0 {
        return Kind::OneShot;
    }
    if duration > 45.0 {
        return Kind::Long;
    }
    // Several attacks spread over the length: a groove. Otherwise a sound that is
    // loud from start to end (pads, drones) still loops; decaying hits, risers and
    // reversed cymbals don't.
    let rhythmic = env.onsets >= 3 && env.last_onset >= 0.5;
    let sustained = env.head_level >= 0.25 && env.tail_level >= 0.25;
    if rhythmic || sustained { Kind::Loop } else { Kind::OneShot }
}

/// Tempi that fit a whole number of beats into a loop of `duration` seconds.
fn length_candidates(duration: f64) -> Vec<f64> {
    [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]
        .iter()
        .map(|beats| 60.0 * beats / duration)
        .filter(|bpm| (55.0..=185.0).contains(bpm))
        .collect()
}

fn round_bpm(bpm: f64) -> f64 {
    (bpm * 10.0).round() / 10.0
}

/// Estimates a loop's tempo. Loops are cut to whole beats, so the length gives
/// the tempo up to a factor of two; the detector (when confident) picks the
/// octave, otherwise the candidate in the most common 70–140 range wins.
/// Detection alone is unreliable on loops of a few seconds.
pub fn loop_bpm(duration: f64, detected: Option<(f64, f64)>) -> Option<f64> {
    let candidates = length_candidates(duration);
    if let Some((bpm, confidence)) = detected {
        if confidence >= 0.3 {
            for factor in [1.0, 0.5, 2.0] {
                let hit = candidates
                    .iter()
                    .map(|&c| (c, ((bpm * factor) - c).abs() / c))
                    .filter(|&(_, err)| err < 0.04)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((c, _)) = hit {
                    return Some(round_bpm(c));
                }
            }
        }
    }
    candidates
        .iter()
        .copied()
        .find(|c| (70.0..140.0).contains(c))
        .or_else(|| candidates.iter().copied().min_by(|a, b| (a - 100.0).abs().total_cmp(&(b - 100.0).abs())))
        .map(round_bpm)
}

fn loudness(pcm: &audio::Pcm) -> (Option<f64>, Option<f64>) {
    let Ok(mut meter) = EbuR128::new(pcm.channels as u32, pcm.sample_rate, Mode::I | Mode::SAMPLE_PEAK) else {
        return (None, None);
    };
    if meter.add_frames_f32(&pcm.samples).is_err() {
        return (None, None);
    }
    let peak = (0..pcm.channels as u32)
        .filter_map(|c| meter.sample_peak(c).ok())
        .fold(0.0f64, f64::max);
    let peak_db = (peak > 0.0).then(|| 20.0 * peak.log10());
    // Integrated loudness needs at least one 400 ms block.
    let lufs = meter.loudness_global().ok().filter(|l| l.is_finite());
    (peak_db, lufs)
}

/// Key of a tonal sound (bass, keys, pads …). Only called once the category says
/// the sound is tonal: the detector names a key for anything, drums included.
pub fn key_from_audio(mono: &[f32], sample_rate: u32) -> Option<String> {
    if (mono.len() as f64) < 2.0 * sample_rate as f64 {
        return None;
    }
    let window = detection_window(mono, sample_rate);
    stratum_dsp::analyze_audio(window, sample_rate, stratum_dsp::AnalysisConfig::default())
        .ok()
        .map(|r| r.key.name())
}

pub fn analyze(path: &Path, name: &str) -> Result<Analysis> {
    let pcm = audio::decode(path, MAX_SECONDS)?;
    let mono = pcm.mono();
    let duration = pcm.frames() as f64 / pcm.sample_rate as f64;
    let (peak_db, lufs) = loudness(&pcm);
    let kind = classify(name, duration, envelope(&mono, pcm.sample_rate));

    let mut bpm = bpm_from_name(name).map(|b| (b, Source::Name));
    let key = key_from_name(name).map(|k| (k, Source::Name));

    // Key comes from the name only: the detector's key confidence is unusable, and
    // without knowing the instrument a drum loop would get a meaningless key.
    // Audio key detection returns once categories can tell tonal sounds apart.
    if kind != Kind::OneShot && bpm.is_none() {
        let window = detection_window(&mono, pcm.sample_rate);
        let detected = stratum_dsp::analyze_audio(window, pcm.sample_rate, stratum_dsp::AnalysisConfig::default())
            .ok()
            .filter(|r| r.bpm > 0.0)
            .map(|r| (r.bpm as f64, r.bpm_confidence as f64));
        bpm = match kind {
            Kind::Loop if duration <= 30.0 => loop_bpm(duration, detected),
            _ => detected.filter(|&(_, c)| c >= 0.3).map(|(b, _)| round_bpm(b)),
        }
        .map(|b| (b, Source::Audio));
    }

    Ok(Analysis { peak_db, lufs, kind, bpm, key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bpm_from_names() {
        for (name, bpm) in [
            ("Trashcans 93bpm", Some(93.0)),
            ("luny_tunes_LOOP 94.0 BPM", Some(94.0)),
            ("German Nard 89.6bpm", Some(89.6)),
            ("14-TOM bpm85 001 0002", Some(85.0)),
            ("Drums_120_BPM_Dry", Some(120.0)),
            ("Pad C0 120bpm", Some(120.0)),
            ("Kick 909", None),
            ("Loop 1000bpm", None),
        ] {
            assert_eq!(bpm_from_name(name), bpm, "{name}");
        }
    }

    #[test]
    fn parses_keys_from_names() {
        for (name, key) in [
            ("Bass Am 120bpm", Some("Am")),
            ("Pad_F#min_90", Some("F#m")),
            ("Lead Bb maj", Some("A#")),
            ("Chord Dbm", Some("C#m")),
            ("Piano key C", Some("C")),
            ("Keys Cmaj7", None),
            ("Drum Loop A 120bpm", None),
            ("Kick B", None),
            ("a day in the life", None),
            ("synth em", Some("Em")),
            ("Pad C0 120bpm", None),
        ] {
            assert_eq!(key_from_name(name).as_deref(), key, "{name}");
        }
    }

    #[test]
    fn classifies_by_name_length_and_envelope() {
        let hit = Envelope { head_level: 0.4, tail_level: 0.01, onsets: 1, last_onset: 0.0 };
        let groove = Envelope { head_level: 0.3, tail_level: 0.2, onsets: 6, last_onset: 0.8 };
        let pad = Envelope { head_level: 0.6, tail_level: 0.7, onsets: 1, last_onset: 0.0 };
        let riser = Envelope { head_level: 0.05, tail_level: 0.8, onsets: 0, last_onset: 0.0 };
        assert_eq!(classify("Kick 01", 0.5, groove), Kind::OneShot);
        assert_eq!(classify("Drum Loop", 4.0, hit), Kind::Loop);
        assert_eq!(classify("Tapfeet 65bpm", 7.4, hit), Kind::Loop);
        assert_eq!(classify("Crash One Shot", 4.0, groove), Kind::OneShot);
        assert_eq!(classify("Long Crash", 3.0, hit), Kind::OneShot);
        assert_eq!(classify("Groove 3", 3.0, groove), Kind::Loop);
        assert_eq!(classify("Warm Pad", 3.0, pad), Kind::Loop);
        assert_eq!(classify("Reverse Crash", 2.0, riser), Kind::OneShot);
        assert_eq!(classify("Barry White - I Found Someone", 220.0, groove), Kind::Long);
    }

    fn sine_bursts(sample_rate: u32, seconds: f32, bursts: &[f32], decay: f32) -> Vec<f32> {
        let n = (sample_rate as f32 * seconds) as usize;
        let mut out = vec![0.0f32; n];
        for &start in bursts {
            let s0 = (start * sample_rate as f32) as usize;
            for i in s0..n {
                let t = (i - s0) as f32 / sample_rate as f32;
                out[i] += (t * 220.0 * std::f32::consts::TAU).sin() * (-t * decay).exp() * 0.5;
            }
        }
        out
    }

    #[test]
    fn envelope_counts_attacks() {
        let groove = envelope(&sine_bursts(44100, 2.0, &[0.0, 0.5, 1.0, 1.5], 12.0), 44100);
        assert_eq!(groove.onsets, 4);
        assert!(groove.last_onset > 0.7);
        let hit = envelope(&sine_bursts(44100, 2.0, &[0.0], 6.0), 44100);
        assert_eq!(hit.onsets, 1);
        assert!(hit.tail_level < 0.05);
    }

    #[test]
    fn estimates_loop_tempo_from_length() {
        // 4 beats in 2.57 s ≈ 93.4 BPM; the 8-beat candidate (186.8) is out of range.
        assert_eq!(loop_bpm(2.57, None), Some(93.4));
        // 4 s fits 60 or 120: without detection prefer 70–140, with it follow detection.
        assert_eq!(loop_bpm(4.0, None), Some(120.0));
        assert_eq!(loop_bpm(4.0, Some((61.0, 0.8))), Some(60.0));
        assert_eq!(loop_bpm(4.0, Some((241.0, 0.8))), Some(120.0));
        // Unconfident detection is ignored.
        assert_eq!(loop_bpm(4.0, Some((61.0, 0.1))), Some(120.0));
    }

    /// Accuracy report against the BPM written in file names:
    /// `ANALYSIS_DIR=/path/to/loops cargo test --release -- --ignored --nocapture bpm_accuracy`
    #[test]
    #[ignore]
    fn bpm_accuracy() {
        let dir = std::env::var("ANALYSIS_DIR").expect("set ANALYSIS_DIR");
        let (mut total, mut exact, mut octave) = (0, 0, 0);
        let t = std::time::Instant::now();
        for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !audio::is_audio_file(path) {
                continue;
            }
            let name = path.file_stem().unwrap().to_string_lossy();
            let Some(truth) = bpm_from_name(&name) else { continue };
            // Analyze as if the name carried no tempo.
            let anon = BPM_RE.replace_all(&name, "").into_owned();
            let Ok(a) = analyze(path, &anon) else { continue };
            total += 1;
            match a.bpm {
                Some((b, _)) if (b - truth).abs() <= 2.0 => exact += 1,
                Some((b, _)) if (b * 2.0 - truth).abs() <= 2.0 || (b / 2.0 - truth).abs() <= 2.0 => octave += 1,
                other => println!("{name}: truth {truth}, got {other:?} ({:?})", a.kind),
            }
        }
        println!("{total} files: {exact} within ±2 BPM, {octave} half/double, in {:?}", t.elapsed());
    }

    /// How many drum hits longer than 1 s are mistaken for loops:
    /// `ANALYSIS_DIR=… cargo test --release -- --ignored --nocapture oneshot_precision`
    #[test]
    #[ignore]
    fn oneshot_precision() {
        let dir = std::env::var("ANALYSIS_DIR").expect("set ANALYSIS_DIR");
        let hit = Regex::new(r"(?i)\b(kick|bd|snare|sd|clap|crash|ride|cymbal|tom|perc|rim)\b").unwrap();
        let (mut total, mut loops) = (0, 0);
        for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if !audio::is_audio_file(path) || !hit.is_match(&name) || LOOP_HINT_RE.is_match(&name) {
                continue;
            }
            let Ok(pcm) = audio::decode(path, 30.0) else { continue };
            let duration = pcm.frames() as f64 / pcm.sample_rate as f64;
            if duration < 1.0 {
                continue;
            }
            total += 1;
            let env = envelope(&pcm.mono(), pcm.sample_rate);
            if classify(&name, duration, env) == Kind::Loop {
                loops += 1;
                if loops <= 15 {
                    println!("{name}: {duration:.1}s {env:?}");
                }
            }
        }
        println!("{total} drum hits ≥ 1 s: {loops} classified as loops");
    }
}

#[cfg(test)]
mod debug {
    /// `ANALYSIS_FILES="a.wav:b.wav" cargo test --release -- --ignored --nocapture stratum_raw`
    #[test]
    #[ignore]
    fn stratum_raw() {
        for p in std::env::var("ANALYSIS_FILES").unwrap().split(':') {
            let pcm = crate::audio::decode(std::path::Path::new(p), 120.0).unwrap();
            let mono = pcm.mono();
            let dur = pcm.frames() as f64 / pcm.sample_rate as f64;
            match stratum_dsp::analyze_audio(&mono, pcm.sample_rate, stratum_dsp::AnalysisConfig::default()) {
                Ok(r) => {
                    let c = stratum_dsp::compute_confidence(&r);
                    println!("{p}\n  dur {dur:.2}s bpm {:.2} conf {:.2}/{:.2} key {} conf {:.2}/{:.2} clarity {:.2}", r.bpm, r.bpm_confidence, c.bpm_confidence, r.key.name(), r.key_confidence, c.key_confidence, r.key_clarity)
                }
                Err(e) => println!("{p}\n  dur {dur:.2}s error {e}"),
            }
        }
    }
}

#[cfg(test)]
mod timing {
    /// `ANALYSIS_FILES="a:b" cargo test --release -- --ignored --nocapture analysis_timing`
    #[test]
    #[ignore]
    fn analysis_timing() {
        for p in std::env::var("ANALYSIS_FILES").unwrap().split(':') {
            let path = std::path::Path::new(p);
            let t = std::time::Instant::now();
            let pcm = crate::audio::decode(path, super::MAX_SECONDS).unwrap();
            let t_decode = t.elapsed();
            let t = std::time::Instant::now();
            let _ = super::loudness(&pcm);
            let t_loud = t.elapsed();
            let t = std::time::Instant::now();
            let mono = pcm.mono();
            let _ = stratum_dsp::analyze_audio(super::detection_window(&mono, pcm.sample_rate), pcm.sample_rate, stratum_dsp::AnalysisConfig::default());
            let t_stratum = t.elapsed();
            println!("{p}\n  decode {t_decode:?} loudness {t_loud:?} stratum {t_stratum:?}");
        }
    }
}
