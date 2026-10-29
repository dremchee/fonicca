//! Peaks belong to immutable source storage, not the current arrangement.
//! A cut reuses the original peaks; an effect builds peaks only for its output.
use super::*;
use crate::editor::{PEAK_BASE, PEAK_FIELDS, Pyramid, merge, push_level, wave_with};

struct Source<'a> {
    storage: &'a Storage,
    channels: usize,
    rate: u32,
}
impl AudioRead for Source<'_> {
    fn channels(&self) -> usize {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn sample_count(&self) -> usize {
        self.storage.len()
    }
    fn sample(&self, i: usize) -> f32 {
        self.storage[i]
    }
    fn range(&self, range: Range<usize>) -> Cow<'_, [f32]> {
        Cow::Borrowed(&self.storage[range])
    }
    fn iter_samples(&self) -> impl Iterator<Item = &f32> {
        self.storage.iter()
    }
}

impl Pyramid {
    pub(in crate::editor) fn cache_bytes(&self) -> usize {
        self.levels
            .iter()
            .map(|l| l.data.capacity() * size_of::<f32>())
            .sum()
    }
    /// Aggregate only samples inside the requested source range. Complete aligned
    /// blocks use cached statistics. At most two partial base blocks read PCM,
    /// so cuts never leak peaks from deleted samples across a piece boundary.
    fn accumulate(
        &self,
        source: &Source<'_>,
        mut start: usize,
        end: usize,
        lane: usize,
        bands: bool,
        acc: &mut [f32],
    ) {
        while start < end {
            if start % PEAK_BASE != 0 || end - start < PEAK_BASE {
                let stop = end.min((start / PEAK_BASE + 1) * PEAK_BASE);
                for frame in start..stop {
                    let samples =
                        &source.storage[frame * source.channels..(frame + 1) * source.channels];
                    let value = if lane == 0 {
                        samples.iter().sum::<f32>() / source.channels as f32
                    } else {
                        samples[lane - 1]
                    };
                    acc[0] = acc[0].min(value);
                    acc[1] = acc[1].max(value);
                    acc[2] += value * value;
                    acc[6] += 1.0;
                }
                if bands {
                    // Band energies at a partial edge are proportional estimates
                    // of its <=64-frame base block; peaks and RMS above are exact.
                    let base = &self.levels[0];
                    let i = (lane * base.blocks + start / PEAK_BASE) * PEAK_FIELDS;
                    let weight = (stop - start) as f32 / base.data[i + 6].max(1.0);
                    for field in 3..6 {
                        acc[field] += base.data[i + field] * weight;
                    }
                }
                start = stop;
            } else {
                let level = self
                    .levels
                    .iter()
                    .rev()
                    .find(|l| start % l.block == 0 && l.block <= end - start)
                    .unwrap();
                let i = (lane * level.blocks + start / level.block) * PEAK_FIELDS;
                merge(acc, &level.data[i..i + PEAK_FIELDS]);
                start += level.block;
            }
        }
    }
}

