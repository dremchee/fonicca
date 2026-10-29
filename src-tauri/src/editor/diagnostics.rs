//! Accounting only: never read PCM pages or materialize a shared snapshot.
use super::{Op, Session, storage::Storage};
use serde::Serialize;
use std::{collections::HashSet, sync::atomic::Ordering};

#[derive(Default)]
pub(super) struct StorageUsage {
    seen: HashSet<usize>,
    ram: usize,
    disk: usize,
    seen_peaks: HashSet<usize>,
    cache: usize,
}
impl StorageUsage {
    pub(super) fn include_peaks(
        &mut self,
        peaks: &std::sync::Arc<std::sync::OnceLock<super::Pyramid>>,
    ) {
        if self
            .seen_peaks
            .insert(std::sync::Arc::as_ptr(peaks) as usize)
        {
            if let Some(p) = peaks.get() {
                self.cache += p.cache_bytes();
            }
        }
    }
    fn ram(&mut self, samples: &Vec<f32>) {
        if !samples.is_empty() && self.seen.insert(samples.as_ptr() as usize) {
            self.ram += samples.capacity() * size_of::<f32>();
        }
    }
    pub(super) fn include(&mut self, storage: &Storage) {
        match storage {
            Storage::Ram(v) => self.ram(v),
            Storage::Shared(v) => self.ram(v),
            Storage::Disk(_) => {
                if !storage.is_empty() && self.seen.insert(storage.as_ptr() as usize) {
                    self.disk += storage.len() * size_of::<f32>();
                }
            }
        }
    }
    fn op(&mut self, op: &Op) {
        if let Op::Replace { samples, .. } | Op::Stretch { samples, .. } = op {
            self.ram(samples);
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDiagnostics {
    sample_id: i64,
    logical_bytes: usize,
    ram_bytes: usize,
    disk_bytes: usize,
    storage_count: usize,
    cache_bytes: usize,
    undo_steps: usize,
    redo_steps: usize,
    opened_ms: f64,
    last_wave_ms: Option<f64>,
}
impl Session {
    pub fn diagnostics(&self) -> EditorDiagnostics {
        let mut usage = StorageUsage::default();
        self.current.samples.measure(&mut usage);
        for snapshot in self.history.iter().chain(&self.future) {
            snapshot.audio.samples.measure(&mut usage);
        }
        // Rendered paste/stretch buffers are also retained by operation history.
        for op in self.ops.iter().chain(&self.redo).chain(&self.saved) {
            usage.op(op);
        }
        let cache_bytes = usage.cache + self.spectrum.as_ref().map_or(0, |s| s.cache_bytes());
        let wave_us = self.last_wave_us.load(Ordering::Relaxed);
        EditorDiagnostics {
            sample_id: self.sample_id,
            logical_bytes: self.current.samples.len() * size_of::<f32>(),
            ram_bytes: usage.ram,
            disk_bytes: usage.disk,
            storage_count: usage.seen.len(),
            cache_bytes,
            undo_steps: self.history.len(),
            redo_steps: self.future.len(),
            opened_ms: self.opened_ms,
            last_wave_ms: (wave_us > 0).then_some(wave_us as f64 / 1000.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio::Pcm,
        editor::{blocks::BlockAudio, storage::Builder},
    };
    use std::sync::Arc;

    #[test]
    fn metadata_and_undo_keep_existing_wave_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.wav");
        super::super::write_wav(
            &path,
            &Pcm {
                samples: vec![0.25; 20000],
                channels: 2,
                sample_rate: 1000,
            },
            Some(32),
            None,
        )
        .unwrap();
        let mut session = Session::open(1, &path, Some(32)).unwrap();
        session.wave(0, 10000, 50, true, false);
        let before = session.diagnostics().cache_bytes;
        assert!(before > 0);
        session
            .try_apply(Op::SetLoop {
                start: 10,
                end: 500,
            })
            .unwrap();
        assert_eq!(
            session.diagnostics().cache_bytes,
            before,
            "metadata must not discard audio peaks"
        );
        session
            .try_apply(Op::Gain {
                start: 100,
                end: 200,
                db: -6.0,
            })
            .unwrap();
        session.wave(0, 10000, 50, true, false);
        session.undo();
        assert!(
            session.diagnostics().cache_bytes >= before,
            "undo must retain cached peaks"
        );
    }

    #[test]
    fn shared_snapshots_and_rendered_ops_are_counted_once() {
        let mut audio = BlockAudio::from(Pcm {
            samples: vec![0.2; 1000],
            channels: 1,
            sample_rate: 1000,
        });
        let original = audio.clone();
        let rendered = Arc::new(vec![0.4; 100]);
        let op = Op::Replace {
            start: 10,
            end: 110,
            samples: rendered.clone(),
        };
        audio.try_apply(&op).unwrap();
        let mut usage = StorageUsage::default();
        audio.samples.measure(&mut usage);
        original.samples.measure(&mut usage);
        audio.clone().samples.measure(&mut usage);
        usage.op(&op);
        assert_eq!(usage.ram, 4400);
        assert_eq!(usage.disk, 0);
        assert_eq!(usage.seen.len(), 2);
    }
    #[test]
    fn mapped_storage_is_not_reported_as_heap_memory() {
        let mut builder = Builder::new(0);
        builder.append(&[0.5; 1000]).unwrap();
        let data = builder.finish().unwrap();
        let mut usage = StorageUsage::default();
        usage.include(&data);
        usage.include(&data);
        assert_eq!(usage.ram, 0);
        assert_eq!(usage.disk, 4000);
        assert_eq!(usage.seen.len(), 1);
    }
}
