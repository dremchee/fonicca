//! Whole-file waveform overview for the player: per block of frames and per lane
//! (the mix, then each channel) the min, max and energy, plus the energy of three
//! frequency bands. Computed by streaming the file, so long songs cost a few
//! megabytes, then reduced to any number of columns in the editor's wave format.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::audio;
use crate::error::Result;

/// Values per block and lane: min, max, sum of squares, low/mid/high band energy, frames.
const FIELDS: usize = 7;
/// Blocks aimed for per file: enough columns for a wide window at 2× density.
const TARGET_BLOCKS: usize = 8192;
/// Overviews kept in memory (recently previewed samples).
const CACHE: usize = 8;

pub struct Overview {
    channels: usize,
    blocks: usize,
    /// `[lane][block][field]`, lane 0 is the mix.
    data: Vec<f32>,
}

struct Lane {
    lp1: f32,
    lp2: f32,
    acc: [f32; FIELDS],
}

impl Lane {
    fn new() -> Self {
        Self { lp1: 0.0, lp2: 0.0, acc: [f32::MAX, f32::MIN, 0.0, 0.0, 0.0, 0.0, 0.0] }
    }

    fn push(&mut self, x: f32, a1: f32, a2: f32) {
        // Two one-pole low-passes split bass (< 250 Hz), mids and highs (> 4 kHz).
        self.lp1 += a1 * (x - self.lp1);
        self.lp2 += a2 * (x - self.lp2);
        let a = &mut self.acc;
        a[0] = a[0].min(x);
        a[1] = a[1].max(x);
        a[2] += x * x;
        a[3] += self.lp1 * self.lp1;
        a[4] += (self.lp2 - self.lp1) * (self.lp2 - self.lp1);
        a[5] += (x - self.lp2) * (x - self.lp2);
        a[6] += 1.0;
    }

    fn take(&mut self) -> [f32; FIELDS] {
        std::mem::replace(&mut self.acc, [f32::MAX, f32::MIN, 0.0, 0.0, 0.0, 0.0, 0.0])
    }
}

impl Overview {
    /// `frames_hint` (from the index) sizes the blocks; without it 256 frames each.
    pub fn compute(path: &Path, frames_hint: Option<usize>) -> Result<Self> {
        let block = frames_hint.map_or(256, |f| (f / TARGET_BLOCKS).max(1));
        let mut lanes: Vec<Lane> = Vec::new();
        let mut per_lane: Vec<Vec<[f32; FIELDS]>> = Vec::new();
        let (mut a1, mut a2) = (0.0, 0.0);
        let mut filled = 0usize;
        let mut channels = 0usize;
        audio::stream(path, |chunk, ch, rate| {
            if lanes.is_empty() {
                channels = ch;
                lanes = (0..=ch).map(|_| Lane::new()).collect();
                per_lane = vec![Vec::new(); ch + 1];
                let coef = |hz: f32| 1.0 - (-std::f32::consts::TAU * hz / rate as f32).exp();
                (a1, a2) = (coef(250.0), coef(4000.0));
            }
            for frame in chunk.chunks(ch) {
                let mix = frame.iter().sum::<f32>() / ch as f32;
                lanes[0].push(mix, a1, a2);
                for (c, &x) in frame.iter().enumerate() {
                    lanes[c + 1].push(x, a1, a2);
                }
                filled += 1;
                if filled == block {
                    for (l, lane) in lanes.iter_mut().enumerate() {
                        per_lane[l].push(lane.take());
                    }
                    filled = 0;
                }
            }
        })?;
        if filled > 0 {
            for (l, lane) in lanes.iter_mut().enumerate() {
                per_lane[l].push(lane.take());
            }
        }
        let blocks = per_lane.first().map_or(0, Vec::len);
        let data = per_lane.into_iter().flatten().flatten().collect();
        Ok(Self { channels: channels.max(1), blocks, data })
    }

