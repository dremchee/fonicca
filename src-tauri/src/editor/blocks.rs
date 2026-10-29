//! Immutable audio pieces. Snapshots share sample storage; edits replace only
//! affected spans, with arithmetic effects rendered in bounded frame blocks.
use super::{
    Op,
    storage::{Builder, RAM_CEILING, Storage},
};
use crate::audio::{AudioRead, Pcm};
use crate::error::{Error, Result};
use std::{
    borrow::Cow,
    ops::Range,
    sync::{Arc, OnceLock},
};
mod waveform;

const BLOCK_FRAMES: usize = 16384;

#[derive(Clone)]
struct Piece {
    data: Arc<Storage>,
    range: Range<usize>,
    peaks: Arc<OnceLock<super::Pyramid>>,
}
#[derive(Clone, Default)]
pub struct Samples {
    pieces: Arc<Vec<Piece>>,
    ends: Arc<Vec<usize>>,
    len: usize,
}
impl Samples {
    fn from_pieces(pieces: Vec<Piece>) -> Self {
        let mut merged: Vec<Piece> = Vec::new();
        for piece in pieces.into_iter().filter(|p| !p.range.is_empty()) {
            if let Some(last) = merged.last_mut() {
                if Arc::ptr_eq(&last.data, &piece.data) && last.range.end == piece.range.start {
                    last.range.end = piece.range.end;
                    continue;
                }
            }
            merged.push(piece);
        }
        let mut len = 0;
        let ends = merged
            .iter()
            .map(|p| {
                len += p.range.len();
                len
            })
            .collect();
        Self {
            pieces: Arc::new(merged),
            ends: Arc::new(ends),
            len,
        }
    }
    pub(super) fn measure(&self, usage: &mut super::diagnostics::StorageUsage) {
        for piece in self.pieces.iter() {
            usage.include(&piece.data);
            usage.include_peaks(&piece.peaks);
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn iter(&self) -> impl Iterator<Item = &f32> {
        self.pieces
            .iter()
            .flat_map(|p| p.data[p.range.clone()].iter())
    }
    pub fn last(&self) -> Option<&f32> {
        self.len.checked_sub(1).map(|i| &self[i])
    }
    fn parts(&self, range: Range<usize>) -> Vec<Piece> {
        assert!(range.start <= range.end && range.end <= self.len);
        let mut offset = 0;
        let mut parts = Vec::new();
        for piece in self.pieces.iter() {
            let end = offset + piece.range.len();
            let a = range.start.max(offset);
            let b = range.end.min(end);
            if a < b {
                parts.push(Piece {
                    data: piece.data.clone(),
                    peaks: piece.peaks.clone(),
                    range: piece.range.start + a - offset..piece.range.start + b - offset,
                });
            }
            if end >= range.end {
                break;
            }
            offset = end;
        }
        parts
    }
    pub fn range(&self, range: Range<usize>) -> Cow<'_, [f32]> {
        assert!(range.start <= range.end && range.end <= self.len);
        if range.is_empty() {
            return Cow::Borrowed(&[]);
        }
        let i = self.ends.partition_point(|end| *end <= range.start);
        let offset = if i == 0 { 0 } else { self.ends[i - 1] };
        let p = &self.pieces[i];
        if range.end <= self.ends[i] {
            return Cow::Borrowed(
                &p.data[p.range.start + range.start - offset..p.range.start + range.end - offset],
            );
        }
        Cow::Owned(
            self.parts(range)
                .iter()
                .flat_map(|p| p.data[p.range.clone()].iter().copied())
                .collect(),
        )
    }
    fn replace(&self, range: Range<usize>, insert: Vec<Piece>) -> Self {
        let mut parts = self.parts(0..range.start);
        parts.extend(insert);
        parts.extend(self.parts(range.end..self.len));
        Self::from_pieces(parts)
    }
    #[cfg(test)]
    pub fn same_snapshot(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pieces, &other.pieces)
    }
    #[cfg(test)]
    pub fn shares_storage(&self, other: &Self) -> bool {
        self.pieces
            .iter()
            .any(|a| other.pieces.iter().any(|b| Arc::ptr_eq(&a.data, &b.data)))
    }
}
impl std::ops::Index<usize> for Samples {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        assert!(i < self.len);
        let p = self.ends.partition_point(|end| *end <= i);
        let offset = if p == 0 { 0 } else { self.ends[p - 1] };
        &self.pieces[p].data[self.pieces[p].range.start + i - offset]
    }
}
impl PartialEq for Samples {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl PartialEq<Vec<f32>> for Samples {
    fn eq(&self, other: &Vec<f32>) -> bool {
        self.iter().eq(other.iter())
    }
}
impl std::fmt::Debug for Samples {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Samples")
            .field("len", &self.len)
            .field("pieces", &self.pieces.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct BlockAudio {
    pub samples: Samples,
    pub channels: usize,
    pub sample_rate: u32,
}
impl From<Pcm> for BlockAudio {
    fn from(pcm: Pcm) -> Self {
        let len = pcm.samples.len();
        let piece = Piece {
            data: Arc::new(Storage::Ram(pcm.samples)),
            peaks: Arc::new(OnceLock::new()),
            range: 0..len,
        };
        Self {
            samples: Samples::from_pieces(vec![piece]),
            channels: pcm.channels,
            sample_rate: pcm.sample_rate,
        }
    }
}
impl AudioRead for BlockAudio {
    fn channels(&self) -> usize {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn sample_count(&self) -> usize {
        self.samples.len()
    }
    fn sample(&self, i: usize) -> f32 {
        self.samples[i]
    }
    fn sample_with_cursor(&self, i: usize, cursor: &mut usize) -> f32 {
        assert!(i < self.samples.len);
        let start = if *cursor == 0 {
            0
        } else {
            self.samples
                .ends
                .get(*cursor - 1)
                .copied()
                .unwrap_or(usize::MAX)
        };
        if i < start || self.samples.ends.get(*cursor).is_none_or(|end| i >= *end) {
            *cursor = self.samples.ends.partition_point(|end| *end <= i);
        }
        let start = if *cursor == 0 {
            0
        } else {
            self.samples.ends[*cursor - 1]
        };
        let p = &self.samples.pieces[*cursor];
        p.data[p.range.start + i - start]
    }

    fn range(&self, r: Range<usize>) -> Cow<'_, [f32]> {
        self.samples.range(r)
    }
    fn iter_samples(&self) -> impl Iterator<Item = &f32> {
        self.samples.iter()
    }
}
impl BlockAudio {
    pub(super) fn from_storage(data: Arc<Storage>, channels: usize, sample_rate: u32) -> Self {
        let len = data.len();
        Self {
            samples: Samples::from_pieces(vec![Piece {
                data,
                peaks: Arc::new(OnceLock::new()),
                range: 0..len,
            }]),
            channels,
            sample_rate,
        }
    }
    pub fn slice_frames(&self, start: usize, end: usize) -> Self {
        Self {
            samples: Samples::from_pieces(
                self.samples
                    .parts(start * self.channels..end * self.channels),
            ),
            channels: self.channels,
            sample_rate: self.sample_rate,
        }
    }
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1)
    }
    #[cfg(test)]
    pub fn apply(&mut self, op: &Op) {
        self.try_apply(op).unwrap();
    }
    pub fn try_apply(&mut self, op: &Op) -> Result<()> {
        let ch = self.channels.max(1);
        let frames = self.frames();
        let (a, b) = op.range();
        let a = a.min(frames);
        let b = b.min(frames).max(a);
        match op {
            Op::SetLoop { .. } | Op::ClearLoop | Op::SetMarkers { .. } => return Ok(()),
            Op::Crop { .. } => {
                self.samples = Samples::from_pieces(self.samples.parts(a * ch..b * ch));
                return Ok(());
            }
            Op::Delete { .. } => {
                self.samples = self.samples.replace(a * ch..b * ch, vec![]);
                return Ok(());
            }
            Op::Replace { samples, .. } | Op::Stretch { samples, .. } => {
                if samples.len() % ch != 0 {
                    return Err(Error::Audio(
                        "replacement audio has incomplete frames".into(),
                    ));
                }
                self.samples = self.samples.replace(
                    a * ch..b * ch,
                    vec![Piece {
                        data: Arc::new(Storage::Shared(samples.clone())),
                        peaks: Arc::new(OnceLock::new()),
                        range: 0..samples.len(),
                    }],
                );
                return Ok(());
            }
            _ => {}
        }
        if a == b {
            return Ok(());
        }
        let mut peak = 0.0_f32;
        let mut means = vec![0.0_f64; ch];
        // Walk contiguous source pieces once; indexing each sample would search
        // the piece table repeatedly after cuts and pastes.
        if matches!(op, Op::Normalize { .. } | Op::DcOffset { .. }) {
            for piece in self.samples.parts(a * ch..b * ch) {
                let values = &piece.data[piece.range];
                if matches!(op, Op::Normalize { .. }) {
                    peak = values.iter().fold(peak, |max, sample| max.max(sample.abs()));
                } else {
                    for frame in values.chunks_exact(ch) {
                        for (mean, sample) in means.iter_mut().zip(frame) { *mean += *sample as f64; }
                    }
                }
            }
        }
        for mean in &mut means { *mean /= (b - a) as f64; }
        let constant_gain = match op {
            Op::Gain { db, .. } => super::db_to_gain(*db),
            Op::Normalize { peak_db, .. } if peak > 1e-9 => super::db_to_gain(*peak_db) / peak,
            _ => 1.0,
        };
        let mut rendered = Builder::new(super::memory::pcm_budget().min(RAM_CEILING));
        for start in (a..b).step_by(BLOCK_FRAMES) {
            let end = (start + BLOCK_FRAMES).min(b);
            let mut values = if matches!(op, Op::Reverse { .. }) {
                let mut data = self
                    .range((a + b - end) * ch..(a + b - start) * ch)
                    .into_owned();
                for i in 0..(end - start) / 2 {
                    for c in 0..ch {
                        data.swap(i * ch + c, ((end - start) - 1 - i) * ch + c);
                    }
                }
                data
            } else {
                self.range(start * ch..end * ch).into_owned()
            };
            // Dispatch once per block. Gains and dB conversions are invariant
            // over the selection; only fades need a gain for each frame.
            match op {
                Op::Gain { .. } | Op::Normalize { .. } => {
                    values.iter_mut().for_each(|sample| *sample *= constant_gain);
                }
                Op::Silence { .. } => values.fill(0.0),
                Op::DcOffset { .. } => {
                    for frame in values.chunks_exact_mut(ch) {
                        for (sample, mean) in frame.iter_mut().zip(&means) { *sample = (*sample as f64 - mean) as f32; }
                    }
                }
                Op::FadeIn { curve, .. } | Op::FadeOut { curve, .. } => {
                    let fade_in = matches!(op, Op::FadeIn { .. });
                    let denominator = (b - a - 1) as f32;
                    for (i, frame) in values.chunks_exact_mut(ch).enumerate() {
                        let t = if b - a > 1 { (start + i - a) as f32 / denominator } else { 1.0 };
                        let gain = curve.gain(if fade_in { t } else { 1.0 - t });
                        frame.iter_mut().for_each(|sample| *sample *= gain);
                    }
                }
                _ => {}, // Reverse was performed above; no arithmetic is needed.
            }
            rendered.append(&values)?;
        }
        let data = Arc::new(rendered.finish()?);
        let len = data.len();
        self.samples = self.samples.replace(
            a * ch..b * ch,
            vec![Piece {
                data,
                peaks: Arc::new(OnceLock::new()),
                range: 0..len,
            }],
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Curve, apply};
    use std::collections::HashSet;
    #[test]
    fn normalization_and_dc_scan_fragmented_multichannel_ranges_without_changing_results() {
        for channels in [1, 2, 3, 6] {
            let source = Pcm { samples: (0..40000 * channels).map(|n| (n as f32 * 0.03).sin() * 0.6 + 0.1).collect(), channels, sample_rate: 48000 };
            let mut blocks = BlockAudio::from(source.clone());
            let mut flat = source;
            for op in [
                Op::Delete { start: 123, end: 456 },
                Op::Replace { start: 500, end: 500, samples: Arc::new(vec![0.25; channels * 111]) },
                Op::Normalize { start: 17, end: 35000, peak_db: -1.0 },
                Op::DcOffset { start: 18, end: 33000 },
            ] {
                apply(&mut flat, &op);
                blocks.try_apply(&op).unwrap();
                assert!(blocks.samples.iter().zip(&flat.samples).all(|(a,b)| (a-b).abs() < 1e-6), "{channels} channels, {op:?}");
            }
        }
    }

    fn owned_bytes(a: &BlockAudio, b: &BlockAudio) -> usize {
        let mut seen = HashSet::new();
        a.samples
            .pieces
            .iter()
            .chain(b.samples.pieces.iter())
            .filter(|p| seen.insert(Arc::as_ptr(&p.data)))
            .map(|p| p.data.len() * 4)
            .sum()
    }
    #[test]
    fn short_edits_share_the_rest_and_snapshots_stay_immutable() {
        let original = BlockAudio::from(Pcm {
            samples: vec![0.5; 4_000_000],
            channels: 2,
            sample_rate: 48000,
        });
        let mut edited = original.clone();
        assert_eq!(owned_bytes(&original, &edited), 16_000_000);
        edited.apply(&Op::Gain {
            start: 100,
            end: 200,
            db: -6.0,
        });
        assert_eq!(owned_bytes(&original, &edited), 16_000_800);
        assert!(edited.samples.shares_storage(&original.samples));
        assert_eq!(original.samples[200], 0.5);
        assert!(edited.samples[200] < 0.3);
        assert_eq!(edited.samples[400], 0.5);
        let before_cut = edited.clone();
        edited.apply(&Op::Crop {
            start: 90,
            end: 210,
        });
        assert_eq!(owned_bytes(&before_cut, &edited), 16_000_800);
        assert_eq!(edited.frames(), 120);
    }
    #[test]
    fn wave_and_spectrogram_read_across_pieces_without_flattening() {
        let source = Pcm {
            samples: (0..12000).map(|i| (i as f32 * 0.06).sin() * 0.4).collect(),
            channels: 2,
            sample_rate: 6000,
        };
        let mut flat = source.clone();
        let mut blocks = BlockAudio::from(source);
        for op in [
            Op::Gain {
                start: 101,
                end: 4011,
                db: -6.0,
            },
            Op::Delete {
                start: 900,
                end: 980,
            },
        ] {
            apply(&mut flat, &op);
            blocks.apply(&op);
        }
        let a = super::super::Pyramid::build(&flat);
        let b = super::super::Pyramid::build(&blocks);
        assert_eq!(
            super::super::wave_with(&flat, Some(&a), 0, flat.frames(), 100, true, true),
            super::super::wave_with(&blocks, Some(&b), 0, blocks.frames(), 100, true, true)
        );
        let a = super::super::spectrogram::Spectrogram::build(&flat).unwrap();
        let b = super::super::spectrogram::Spectrogram::build(&blocks).unwrap();
        assert_eq!(
            a.range(0, flat.frames(), 100),
            b.range(0, blocks.frames(), 100)
        );
    }

    #[test]
    fn block_edits_match_contiguous_reference_across_block_and_channel_boundaries() {
        for channels in [1, 2, 3] {
            let source = Pcm {
                samples: (0..40003 * channels)
                    .map(|i| ((i as f32) * 0.113).sin() * 0.7 + 0.04)
                    .collect(),
                channels,
                sample_rate: 48000,
            };
            let mut flat = source.clone();
            let mut blocks = BlockAudio::from(source);
            let operations = vec![
                Op::Gain {
                    start: 11,
                    end: 38001,
                    db: -3.0,
                },
                Op::FadeIn {
                    start: 4,
                    end: 37002,
                    curve: Curve::Smooth,
                },
                Op::FadeOut {
                    start: 1,
                    end: 32000,
                    curve: Curve::Exponential,
                },
                Op::DcOffset {
                    start: 17,
                    end: 39000,
                },
                Op::Normalize {
                    start: 4,
                    end: 34000,
                    peak_db: 0.0,
                },
                Op::Reverse {
                    start: 3,
                    end: 36004,
                },
                Op::Silence {
                    start: 100,
                    end: 501,
                },
                Op::Delete { start: 12, end: 21 },
                Op::Replace {
                    start: 140,
                    end: 151,
                    samples: Arc::new(vec![0.2; channels * 20]),
                },
                Op::Stretch {
                    start: 40,
                    end: 70,
                    samples: Arc::new(vec![0.4; channels * 80]),
                },
                Op::Crop {
                    start: 21,
                    end: 39900,
                },
            ];
            for op in operations {
                apply(&mut flat, &op);
                blocks.apply(&op);
                assert_eq!(blocks.samples.len(), flat.samples.len(), "{op:?}");
                assert!(
                    blocks
                        .samples
                        .iter()
                        .zip(&flat.samples)
                        .all(|(a, b)| (a - b).abs() < 1e-5),
                    "{op:?}"
                );
            }
        }
    }
}
