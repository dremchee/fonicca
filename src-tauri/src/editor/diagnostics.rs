//! Accounting only: never read PCM pages or materialize a shared snapshot.
use super::{Op, Session, storage::Storage};
use serde::Serialize;
use std::{collections::HashSet, sync::{Arc, Weak, OnceLock, atomic::Ordering}};

#[derive(Default)]
pub(super) struct StorageUsage {
    seen_samples: HashSet<usize>,
    seen: HashSet<usize>,
    ram: usize,
    disk: usize,
    seen_peaks: HashSet<usize>,
    peaks: Vec<Weak<OnceLock<super::Pyramid>>>,
}

/// Counts retain no audio/file handles. Weak peak references observe lazy cache
/// construction (including stale worker completion) without retaining its data.
pub(super) struct Inventory {
    ram: usize,
    disk: usize,
    storage_count: usize,
    peaks: Vec<Weak<OnceLock<super::Pyramid>>>,
}
impl Inventory {
    fn cache_bytes(&self) -> usize {
        self.peaks.iter().filter_map(Weak::upgrade)
            .map(|p| p.get().map_or(0, |p| p.cache_bytes())).sum()
    }
}
impl StorageUsage {
    pub(super) fn include_samples(&mut self, identity: usize) -> bool {
        self.seen_samples.insert(identity)
    }
    pub(super) fn include_peaks(
        &mut self,
        peaks: &std::sync::Arc<std::sync::OnceLock<super::Pyramid>>,
    ) {
        if self.seen_peaks.insert(Arc::as_ptr(peaks) as usize) {
            self.peaks.push(Arc::downgrade(peaks));
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
    pub(super) fn invalidate_diagnostics(&mut self) {
        *self.diagnostics_inventory.get_mut().unwrap() = None;
    }

    fn measure_inventory(&self) -> Inventory {
        let mut usage = StorageUsage::default();
        self.current.samples.measure(&mut usage);
        for snapshot in self.history.iter().chain(&self.future) {
            snapshot.audio.samples.measure(&mut usage);
        }
        // Rendered paste/stretch buffers are also retained by operation history.
        for op in self.ops.iter().chain(&self.redo).chain(&self.saved) {
            usage.op(op);
        }
        Inventory { ram: usage.ram, disk: usage.disk, storage_count: usage.seen.len(), peaks: usage.peaks }
    }

    pub fn diagnostics(&self) -> EditorDiagnostics {
        let mut memo = self.diagnostics_inventory.lock().unwrap();
        let usage = memo.get_or_insert_with(|| self.measure_inventory());
        let cache_bytes = usage.cache_bytes() + self.spectrum.as_ref().map_or(0, |s| s.cache_bytes());
        let wave_us = self.last_wave_us.load(Ordering::Relaxed);
        EditorDiagnostics {
            sample_id: self.sample_id,
            logical_bytes: self.current.samples.len() * size_of::<f32>(),
            ram_bytes: usage.ram,
            disk_bytes: usage.disk,
            storage_count: usage.storage_count,
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

    fn fixture(root: &std::path::Path) -> Session {
        let path = root.join("diagnostics.wav");
        super::super::write_wav(&path, &Pcm { samples: vec![0.2; 20000], channels: 1, sample_rate: 1000 }, Some(32), None).unwrap();
        Session::open(1, &path, Some(32)).unwrap()
    }
    fn assert_fresh(session: &Session) {
        let cached = serde_json::to_value(session.diagnostics()).unwrap();
        session.diagnostics_inventory.lock().unwrap().take();
        let fresh = serde_json::to_value(session.diagnostics()).unwrap();
        assert_eq!(cached, fresh, "memoized accounting must match a complete inventory");
    }

    #[test]
    fn memo_tracks_edits_undo_redo_and_redo_discard() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = s.diagnostics().ram_bytes;
        for _ in 0..4 {
            s.try_apply(Op::Gain { start: 100, end: 200, db: -1.0 }).unwrap();
            assert_fresh(&s);
        }
        let retained = s.diagnostics().ram_bytes;
        assert!(retained > original);
        s.undo(); assert_fresh(&s);
        s.redo(); assert_fresh(&s);
        s.undo(); s.undo();
        s.try_apply(Op::SetLoop { start: 0, end: 10 }).unwrap();
        assert_fresh(&s);
        assert!(s.diagnostics().ram_bytes < retained);
        assert_eq!(s.diagnostics().redo_steps, 0);
    }

    #[test]
    fn late_wave_and_spectrum_caches_are_visible_without_an_audio_revision_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let worker = s.audio().clone();
        s.try_apply(Op::Gain { start: 100, end: 200, db: -1.0 }).unwrap();
        assert_eq!(s.diagnostics().cache_bytes, 0);
        // This old worker's audio is now retained only by history. Its cache
        // completion does not call Session::record_wave_time for the new revision.
        worker.wave(0, 20000, 50, true, false);
        let wave_bytes = s.diagnostics().cache_bytes;
        assert!(wave_bytes > 0);
        assert_fresh(&s);
        let spectrum = Arc::new(super::super::spectrogram::Spectrogram::build(s.audio()).unwrap());
        let bytes = spectrum.cache_bytes();
        s.cache_spectrogram(s.audio_revision(), spectrum).unwrap();
        assert_eq!(s.diagnostics().cache_bytes, wave_bytes + bytes);
        assert_fresh(&s);
    }

    #[test]
    fn diagnostics_inventory_does_not_keep_audio_or_peaks_alive() {
        let data = Arc::new(Storage::Ram(vec![0.2; 20000]));
        let weak_data = Arc::downgrade(&data);
        let audio = BlockAudio::from_storage(data.clone(), 1, 1000);
        audio.wave(0, 20000, 50, true, false);
        let mut usage = StorageUsage::default();
        audio.samples.measure(&mut usage);
        let inventory = Inventory { ram: usage.ram, disk: usage.disk, storage_count: usage.seen.len(), peaks: usage.peaks };
        assert!(inventory.cache_bytes() > 0);
        drop(audio); drop(data);
        assert!(weak_data.upgrade().is_none());
        assert!(inventory.peaks.iter().all(|p| p.upgrade().is_none()));
        assert_eq!(inventory.cache_bytes(), 0);
    }

    #[test]
    fn saved_anchor_replacement_invalidates_accounting_even_without_revision_change() {
        use super::super::saving::{Snapshot, SaveMode, SavedFile};
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let rendered = Arc::new(vec![0.4; 100]);
        s.try_apply(Op::Replace { start: 100, end: 200, samples: rendered }).unwrap();
        s.mark_saved();
        s.undo();
        s.try_apply(Op::SetLoop { start: 0, end: 10 }).unwrap();
        let retained = s.diagnostics().ram_bytes;
        let revision = s.revision();
        let snapshot = Snapshot::new(&s, SaveMode::Save, None, None).unwrap();
        snapshot.commit(Some(&mut s), &SavedFile { path: dir.path().join("saved.wav"), backup: None }, None).unwrap();
        assert_eq!(s.revision(), revision);
        assert_eq!(s.diagnostics().ram_bytes, retained - 100 * size_of::<f32>());
        assert_fresh(&s);
    }

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
