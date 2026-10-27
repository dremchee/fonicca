//! Single-file audio editor. Edits are kept as a list of operations on top of
//! the decoded original, so undo is unlimited and the file on disk is untouched
//! until saved.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::audio::{self, Pcm};
use crate::error::{Error, Result};

pub mod clipboard;
mod loops;
mod markers;
mod slices;
pub mod processing;
pub mod saving;
pub mod spectrogram;
pub use loops::{zero_crossing, Loop};
pub use markers::{append_markers, Marker};
pub use slices::{slices, SliceBy, Span};

/// Files longer than this are not opened (the whole file is kept in memory).
const MAX_SECONDS: f64 = 20.0 * 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Curve {
    Linear,
    /// Equal-power (sine) shape, sounds even for crossfades.
    Smooth,
    /// Fast start or end, for natural-sounding decays.
    Exponential,
}

impl Curve {
    /// Gain for position `t` in 0..=1 of a fade in.
    fn gain(self, t: f32) -> f32 {
        match self {
            Curve::Linear => t,
            Curve::Smooth => (t * std::f32::consts::FRAC_PI_2).sin(),
            Curve::Exponential => t * t,
        }
    }
}

/// One edit. Ranges are in frames of the buffer as it is when the edit applies;
/// `end` is exclusive.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    /// Keep only the range.
    Crop { start: usize, end: usize },
    /// Remove the range.
    Delete { start: usize, end: usize },
    Silence { start: usize, end: usize },
    FadeIn { start: usize, end: usize, curve: Curve },
    FadeOut { start: usize, end: usize, curve: Curve },
    Gain { start: usize, end: usize, db: f32 },
    /// Scale the range so its peak hits `peak_db`.
    Normalize { start: usize, end: usize, peak_db: f32 },
    Reverse { start: usize, end: usize },
    /// Subtract each channel's mean over the range; no filtering or peak scaling.
    DcOffset { start: usize, end: usize },
    /// Loop points for samplers (the WAV's `smpl` chunk); the audio is untouched.
    SetLoop { start: usize, end: usize },
    ClearLoop,
    /// Slice markers, all at once (cleaned up when applied); the audio is untouched.
    SetMarkers { markers: Vec<Marker> },
    /// Rendered pitch/time edit. Unlike paste, positions inside the range scale with it.
    #[serde(skip)]
    Stretch { start: usize, end: usize, samples: std::sync::Arc<Vec<f32>> },
    /// Paste in this buffer's format; `end == start` inserts. Built on the backend.
    #[serde(skip)]
    Replace { start: usize, end: usize, samples: std::sync::Arc<Vec<f32>> },
}

impl Op {
    fn changes_audio(&self) -> bool {
        !matches!(self, Op::SetLoop { .. } | Op::ClearLoop | Op::SetMarkers { .. })
    }