    /// Packs `columns` columns in the editor wave format (`editor::wave`, mode 0).
    pub fn render(&self, columns: usize, split: bool) -> Vec<u8> {
        let lanes: Vec<usize> = if split && self.channels > 1 { (1..=self.channels).collect() } else { vec![0] };
        let columns = columns.max(1);
        let mut out = Vec::with_capacity(16 + lanes.len() * columns * 24);
        for v in [0u32, lanes.len() as u32, columns as u32, 6] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for &lane in &lanes {
            let base = lane * self.blocks * FIELDS;
            for c in 0..columns {
                let (mut lo, mut hi) = (0.0f32, 0.0f32);
                let mut sums = [0.0f32; 5];
                if self.blocks > 0 {
                    let a = (c * self.blocks / columns).min(self.blocks - 1);
                    let b = ((c + 1) * self.blocks / columns).clamp(a + 1, self.blocks);
                    (lo, hi) = (f32::MAX, f32::MIN);
                    for blk in a..b {
                        let v = &self.data[base + blk * FIELDS..base + (blk + 1) * FIELDS];
                        lo = lo.min(v[0]);
                        hi = hi.max(v[1]);
                        for (s, x) in sums.iter_mut().zip(&v[2..]) {
                            *s += x;
                        }
                    }
                }
                let n = sums[4].max(1.0);
                for v in [lo, hi, (sums[0] / n).sqrt(), (sums[1] / n).sqrt(), (sums[2] / n).sqrt(), (sums[3] / n).sqrt()] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        out
    }
}

/// Recently computed overviews, keyed by sample id plus size and mtime so an
/// edited file is read again.
#[derive(Default)]
pub struct Cache {
    entries: Mutex<VecDeque<((i64, i64, i64), Arc<Overview>)>>,
}

impl Cache {
    pub fn get(&self, key: (i64, i64, i64)) -> Option<Arc<Overview>> {
        self.entries.lock().unwrap().iter().find(|(k, _)| *k == key).map(|(_, o)| Arc::clone(o))
    }

    pub fn put(&self, key: (i64, i64, i64), overview: Arc<Overview>) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|(k, _)| k.0 != key.0);
        entries.push_front((key, overview));
        entries.truncate(CACHE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unpack(bytes: &[u8]) -> (u32, u32, u32, Vec<f32>) {
        let u = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
        let values = bytes[16..].chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
        (u(1), u(2), u(3), values)
    }

    #[test]
    fn overview_matches_the_signal_per_channel_and_band() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("st.wav");
        let sr = 44100;
        // Left 100 Hz, right 10 kHz, 1 s.
        let samples: Vec<f32> = (0..sr)
            .flat_map(|i| {
                let t = i as f32 / sr as f32;
                [(t * 100.0 * std::f32::consts::TAU).sin() * 0.5, (t * 10000.0 * std::f32::consts::TAU).sin() * 0.5]
            })
            .collect();
        crate::editor::write_wav(&path, &audio::Pcm { samples, channels: 2, sample_rate: sr as u32 }, None, None).unwrap();

        let o = Overview::compute(&path, Some(sr)).unwrap();
        let (lanes, count, stride, v) = unpack(&o.render(20, true));
        assert_eq!((lanes, count, stride), (2, 20, 6));
        let left = &v[10 * 6..11 * 6];
        let right = &v[(20 + 10) * 6..(20 + 11) * 6];
        assert!((left[1] - 0.5).abs() < 0.01 && (left[2] - 0.354).abs() < 0.01, "{left:?}");
        assert!(left[3] > left[5] && right[5] > right[3]);
        // Mixed: one lane.
        let (lanes, ..) = unpack(&o.render(20, false));
        assert_eq!(lanes, 1);
    }

    #[test]
    fn cache_keeps_recent_entries_and_replaces_stale_ones() {
        let cache = Cache::default();
        let o = Arc::new(Overview { channels: 1, blocks: 0, data: Vec::new() });
        cache.put((1, 10, 100), Arc::clone(&o));
        assert!(cache.get((1, 10, 100)).is_some());
        cache.put((1, 11, 101), Arc::clone(&o));
        assert!(cache.get((1, 10, 100)).is_none());
        for id in 2..20 {
            cache.put((id, 0, 0), Arc::clone(&o));
        }
        assert!(cache.get((1, 11, 101)).is_none());
    }
}