impl BlockAudio {
    pub fn wave(
        &self,
        start: usize,
        end: usize,
        columns: usize,
        split: bool,
        bands: bool,
    ) -> Vec<u8> {
        let columns = columns.max(1);
        let start = start.min(self.frames());
        let end = end.min(self.frames()).max(start);
        let span = end - start;
        // High zoom reads only the visible samples and needs no whole-file cache.
        if span / columns < PEAK_BASE * 2 {
            return wave_with(self, None, start, end, columns, split, bands);
        }
        let mut sources = Vec::new();
        let mut offset = 0;
        for piece in self.samples.pieces.iter() {
            let frames = piece.range.len() / self.channels;
            if offset < end && offset + frames > start {
                let source = Source {
                    storage: &piece.data,
                    channels: self.channels,
                    rate: self.sample_rate,
                };
                let peaks = piece.peaks.get_or_init(|| Pyramid::build(&source));
                sources.push((offset, frames, piece, peaks));
            }
            offset += frames;
        }
        let lanes = if split { self.channels } else { 1 };
        let mut out = Vec::with_capacity(16 + lanes * columns * 6 * 2);
        for v in [0, lanes as u32, columns as u32, 6] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for lane in 0..lanes {
            let stored = if split { lane + 1 } else { 0 };
            let mut first = 0;
            for c in 0..columns {
                let a = start + c * span / columns;
                let b = start + (c + 1) * span / columns;
                while first < sources.len() && sources[first].0 + sources[first].1 <= a {
                    first += 1;
                }
                let mut acc = [f32::MAX, f32::MIN, 0.0, 0.0, 0.0, 0.0, 0.0];
                for &(offset, frames, piece, peaks) in &sources[first..] {
                    if offset >= b {
                        break;
                    }
                    let origin = piece.range.start / self.channels;
                    let source = Source {
                        storage: &piece.data,
                        channels: self.channels,
                        rate: self.sample_rate,
                    };
                    peaks.accumulate(
                        &source,
                        origin + a.max(offset) - offset,
                        origin + b.min(offset + frames) - offset,
                        stored,
                        bands,
                        &mut acc,
                    );
                }
                let n = acc[6].max(1.0);
                let band = |e: f32| if bands { (e / n).sqrt() } else { 0.0 };
                for v in [
                    acc[0],
                    acc[1],
                    (acc[2] / n).sqrt(),
                    band(acc[3]),
                    band(acc[4]),
                    band(acc[5]),
                ] {
                    push_level(&mut out, v);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Curve, apply, wave};

    fn fixture(channels: usize) -> Pcm {
        Pcm {
            samples: (0..50000)
                .flat_map(|frame| {
                    (0..channels).map(move |lane| {
                        ((frame as f32 * (0.013 + lane as f32 * 0.007)).sin() * 0.7)
                            * (0.6 + 0.4 * (frame as f32 * 0.0002).cos())
                    })
                })
                .collect(),
            channels,
            sample_rate: 8000,
        }
    }
    fn assert_wave(
        pcm: &Pcm,
        audio: &BlockAudio,
        start: usize,
        end: usize,
        columns: usize,
        split: bool,
    ) {
        let direct = wave(pcm, start, end, columns, split, false);
        let cached = audio.wave(start, end, columns, split, false);
        assert_eq!(&direct[..16], &cached[..16]);
        for (i, (a, b)) in direct[16..]
            .chunks_exact(2)
            .zip(cached[16..].chunks_exact(2))
            .enumerate()
        {
            let a = i16::from_le_bytes(a.try_into().unwrap()) as i32;
            let b = i16::from_le_bytes(b.try_into().unwrap()) as i32;
            assert!((a - b).abs() <= 1, "wave differs at {i}: {a} vs {b}");
        }
    }
    #[test]
    fn edited_piece_wave_matches_direct_samples_at_arbitrary_boundaries() {
        for channels in [1, 2, 3] {
            let mut pcm = fixture(channels);
            let mut audio = BlockAudio::from(pcm.clone());
            audio.wave(0, 50000, 100, true, false);
            let cached = audio.samples.pieces[0].peaks.clone();
            let address = cached.get().unwrap() as *const Pyramid;
            for op in [
                Op::FadeOut {
                    start: 1043,
                    end: 5701,
                    curve: Curve::Smooth,
                },
                Op::Delete {
                    start: 127,
                    end: 2903,
                },
                Op::Crop {
                    start: 57,
                    end: 37129,
                },
                Op::Gain {
                    start: 1943,
                    end: 2371,
                    db: -6.0,
                },
                Op::Reverse {
                    start: 187,
                    end: 3137,
                },
                Op::Replace {
                    start: 113,
                    end: 503,
                    samples: Arc::new(vec![0.1; channels * 251]),
                },
            ] {
                apply(&mut pcm, &op);
                audio.try_apply(&op).unwrap();
                for split in [false, true] {
                    assert_wave(&pcm, &audio, 0, pcm.frames(), 100, split);
                    assert_wave(&pcm, &audio, 19, pcm.frames() - 27, 17, split);
                    assert_wave(&pcm, &audio, 111, 511, 1000, split);
                }
                assert_eq!(address, cached.get().unwrap() as *const Pyramid);
            }
        }
    }
    #[test]
    fn deleted_spikes_do_not_leak_from_cached_boundary_blocks() {
        let mut pcm = Pcm {
            samples: vec![0.0; 50000],
            channels: 1,
            sample_rate: 8000,
        };
        pcm.samples[21..37].fill(1.0);
        let mut audio = BlockAudio::from(pcm.clone());
        audio.wave(0, pcm.frames(), 100, false, false);
        let op = Op::Delete { start: 19, end: 39 };
        apply(&mut pcm, &op);
        audio.try_apply(&op).unwrap();
        assert_wave(&pcm, &audio, 0, pcm.frames(), 100, false);
    }
    #[test]
    fn zoomed_view_does_not_build_a_full_source_cache() {
        let audio = BlockAudio::from(fixture(2));
        audio.wave(1000, 1100, 200, true, true);
        assert!(audio.samples.pieces[0].peaks.get().is_none());
    }
    #[test]
    fn cached_frequency_colors_keep_low_and_high_channels_distinct() {
        let rate = 44100;
        let pcm = Pcm {
            samples: (0..rate * 2)
                .flat_map(|f| {
                    let t = f as f32 / rate as f32;
                    [
                        (t * 100.0 * std::f32::consts::TAU).sin() * 0.5,
                        (t * 10000.0 * std::f32::consts::TAU).sin() * 0.5,
                    ]
                })
                .collect(),
            channels: 2,
            sample_rate: rate as u32,
        };
        let audio = BlockAudio::from(pcm);
        let bytes = audio.wave(17, rate * 2 - 21, 100, true, true);
        let values: Vec<_> = bytes[16..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let left = &values[50 * 6..51 * 6];
        let right = &values[(100 + 50) * 6..(100 + 51) * 6];
        assert!(left[3] > left[4] && left[3] > left[5]);
        assert!(right[5] > right[3] && right[5] > right[4]);
    }
}
