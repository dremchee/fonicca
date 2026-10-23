//! Single-file audio editor. Edits are kept as a list of operations on top of
//! the decoded original, so undo is unlimited and the file on disk is untouched
//! until saved.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio::{self, Pcm};
use crate::error::{Error, Result};

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
}

impl Op {
    fn range(&self) -> (usize, usize) {
        match *self {
            Op::Crop { start, end }
            | Op::Delete { start, end }
            | Op::Silence { start, end }
            | Op::FadeIn { start, end, .. }
            | Op::FadeOut { start, end, .. }
            | Op::Gain { start, end, .. }
            | Op::Normalize { start, end, .. }
            | Op::Reverse { start, end } => (start, end),
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

/// An open file with its edit history.
pub struct Session {
    pub sample_id: i64,
    pub path: PathBuf,
    pub bit_depth: Option<u32>,
    original: Pcm,
    current: Pcm,
    ops: Vec<Op>,
    redo: Vec<Op>,
    /// Ops at the last save, i.e. what is on disk.
    saved: Vec<Op>,
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
}

impl Session {
    pub fn open(sample_id: i64, path: &Path, bit_depth: Option<u32>) -> Result<Self> {
        let pcm = audio::decode(path, MAX_SECONDS + 1.0)?;
        if pcm.frames() as f64 > MAX_SECONDS * pcm.sample_rate as f64 {
            return Err(Error::Other("files longer than 20 minutes cannot be edited".into()));
        }
        Ok(Self {
            sample_id,
            path: path.to_path_buf(),
            bit_depth,
            current: Pcm { samples: pcm.samples.clone(), channels: pcm.channels, sample_rate: pcm.sample_rate },
            original: pcm,
            ops: Vec::new(),
            redo: Vec::new(),
            saved: Vec::new(),
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
        }
    }

    pub fn audio(&self) -> &Pcm {
        &self.current
    }

    pub fn apply(&mut self, op: Op) {
        apply(&mut self.current, &op);
        self.ops.push(op);
        self.redo.clear();
    }

    pub fn undo(&mut self) {
        let Some(op) = self.ops.pop() else { return };
        self.redo.push(op);
        // Re-render from the original: ops are cheap compared to keeping snapshots.
        self.current = Pcm {
            samples: self.original.samples.clone(),
            channels: self.original.channels,
            sample_rate: self.original.sample_rate,
        };
        for op in &self.ops {
            apply(&mut self.current, op);
        }
    }

    pub fn redo(&mut self) {
        if let Some(op) = self.redo.pop() {
            apply(&mut self.current, &op);
            self.ops.push(op);
        }
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.ops.clone();
    }
}

/// Min/max of the mono mix in `columns` equal parts of `[start, end)`, scaled to i8.
pub fn peaks(pcm: &Pcm, start: usize, end: usize, columns: usize) -> Vec<i8> {
    let ch = pcm.channels.max(1);
    let frames = pcm.frames();
    let (start, end) = (start.min(frames), end.min(frames));
    let mut out = Vec::with_capacity(columns * 2);
    if end <= start || columns == 0 {
        out.resize(columns * 2, 0);
        return out;
    }
    let span = end - start;
    for c in 0..columns {
        let a = start + c * span / columns;
        let b = (start + (c + 1) * span / columns).max(a + 1).min(end);
        let (mut lo, mut hi) = (0.0f32, 0.0f32);
        for f in a..b {
            let v = pcm.samples[f * ch..(f + 1) * ch].iter().sum::<f32>() / ch as f32;
            lo = lo.min(v);
            hi = hi.max(v);
        }
        out.push((lo.clamp(-1.0, 1.0) * 127.0) as i8);
        out.push((hi.clamp(-1.0, 1.0) * 127.0) as i8);
    }
    out
}

/// Writes a WAV file: integer PCM at the source bit depth (8/16/24), otherwise
/// 32-bit float. Integer samples are clipped.
pub fn write_wav(path: &Path, pcm: &Pcm, bit_depth: Option<u32>) -> Result<()> {
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
    Ok(())
}

/// A free name next to `path`: "Kick (edit).wav", "Kick (edit 2).wav", …
pub fn edited_path(path: &Path) -> PathBuf {
    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    (1..)
        .map(|n| dir.join(if n == 1 { format!("{stem} (edit).wav") } else { format!("{stem} (edit {n}).wav") }))
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
        write_wav(&path, &ramp(100, 2), Some(24)).unwrap();
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
        write_wav(&src, &pcm, Some(16)).unwrap();
        let back = audio::decode(&src, 10.0).unwrap();
        assert_eq!((back.channels, back.sample_rate, back.frames()), (2, 1000, 64));
        assert!((back.samples[10] - pcm.samples[10]).abs() < 1e-3);
        let spec = hound::WavReader::open(&src).unwrap().spec();
        assert_eq!(spec.bits_per_sample, 16);

        let first = edited_path(&src);
        assert_eq!(first.file_name().unwrap(), "Kick (edit).wav");
        std::fs::write(&first, b"").unwrap();
        assert_eq!(edited_path(&src).file_name().unwrap(), "Kick (edit 2).wav");
    }

    #[test]
    fn peaks_cover_the_visible_range() {
        let p = Pcm { samples: vec![0.0, 1.0, -1.0, 0.5], channels: 1, sample_rate: 1000 };
        assert_eq!(peaks(&p, 0, 4, 2), vec![0, 127, -127, 63]);
        assert_eq!(peaks(&p, 2, 4, 1), vec![-127, 63]);
    }
}