    fn range(&self) -> (usize, usize) {
        match *self {
            Op::Crop { start, end }
            | Op::Delete { start, end }
            | Op::Silence { start, end }
            | Op::FadeIn { start, end, .. }
            | Op::FadeOut { start, end, .. }
            | Op::Gain { start, end, .. }
            | Op::Normalize { start, end, .. }
            | Op::Reverse { start, end }
            | Op::DcOffset { start, end }
            | Op::SetLoop { start, end }
            | Op::Stretch { start, end, .. }
            | Op::Replace { start, end, .. } => (start, end),
            Op::ClearLoop | Op::SetMarkers { .. } => (0, 0),
        }
    }
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Applies one operation in place. Ranges are clamped to the buffer.
pub fn apply(pcm: &mut Pcm, op: &Op) {
    let ch = pcm.channels.max(1);
    let frames = pcm.frames();
    let (start, end) = op.range();
    let (start, end) = (start.min(frames), end.min(frames).max(start.min(frames)));
    let len = end - start;

    match *op {
        Op::SetLoop { .. } | Op::ClearLoop | Op::SetMarkers { .. } => {}
        Op::Replace { ref samples, .. } | Op::Stretch { ref samples, .. } => {
            pcm.samples.splice(start * ch..end * ch, samples.iter().copied());
        }
        Op::Crop { .. } => {
            pcm.samples.truncate(end * ch);
            pcm.samples.drain(..start * ch);
        }
        Op::Delete { .. } => {
            pcm.samples.drain(start * ch..end * ch);
        }
        Op::Silence { .. } => pcm.samples[start * ch..end * ch].fill(0.0),
        Op::FadeIn { curve, .. } | Op::FadeOut { curve, .. } => {
            let fade_in = matches!(op, Op::FadeIn { .. });
            for (i, frame) in pcm.samples[start * ch..end * ch].chunks_mut(ch).enumerate() {
                let t = if len > 1 { i as f32 / (len - 1) as f32 } else { 1.0 };
                let g = curve.gain(if fade_in { t } else { 1.0 - t });
                frame.iter_mut().for_each(|s| *s *= g);
            }
        }
        Op::Gain { db, .. } => {
            let g = db_to_gain(db);
            pcm.samples[start * ch..end * ch].iter_mut().for_each(|s| *s *= g);
        }
        Op::Normalize { peak_db, .. } => {
            let peak = pcm.samples[start * ch..end * ch].iter().fold(0.0f32, |m, s| m.max(s.abs()));
            if peak > 1e-9 {
                let g = db_to_gain(peak_db) / peak;
                pcm.samples[start * ch..end * ch].iter_mut().for_each(|s| *s *= g);
            }
        }
        Op::DcOffset { .. } => {
            if len > 0 {
                let range = &mut pcm.samples[start * ch..end * ch];
                // Accumulate in f64 so long files do not lose small offsets.
                let mut means = vec![0.0f64; ch];
                for frame in range.chunks_exact(ch) {
                    for (mean, sample) in means.iter_mut().zip(frame) {
                        *mean += *sample as f64;
                    }
                }
                means.iter_mut().for_each(|mean| *mean /= len as f64);
                for frame in range.chunks_exact_mut(ch) {
                    for (sample, mean) in frame.iter_mut().zip(&means) {
                        *sample = (*sample as f64 - mean) as f32;
                    }
                }
            }
        }
        Op::Reverse { .. } => {
            let range = &mut pcm.samples[start * ch..end * ch];
            let n = range.len() / ch;
            for i in 0..n / 2 {
                for c in 0..ch {
                    range.swap(i * ch + c, (n - 1 - i) * ch + c);
                }
            }
        }
    }
}

// Unique across sessions, so reopening a file invalidates an in-flight render too.
static EDIT_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_revision() -> u64 {
    EDIT_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// An open file with its edit history.
pub struct Session {
    revision: u64,
    audio_revision: u64,
    spectrum: Option<std::sync::Arc<spectrogram::Spectrogram>>,
    pub sample_id: i64,
    pub path: PathBuf,
    pub original_path: PathBuf,
    pub has_saved_target: bool,
    pub bit_depth: Option<u32>,
    original: Pcm,
    current: Pcm,
    /// Loop points read from the file, and after the edits.
    original_loop: Option<Loop>,
    current_loop: Option<Loop>,
    /// Slice markers read from the file, and after the edits.
    original_markers: Vec<Marker>,
    current_markers: Vec<Marker>,
    ops: Vec<Op>,
    redo: Vec<Op>,
    /// Ops at the last save, i.e. what is on disk.
    saved: Vec<Op>,
    /// Peaks of `current` for wide wave requests, built on first use after each change.
    pyramid: OnceLock<Pyramid>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub sample_id: i64,
    pub frames: usize,
    pub sample_rate: u32,
    pub channels: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub dirty: bool,
    pub has_saved_target: bool,
    pub can_replace_original: bool,
    pub original_path: String,
    #[serde(rename = "loop")]
    pub loop_points: Option<Loop>,
    pub markers: Vec<Marker>,
}

impl Session {
    pub fn open(sample_id: i64, path: &Path, bit_depth: Option<u32>) -> Result<Self> {
        let pcm = audio::decode(path, MAX_SECONDS + 1.0)?;
        if pcm.frames() as f64 > MAX_SECONDS * pcm.sample_rate as f64 {
            return Err(Error::Other("files longer than 20 minutes cannot be edited".into()));
        }
        let frames = pcm.frames();
        let original_loop = loops::read_smpl_loop(path)
            .map(|l| Loop { start: l.start.min(frames), end: l.end.min(frames) })
            .filter(|l| l.end > l.start);
        let original_markers = markers::normalized(markers::read_markers(path), frames);
        Ok(Self {
            revision: next_revision(),
            audio_revision: next_revision(),
            spectrum: None,
            sample_id,
            path: path.to_path_buf(),
            original_path: path.to_path_buf(),
            has_saved_target: false,
            bit_depth,
            current: Pcm { samples: pcm.samples.clone(), channels: pcm.channels, sample_rate: pcm.sample_rate },
            original: pcm,
            original_loop,
            current_loop: original_loop,
            current_markers: original_markers.clone(),
            original_markers,
            ops: Vec::new(),
            redo: Vec::new(),
            saved: Vec::new(),
            pyramid: OnceLock::new(),
        })
    }

    pub fn state(&self) -> State {
        State {
            sample_id: self.sample_id,
            frames: self.current.frames(),
            sample_rate: self.current.sample_rate,
            channels: self.current.channels,
            can_undo: !self.ops.is_empty(),
            can_redo: !self.redo.is_empty(),
            dirty: self.ops != self.saved,
            has_saved_target: self.has_saved_target,
            can_replace_original: self.original_path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("wav")),
            original_path: self.original_path.to_string_lossy().into_owned(),
            loop_points: self.current_loop,
            markers: self.current_markers.clone(),
        }
    }

    pub fn revision(&self) -> u64 { self.revision }

    pub fn apply_processed(&mut self, revision: u64, op: Op) -> Result<()> {
        if self.revision != revision {
            return Err(Error::Other("the audio changed during processing; please try again".into()));
        }
        self.apply(op);
        Ok(())
    }

    pub fn audio_revision(&self) -> u64 { self.audio_revision }

    pub fn spectrogram(&self) -> Option<std::sync::Arc<spectrogram::Spectrogram>> {
        self.spectrum.clone()
    }

    pub fn cache_spectrogram(&mut self, revision: u64, data: std::sync::Arc<spectrogram::Spectrogram>) -> Result<()> {
        if self.audio_revision != revision {
            return Err(Error::Other("the audio changed during spectrogram analysis".into()));
        }
        self.spectrum = Some(data);
        Ok(())
    }

    fn invalidate_spectrogram(&mut self, op: &Op) {
        if op.changes_audio() {
            self.audio_revision = next_revision();
            self.spectrum = None;
        }
    }

    pub fn markers(&self) -> &[Marker] {
        &self.current_markers
    }

    pub fn audio(&self) -> &Pcm {
        &self.current
    }

    pub fn loop_points(&self) -> Option<Loop> {
        self.current_loop
    }

    fn apply_op(&mut self, op: &Op) {
        self.current_loop = loops::moved_loop(self.current_loop, op, self.current.frames(), self.current.channels);
        self.current_markers = markers::moved_markers(&self.current_markers, op, self.current.frames(), self.current.channels);
        apply(&mut self.current, op);
    }

    pub fn apply(&mut self, op: Op) {
        self.apply_op(&op);
        self.invalidate_spectrogram(&op);
        self.ops.push(op);
        self.redo.clear();
        self.pyramid = OnceLock::new();
        self.revision = next_revision();
    }

    pub fn undo(&mut self) {
        let Some(op) = self.ops.pop() else { return };
        self.invalidate_spectrogram(&op);
        self.redo.push(op);
        // Re-render from the original: ops are cheap compared to keeping snapshots.
        self.current = Pcm {
            samples: self.original.samples.clone(),
            channels: self.original.channels,
            sample_rate: self.original.sample_rate,
        };
        self.current_loop = self.original_loop;
        self.current_markers = self.original_markers.clone();
        for op in std::mem::take(&mut self.ops) {
            self.apply_op(&op);
            self.ops.push(op);
        }
        self.pyramid = OnceLock::new();
        self.revision = next_revision();
    }

    pub fn redo(&mut self) {
        if let Some(op) = self.redo.pop() {
            self.apply_op(&op);
            self.invalidate_spectrogram(&op);
            self.ops.push(op);
            self.pyramid = OnceLock::new();
            self.revision = next_revision();
        }
    }

    /// Wave data of the current audio (see [`wave`]), wide ranges from the pyramid.
    pub fn wave(&self, start: usize, end: usize, columns: usize, split: bool, bands: bool) -> Vec<u8> {
        let pyramid = self.pyramid.get_or_init(|| Pyramid::build(&self.current));
        wave_with(&self.current, Some(pyramid), start, end, columns, split, bands)
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.ops.clone();
    }
}

/// Waveform data for drawing, packed little-endian:
/// `u32 mode, u32 lanes, u32 count, u32 stride`, then values lane by lane as `i16`
/// fractions of full scale (see [`push_level`]): half the size of `f32`, and finer
/// than a pixel even at the deepest vertical zoom.
/// - mode 0 (columns): per column `min, max, rms, low, mid, high` (band RMS, 0 without bands);
/// - mode 1 (samples, when there are fewer frames than columns): one value per frame.
///
/// Lanes are the channels (`split`) or one mixed lane.
/// Without a pyramid, reading every frame; the reference the pyramid is tested against.
#[cfg(test)]
pub fn wave(pcm: &Pcm, start: usize, end: usize, columns: usize, split: bool, bands: bool) -> Vec<u8> {
    wave_with(pcm, None, start, end, columns, split, bands)
}

/// Fields per pyramid block and lane: min, max, sum of squares, low/mid/high band
/// energy, frames.
const PEAK_FIELDS: usize = 7;
/// Frames per block at the finest pyramid level; each level up is 4× coarser.
const PEAK_BASE: usize = 64;

/// Min, max and energies per block of frames at several block sizes, so a wide wave
/// request adds up a few thousand blocks instead of reading every frame.
pub struct Pyramid {
    /// Lanes stored: the mix, then each channel.
    lanes: usize,
    levels: Vec<PeakLevel>,
}

struct PeakLevel {
    block: usize,
    blocks: usize,
    /// `[lane][block][field]`.
    data: Vec<f32>,
}

impl Pyramid {
    pub fn build(pcm: &Pcm) -> Self {
        let ch = pcm.channels.max(1);
        let lanes = ch + 1;
        let frames = pcm.frames();
        let blocks = frames.div_ceil(PEAK_BASE);
        let mut data = vec![0.0f32; lanes * blocks * PEAK_FIELDS];
        // The same two one-pole low-passes as the direct path, run once over the file.
        let coef = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz / pcm.sample_rate.max(1) as f32).exp();
        let (a1, a2) = (coef(250.0), coef(4000.0));
        for lane in 0..lanes {
            let (mut lp1, mut lp2) = (0.0f32, 0.0f32);
            for blk in 0..blocks {
                let acc = &mut data[(lane * blocks + blk) * PEAK_FIELDS..(lane * blocks + blk + 1) * PEAK_FIELDS];
                acc[0] = f32::MAX;
                acc[1] = f32::MIN;
                for f in blk * PEAK_BASE..((blk + 1) * PEAK_BASE).min(frames) {
                    let frame = &pcm.samples[f * ch..(f + 1) * ch];
                    let x = if lane == 0 { frame.iter().sum::<f32>() / ch as f32 } else { frame[lane - 1] };
                    lp1 += a1 * (x - lp1);
                    lp2 += a2 * (x - lp2);
                    acc[0] = acc[0].min(x);
                    acc[1] = acc[1].max(x);
                    acc[2] += x * x;
                    acc[3] += lp1 * lp1;
                    acc[4] += (lp2 - lp1) * (lp2 - lp1);
                    acc[5] += (x - lp2) * (x - lp2);
                    acc[6] += 1.0;
                }
            }
        }
        let mut levels = vec![PeakLevel { block: PEAK_BASE, blocks, data }];
        // Coarser levels until a whole file fits in a few columns' worth of blocks.
        while levels.last().unwrap().blocks > 64 {
            let prev = levels.last().unwrap();
            let blocks = prev.blocks.div_ceil(4);
            let mut data = vec![0.0f32; lanes * blocks * PEAK_FIELDS];
            for lane in 0..lanes {
                for blk in 0..blocks {
                    let out = &mut data[(lane * blocks + blk) * PEAK_FIELDS..(lane * blocks + blk + 1) * PEAK_FIELDS];
                    out[0] = f32::MAX;
                    out[1] = f32::MIN;
                    for child in blk * 4..((blk + 1) * 4).min(prev.blocks) {
                        merge(out, &prev.data[(lane * prev.blocks + child) * PEAK_FIELDS..(lane * prev.blocks + child + 1) * PEAK_FIELDS]);
                    }
                }
            }
            levels.push(PeakLevel { block: prev.block * 4, blocks, data });
        }
        Self { lanes, levels }
    }

