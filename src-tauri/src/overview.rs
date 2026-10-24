//! Whole-file waveform overview for the player: per block of frames and per lane
//! (the mix, then each channel) the min, max and energy, plus the energy of three
//! frequency bands. Computed by streaming the file, so long songs cost a few
//! megabytes, then reduced to any number of columns in the editor's wave format.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::audio;
use crate::error::Result;

/// Values per block and lane: min, max, sum of squares, low/mid/high band energy, frames.
const FIELDS: usize = 7;
/// Blocks aimed for per file: enough columns for a wide window at 2× density.
const TARGET_BLOCKS: usize = 8192;
/// Overviews kept in memory (recently previewed samples).
const CACHE: usize = 8;
/// Disk space for cached overviews; the least recently used go first beyond it.
const DISK_BUDGET: u64 = 256 << 20;
/// Leading bytes of a cached overview file, then its format version.
const DISK_MAGIC: &[u8; 4] = b"SPWV";
const DISK_VERSION: u32 = 1;

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

    /// The overview as bytes for the disk cache: magic, version, channels, blocks, data.
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + self.data.len() * 4);
        out.extend_from_slice(DISK_MAGIC);
        for v in [DISK_VERSION, self.channels as u32, self.blocks as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in &self.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    fn decode(bytes: &[u8]) -> Option<Self> {
        let u = |i: usize| bytes.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        if bytes.get(..4)? != DISK_MAGIC || u(4)? != DISK_VERSION {
            return None;
        }
        let (channels, blocks) = (u(8)? as usize, u(12)? as usize);
        let values = (channels + 1).checked_mul(blocks)?.checked_mul(FIELDS)?;
        if channels == 0 || bytes.len() != 16 + values * 4 {
            return None;
        }
        let data = bytes[16..].chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
        Some(Self { channels, blocks, data })
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

/// Key of a cached overview: sample id, file size and mtime, so an edited file is
/// read again.
pub type Key = (i64, i64, i64);

/// Overviews on disk, one file per sample, so the player does not decode a long
/// file again after a restart.
#[derive(Clone)]
pub struct DiskCache {
    dir: PathBuf,
    budget: u64,
}

impl DiskCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, budget: DISK_BUDGET }
    }

    fn file(&self, key: Key) -> PathBuf {
        self.dir.join(format!("{}-{}-{}.wave", key.0, key.1, key.2))
    }

    /// The cached overview, if any; reading it marks it as recently used. Disk
    /// trouble just means a miss.
    pub fn load(&self, key: Key) -> Option<Overview> {
        let path = self.file(key);
        let overview = Overview::decode(&std::fs::read(&path).ok()?)?;
        if let Ok(f) = std::fs::File::options().write(true).open(&path) {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(overview)
    }

    /// Stores an overview, replacing older ones of the same sample, then keeps the
    /// directory within its budget. Errors are ignored: it is only a cache.
    pub fn store(&self, key: Key, overview: &Overview) {
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let path = self.file(key);
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, overview.encode()).is_err() || std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        let prefix = format!("{}-", key.0);
        let mut files: Vec<(PathBuf, u64, SystemTime)> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let name = p.file_name()?.to_str()?.to_owned();
                if name.starts_with(&prefix) && p != path {
                    let _ = std::fs::remove_file(&p);
                    return None;
                }
                let meta = e.metadata().ok()?;
                Some((p, meta.len(), meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        if total <= self.budget {
            return;
        }
        // Oldest first, down to 80% so pruning does not run on every store.
        files.sort_by_key(|f| f.2);
        for (p, len, _) in files {
            if total <= self.budget / 5 * 4 {
                break;
            }
            if p != path && std::fs::remove_file(&p).is_ok() {
                total -= len;
            }
        }
    }
}

/// Recently computed overviews in memory, backed by an optional disk cache.
#[derive(Default)]
pub struct Cache {
    entries: Mutex<VecDeque<(Key, Arc<Overview>)>>,
    disk: Option<DiskCache>,
}

impl Cache {
    pub fn with_disk(dir: PathBuf) -> Self {
        Self { entries: Default::default(), disk: Some(DiskCache::new(dir)) }
    }

    /// The disk side, to use off the async runtime.
    pub fn disk(&self) -> Option<DiskCache> {
        self.disk.clone()
    }

    pub fn get(&self, key: Key) -> Option<Arc<Overview>> {
        self.entries.lock().unwrap().iter().find(|(k, _)| *k == key).map(|(_, o)| Arc::clone(o))
    }

    pub fn put(&self, key: Key, overview: Arc<Overview>) {
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

    fn small_overview(seed: f32) -> Overview {
        Overview { channels: 2, blocks: 4, data: (0..3 * 4 * FIELDS).map(|i| i as f32 * seed).collect() }
    }

    #[test]
    fn disk_cache_round_trips_and_replaces_stale_versions() {
        let dir = tempfile::tempdir().unwrap();
        let disk = DiskCache::new(dir.path().join("waveforms"));
        assert!(disk.load((1, 10, 100)).is_none());
        disk.store((1, 10, 100), &small_overview(0.5));
        let back = disk.load((1, 10, 100)).unwrap();
        assert_eq!((back.channels, back.blocks, back.data.len()), (2, 4, 3 * 4 * FIELDS));
        assert_eq!(back.data[7], 3.5);

        // The file changed: the new version replaces the old one on disk.
        disk.store((1, 11, 101), &small_overview(1.0));
        assert!(disk.load((1, 10, 100)).is_none());
        assert!(disk.load((1, 11, 101)).is_some());
        // Garbage or a newer format is a miss, not an error.
        std::fs::write(disk.file((2, 0, 0)), b"SPWV\x09\0\0\0junk").unwrap();
        assert!(disk.load((2, 0, 0)).is_none());
    }

    #[test]
    fn disk_cache_drops_least_recently_used_beyond_budget() {
        let dir = tempfile::tempdir().unwrap();
        let one = small_overview(1.0).encode().len() as u64;
        let disk = DiskCache { dir: dir.path().to_path_buf(), budget: one * 3 };
        for id in 1..=3 {
            disk.store((id, 0, 0), &small_overview(1.0));
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // Reading 1 makes it recent; storing a fourth goes over budget and drops 2.
        assert!(disk.load((1, 0, 0)).is_some());
        std::thread::sleep(std::time::Duration::from_millis(20));
        disk.store((4, 0, 0), &small_overview(1.0));
        assert!(disk.load((1, 0, 0)).is_some());
        assert!(disk.load((2, 0, 0)).is_none());
        assert!(disk.load((4, 0, 0)).is_some());
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