    /// The coarsest level that still has at least two blocks per `frames_per_column`,
    /// so a column's edges are off by at most half a column of blocks.
    fn level_for(&self, frames_per_column: usize) -> Option<&PeakLevel> {
        self.levels.iter().rev().find(|l| l.block * 2 <= frames_per_column)
    }
}

fn merge(into: &mut [f32], v: &[f32]) {
    into[0] = into[0].min(v[0]);
    into[1] = into[1].max(v[1]);
    for i in 2..PEAK_FIELDS {
        into[i] += v[i];
    }
}

/// Appends one wave value, clamped to full scale: louder peaks are drawn clipped anyway.
pub(crate) fn push_level(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&((v.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).to_le_bytes());
}

fn wave_with(pcm: &Pcm, pyramid: Option<&Pyramid>, start: usize, end: usize, columns: usize, split: bool, bands: bool) -> Vec<u8> {
    let ch = pcm.channels.max(1);
    let frames = pcm.frames();
    let (start, end) = (start.min(frames), end.min(frames).max(start.min(frames)));
    let lanes = if split { ch } else { 1 };
    let span = end - start;
    let sample_mode = span <= columns;
    let (count, stride) = if sample_mode { (span, 1) } else { (columns, 6) };

    let mut out = Vec::with_capacity(16 + lanes * count * stride * 2);
    for v in [sample_mode as u32, lanes as u32, count as u32, stride as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    let level = if sample_mode { None } else { pyramid.and_then(|p| p.level_for(span / columns).map(|l| (p, l))) };
    if let Some((p, level)) = level {
        debug_assert_eq!(p.lanes, ch + 1);
        for lane in 0..lanes {
            let stored = if split { lane + 1 } else { 0 };
            let base = stored * level.blocks * PEAK_FIELDS;
            for c in 0..columns {
                let a = start + c * span / columns;
                let b = (start + (c + 1) * span / columns).max(a + 1).min(end);
                let (ba, bb) = (a / level.block, b.div_ceil(level.block).min(level.blocks));
                let mut acc = [f32::MAX, f32::MIN, 0.0, 0.0, 0.0, 0.0, 0.0];
                for blk in ba..bb.max(ba + 1).min(level.blocks) {
                    merge(&mut acc, &level.data[base + blk * PEAK_FIELDS..base + (blk + 1) * PEAK_FIELDS]);
                }
                let n = acc[6].max(1.0);
                let band = |e: f32| if bands { (e / n).sqrt() } else { 0.0 };
                for v in [acc[0], acc[1], (acc[2] / n).sqrt(), band(acc[3]), band(acc[4]), band(acc[5])] {
                    push_level(&mut out, v);
                }
            }
        }
        return out;
    }
    let value = |f: usize, lane: usize| -> f32 {
        let frame = &pcm.samples[f * ch..(f + 1) * ch];
        if split { frame[lane] } else { frame.iter().sum::<f32>() / ch as f32 }
    };

    for lane in 0..lanes {
        if sample_mode {
            for f in start..end {
                push_level(&mut out, value(f, lane));
            }
            continue;
        }
        // Three bands from two one-pole low-passes (250 Hz and 4 kHz), warmed up
        // on a little audio before the range so the first columns are settled.
        let coef = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz / pcm.sample_rate as f32).exp();
        let (a1, a2) = (coef(250.0), coef(4000.0));
        let (mut lp1, mut lp2) = (0.0f32, 0.0f32);
        if bands {
            for f in start.saturating_sub(2048)..start {
                let x = value(f, lane);
                lp1 += a1 * (x - lp1);
                lp2 += a2 * (x - lp2);
            }
        }
        for c in 0..columns {
            let a = start + c * span / columns;
            let b = (start + (c + 1) * span / columns).max(a + 1).min(end);
            let (mut lo, mut hi, mut sq) = (f32::MAX, f32::MIN, 0.0f32);
            let (mut e_low, mut e_mid, mut e_high) = (0.0f32, 0.0f32, 0.0f32);
            for f in a..b {
                let x = value(f, lane);
                lo = lo.min(x);
                hi = hi.max(x);
                sq += x * x;
                if bands {
                    lp1 += a1 * (x - lp1);
                    lp2 += a2 * (x - lp2);
                    e_low += lp1 * lp1;
                    e_mid += (lp2 - lp1) * (lp2 - lp1);
                    e_high += (x - lp2) * (x - lp2);
                }
            }
            let n = (b - a) as f32;
            for v in [lo, hi, (sq / n).sqrt(), (e_low / n).sqrt(), (e_mid / n).sqrt(), (e_high / n).sqrt()] {
                push_level(&mut out, v);
            }
        }
    }
    out
}

/// Writes a WAV file: integer PCM at the source bit depth (8/16/24), otherwise
/// 32-bit float. Integer samples are clipped. Loop points go into a `smpl` chunk.
pub fn write_wav(path: &Path, pcm: &Pcm, bit_depth: Option<u32>, loop_points: Option<Loop>) -> Result<()> {
    let err = |e: hound::Error| Error::Other(format!("writing WAV: {e}"));
    let bits = match bit_depth {
        Some(b @ (8 | 16 | 24)) => b as u16,
        _ => 32,
    };
    let spec = hound::WavSpec {
        channels: pcm.channels as u16,
        sample_rate: pcm.sample_rate,
        bits_per_sample: bits,
        sample_format: if bits == 32 { hound::SampleFormat::Float } else { hound::SampleFormat::Int },
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(err)?;
    if bits == 32 {
        for &s in &pcm.samples {
            w.write_sample(s).map_err(err)?;
        }
    } else {
        let max = ((1i64 << (bits - 1)) - 1) as f32;
        for &s in &pcm.samples {
            w.write_sample((s.clamp(-1.0, 1.0) * max).round() as i32).map_err(err)?;
        }
    }
    w.finalize().map_err(err)?;
    if let Some(lp) = loop_points {
        loops::append_smpl(path, lp, pcm.sample_rate)?;
    }
    Ok(())
}

/// A free name next to `path`: "Kick — edited.wav", "Kick — edited 2.wav", …
pub fn edited_path(path: &Path) -> PathBuf {
    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    (1..)
        .map(|n| dir.join(if n == 1 { format!("{stem} — edited.wav") } else { format!("{stem} — edited {n}.wav") }))
        .find(|p| !p.exists())
        .expect("some name is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(frames: usize, channels: usize) -> Pcm {
        // Frame i holds (i + 1) / 100 in every channel.
        let samples = (0..frames).flat_map(|i| std::iter::repeat_n((i + 1) as f32 / 100.0, channels)).collect();
        Pcm { samples, channels, sample_rate: 1000 }
    }

    fn mono(pcm: &Pcm) -> Vec<f32> {
        pcm.samples.chunks(pcm.channels).map(|f| f[0]).collect()
    }

    #[test]
    fn crop_delete_silence_and_reverse_work_on_frames() {
        let mut p = ramp(10, 2);
        apply(&mut p, &Op::Crop { start: 2, end: 6 });
        assert_eq!(mono(&p), [0.03, 0.04, 0.05, 0.06]);
        apply(&mut p, &Op::Delete { start: 1, end: 3 });
        assert_eq!(mono(&p), [0.03, 0.06]);

        let mut p = ramp(5, 2);
        apply(&mut p, &Op::Silence { start: 1, end: 3 });
        assert_eq!(mono(&p), [0.01, 0.0, 0.0, 0.04, 0.05]);
        apply(&mut p, &Op::Reverse { start: 0, end: 5 });
        assert_eq!(mono(&p), [0.05, 0.04, 0.0, 0.0, 0.01]);
        // Channels stay together.
        assert!(p.samples.chunks(2).all(|f| f[0] == f[1]));
    }

    #[test]
    fn fades_gain_and_normalize() {
        let mut p = Pcm { samples: vec![0.5; 5], channels: 1, sample_rate: 1000 };
        apply(&mut p, &Op::FadeIn { start: 0, end: 5, curve: Curve::Linear });
        assert_eq!(mono(&p), [0.0, 0.125, 0.25, 0.375, 0.5]);
        apply(&mut p, &Op::FadeOut { start: 0, end: 5, curve: Curve::Linear });
        assert_eq!(p.samples[0], 0.0);
        assert_eq!(p.samples[4], 0.0);

        let mut p = Pcm { samples: vec![0.25, -0.5, 0.1], channels: 1, sample_rate: 1000 };
        apply(&mut p, &Op::Normalize { start: 0, end: 3, peak_db: 0.0 });
        assert!((p.samples[1] + 1.0).abs() < 1e-6 && (p.samples[0] - 0.5).abs() < 1e-6);
        apply(&mut p, &Op::Gain { start: 0, end: 3, db: -6.0206 });
        assert!((p.samples[1] + 0.5).abs() < 1e-4);
    }

    #[test]
    fn dc_offset_centres_each_channel_and_preserves_the_waveform() {
        let mut pcm = Pcm {
            samples: vec![0.0, -0.75, 0.25, -0.5, 0.5, -0.25, 0.25, -0.5],
            channels: 2, sample_rate: 48000,
        };
        apply(&mut pcm, &Op::DcOffset { start: 0, end: 4 });
        assert_eq!(pcm.samples, [-0.25, -0.25, 0.0, 0.0, 0.25, 0.25, 0.0, 0.0]);
        assert_eq!(pcm.frames(), 4);
        assert_eq!(pcm.sample_rate, 48000);
        let once = pcm.samples.clone();
        apply(&mut pcm, &Op::DcOffset { start: 0, end: 4 });
        assert_eq!(pcm.samples, once);
    }

    #[test]
    fn dc_offset_only_changes_the_clamped_selection_and_handles_silence() {
        let original = vec![0.8, 0.25, 0.5, 0.75, -0.6];
        let mut pcm = Pcm { samples: original.clone(), channels: 1, sample_rate: 48000 };
        apply(&mut pcm, &Op::DcOffset { start: 1, end: 4 });
        assert_eq!(pcm.samples, [0.8, -0.25, 0.0, 0.25, -0.6]);
        let before = pcm.samples.clone();
        for (start, end) in [(2, 2), (20, 30), (4, 1)] {
            apply(&mut pcm, &Op::DcOffset { start, end });
            assert_eq!(pcm.samples, before);
        }
        apply(&mut pcm, &Op::DcOffset { start: 4, end: usize::MAX });
        assert_eq!(pcm.samples[4], 0.0);
        let mut silence = Pcm { samples: vec![0.0; 20], channels: 2, sample_rate: 48000 };
        apply(&mut silence, &Op::DcOffset { start: 0, end: 10 });
        assert!(silence.samples.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn dc_offset_undo_redo_save_and_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dc.wav");
        let pcm = Pcm { samples: vec![0.0, -0.75, 0.25, -0.5, 0.5, -0.25, 0.25, -0.5], channels: 2, sample_rate: 48000 };
        write_wav(&path, &pcm, Some(32), Some(Loop { start: 1, end: 3 })).unwrap();
        append_markers(&path, &[Marker { frame: 2, skip: false }]).unwrap();
        let mut session = Session::open(1, &path, Some(32)).unwrap();
        let before = session.audio().samples.clone();
        session.apply(Op::DcOffset { start: 0, end: 4 });
        let corrected = session.audio().samples.clone();
        assert!(session.state().dirty);
        assert_eq!(session.loop_points(), Some(Loop { start: 1, end: 3 }));
        assert_eq!(session.markers(), &[Marker { frame: 2, skip: false }]);
        session.undo();
        assert_eq!(session.audio().samples, before);
        assert!(!session.state().dirty);
        session.redo();
        assert_eq!(session.audio().samples, corrected);
        let saved = dir.path().join("corrected.wav");
        write_wav(&saved, session.audio(), Some(32), session.loop_points()).unwrap();
        append_markers(&saved, session.markers()).unwrap();
        let reopened = Session::open(2, &saved, Some(32)).unwrap();
        assert_eq!(reopened.audio().samples, corrected);
        assert_eq!(reopened.loop_points(), session.loop_points());
        assert_eq!(reopened.markers(), session.markers());
        // The editor never overwrites the original before an explicit save.
        assert_eq!(Session::open(3, &path, Some(32)).unwrap().audio().samples, before);
    }

    #[test]
    fn out_of_range_edits_are_clamped() {
        let mut p = ramp(4, 1);
        apply(&mut p, &Op::Delete { start: 3, end: 100 });
        assert_eq!(p.frames(), 3);
        apply(&mut p, &Op::Silence { start: 10, end: 20 });
        assert_eq!(p.frames(), 3);
    }

    #[test]
    fn undo_redo_and_dirty_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, &ramp(100, 2), Some(24), None).unwrap();
        let mut s = Session::open(1, &path, Some(24)).unwrap();
        assert!(!s.state().dirty && !s.state().can_undo);

        s.apply(Op::Crop { start: 10, end: 60 });
        s.apply(Op::Gain { start: 0, end: 50, db: -6.0 });
        assert_eq!(s.state().frames, 50);
        assert!(s.state().dirty);

        s.undo();
        s.undo();
        assert_eq!(s.state().frames, 100);
        assert!(!s.state().dirty && s.state().can_redo);
        s.redo();
        assert_eq!(s.state().frames, 50);

        s.mark_saved();
        assert!(!s.state().dirty);
        // A new edit after undo discards the redo history.
        s.undo();
        s.apply(Op::Reverse { start: 0, end: 10 });
        assert!(!s.state().can_redo && s.state().dirty);
    }

    #[test]
    fn wav_round_trip_keeps_format_and_names_do_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Kick.wav");
        let pcm = ramp(64, 2);
        write_wav(&src, &pcm, Some(16), None).unwrap();
        let back = audio::decode(&src, 10.0).unwrap();
        assert_eq!((back.channels, back.sample_rate, back.frames()), (2, 1000, 64));
        assert!((back.samples[10] - pcm.samples[10]).abs() < 1e-3);
        let spec = hound::WavReader::open(&src).unwrap().spec();
        assert_eq!(spec.bits_per_sample, 16);

        let first = edited_path(&src);
        assert_eq!(first.file_name().unwrap(), "Kick — edited.wav");
        std::fs::write(&first, b"").unwrap();
        assert_eq!(edited_path(&src).file_name().unwrap(), "Kick — edited 2.wav");
    }

    fn unpack(bytes: &[u8]) -> (u32, u32, u32, u32, Vec<f32>) {
        let u = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
        let values = bytes[16..].chunks_exact(2).map(|b| i16::from_le_bytes(b.try_into().unwrap()) as f32 / i16::MAX as f32).collect();
        (u(0), u(1), u(2), u(3), values)
    }

    #[test]
    fn wave_packs_columns_lanes_and_bands() {
        // Stereo: left a 100 Hz sine, right a 10 kHz sine, 1 s at 44.1 kHz.
        let sr = 44100;
        let samples = (0..sr)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                [(t * 100.0 * std::f32::consts::TAU).sin() * 0.5, (t * 10000.0 * std::f32::consts::TAU).sin() * 0.5]
            })
            .collect();
        let p = Pcm { samples, channels: 2, sample_rate: sr as u32 };
        let (mode, lanes, count, stride, v) = unpack(&wave(&p, 0, sr, 10, true, true));
        assert_eq!((mode, lanes, count, stride), (0, 2, 10, 6));
        assert_eq!(v.len(), 2 * 10 * 6);
        // Column 5 of each lane: peak ~0.5, RMS ~0.35, energy in the expected band.
        let left = &v[5 * 6..6 * 6];
        let right = &v[(10 + 5) * 6..(10 + 6) * 6];
        assert!((left[1] - 0.5).abs() < 0.01 && (left[2] - 0.354).abs() < 0.01);
        assert!(left[3] > left[4] && left[3] > left[5], "low band dominates: {left:?}");
        assert!(right[5] > right[3] && right[5] > right[4], "high band dominates: {right:?}");
        // Mixed into one lane.
        let (_, lanes, ..) = unpack(&wave(&p, 0, sr, 10, false, false));
        assert_eq!(lanes, 1);
    }

    #[test]
    fn pyramid_matches_reading_every_frame() {
        // 60 s stereo: a slow swell on the left, a decaying tone on the right.
        let sr = 8000;
        let frames = sr * 60;
        let samples = (0..frames)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                let tone = (t * 440.0 * std::f32::consts::TAU).sin();
                [tone * (0.2 + 0.7 * (t / 9.0).sin().abs()), tone * (-t / 20.0).exp()]
            })
            .collect();
        let p = Pcm { samples, channels: 2, sample_rate: sr as u32 };
        let pyramid = Pyramid::build(&p);
        for (start, end, split) in [(0, frames, true), (frames / 3, frames / 3 + sr * 7, false), (17, frames - 5, true)] {
            let (_, lanes, count, _, direct) = unpack(&wave(&p, start, end, 1000, split, false));
            let (_, lanes_p, count_p, _, fast) = unpack(&wave_with(&p, Some(&pyramid), start, end, 1000, split, false));
            assert_eq!((lanes, count), (lanes_p, count_p));
            // Min/max/RMS agree within what shifting a column edge by a block can change.
            let worst = direct.iter().zip(&fast).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            assert!(worst < 0.05, "pyramid differs by {worst} for {start}..{end}");
        }
    }

    #[test]
    fn wave_switches_to_samples_when_zoomed_in() {
        let p = Pcm { samples: vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3], channels: 2, sample_rate: 1000 };
        let (mode, lanes, count, stride, v) = unpack(&wave(&p, 0, 3, 100, true, false));
        assert_eq!((mode, lanes, count, stride), (1, 2, 3, 1));
        let want = [0.1, 0.2, 0.3, -0.1, -0.2, -0.3];
        assert!(v.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-4), "{v:?}");
    }
}
