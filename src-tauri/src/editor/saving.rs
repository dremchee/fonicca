//! File saving policy shared by the editor commands and filesystem tests.
//! File writes are serialized independently from live editor access.
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::{
    blocks::BlockAudio, Loop, Marker, Op, Session, Span, State,
};
#[cfg(test)]
use crate::{audio::Pcm, editor::write_wav};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveMode {
    Save,
    SaveAs,
    ReplaceOriginal,
    Selection,
}

pub struct SavedFile {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
}
static SERIAL: AtomicU64 = AtomicU64::new(0);
static FILE_WRITES: Mutex<()> = Mutex::new(());
fn unique_suffix() -> String {
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
}
fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

struct Lease(Arc<AtomicBool>);
impl Lease {
    fn acquire(session: &Session) -> Result<Self> {
        let flag = session.save_in_flight.clone();
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| Error::Other("this editor is already saving audio".into()))?;
        Ok(Self(flag))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Cheap immutable state captured while holding the session map mutex. The
/// lease rejects overlapping saves of the same session and also identifies it
/// after close/reopen. New audio edits may continue while this version is written.
pub struct Snapshot {
    lease: Lease,
    source_id: i64,
    audio: BlockAudio,
    loop_points: Option<Loop>,
    markers: Vec<Marker>,
    ops: Vec<Op>,
    bit_depth: Option<u32>,
    original_path: PathBuf,
    path: PathBuf,
    has_saved_target: bool,
    sample_rate: Option<u32>,
    mode: SaveMode,
    destination: Option<PathBuf>,
}
impl Snapshot {
    pub fn new(
        session: &Session,
        mode: SaveMode,
        destination: Option<&Path>,
        range: Option<Span>,
    ) -> Result<Self> {
        let lease = Lease::acquire(session)?;
        let (audio, loop_points, markers) = if mode == SaveMode::Selection {
            let Span { start, end } =
                range.ok_or_else(|| Error::Other("select a non-empty fragment".into()))?;
            let audio = session.audio();
            if start >= end || end > audio.frames() {
                return Err(Error::Other(
                    "select a non-empty fragment inside the file".into(),
                ));
            }
            let selected = audio.slice_frames(start, end);
            let lp = session
                .loop_points()
                .map(|lp| Loop {
                    start: lp.start.clamp(start, end) - start,
                    end: lp.end.clamp(start, end) - start,
                })
                .filter(|lp| lp.start < lp.end);
            let markers = session
                .markers()
                .iter()
                .filter(|m| m.frame >= start && m.frame < end)
                .map(|m| Marker {
                    frame: m.frame - start,
                    skip: m.skip,
                })
                .collect();
            (selected, lp, markers)
        } else {
            (
                session.audio().clone(),
                session.loop_points(),
                session.markers().to_vec(),
            )
        };
        Ok(Self {
            lease,
            source_id: session.sample_id,
            audio,
            loop_points,
            markers,
            ops: session.ops.clone(),
            bit_depth: session.bit_depth,
            original_path: session.original_path.clone(),
            path: session.path.clone(),
            has_saved_target: session.has_saved_target,
            sample_rate: if mode == SaveMode::Save { session.export_sample_rate } else { None },
            mode,
            destination: destination.map(Path::to_path_buf),
        })
    }

    pub fn source_id(&self) -> i64 {
        self.source_id
    }

    pub fn with_sample_rate(mut self, rate: Option<u32>) -> Self {
        self.sample_rate = rate;
        self
    }

    fn target(&self) -> Result<PathBuf> {
        let target = match self.mode {
            SaveMode::Save if self.has_saved_target => self.path.clone(),
            SaveMode::Save => super::edited_path(&self.original_path),
            SaveMode::ReplaceOriginal => {
                if !self
                    .original_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
                {
                    return Err(Error::Other("only WAV originals can be replaced".into()));
                }
                self.original_path.clone()
            }
            SaveMode::SaveAs | SaveMode::Selection => self
                .destination
                .clone()
                .ok_or_else(|| Error::Other("choose a destination file".into()))?,
        };
        if !target.is_absolute() { return Err(Error::Other("choose an absolute audio destination".into())); }
        super::encoding::Format::from_path(&target)?;
        if matches!(self.mode, SaveMode::SaveAs | SaveMode::Selection)
            && (same_file(&target, &self.original_path) || same_file(&target, &self.path))
        {
            return Err(Error::Other(
                "choose a different file; use Save or Replace original to update the current file"
                    .into(),
            ));
        }
        Ok(target)
    }

    /// Run on a blocking worker. The separate writer mutex covers choosing a
    /// unique destination, backup, atomic replacement and the completion callback
    /// (indexing/short live-session commit), serializing cross-window file updates.
    pub fn write_with<T>(
        &self,
        backups: &Path,
        complete: impl FnOnce(SavedFile) -> Result<T>,
    ) -> Result<T> {
        let _writer = FILE_WRITES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let target = self.target()?;
        let parent = target
            .parent()
            .ok_or_else(|| Error::Other("missing destination folder".into()))?;
        let tmp = parent.join(format!(".fonicca-{}.wav.tmp", unique_suffix()));
        OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let tmp = Temporary(tmp);
        super::encoding::write_at_rate(&tmp.0, super::encoding::Format::from_path(&target)?, &self.audio, self.bit_depth, self.loop_points, &self.markers, self.sample_rate)?;
        let backup = if target.exists() {
            fs::create_dir_all(backups)?;
            let name = target.file_name().unwrap().to_string_lossy();
            let backup = backups.join(format!("{}-{name}", unique_suffix()));
            fs::copy(&target, &backup)?;
            Some(backup)
        } else {
            None
        };
        fs::rename(&tmp.0, &target)?;
        complete(SavedFile {
            path: target,
            backup,
        })
    }

    /// Only this session can receive the result. The saved history anchor is the
    /// captured version; any edits made during the write remain dirty.
    pub fn commit(
        &self,
        session: Option<&mut Session>,
        output: &SavedFile,
        indexed_id: Option<i64>,
    ) -> Result<State> {
        let session = session.filter(|session| Arc::ptr_eq(&session.save_in_flight, &self.lease.0))
            .ok_or_else(|| Error::Other(format!("audio was saved to {}, but the editor session changed; the new session was left untouched", output.path.display())))?;
        if self.mode != SaveMode::Selection {
            session.path = output.path.clone();
            session.has_saved_target = true;
            session.export_sample_rate = self.sample_rate;
            session.saved = self.ops.clone();
            if let Some(id) = indexed_id {
                session.sample_id = id;
            }
        }
        Ok(session.state())
    }
}

#[cfg(test)]
pub(super) fn save_session(
    session: &mut Session,
    backups: &Path,
    mode: SaveMode,
    destination: Option<&Path>,
    range: Option<Span>,
) -> Result<SavedFile> {
    let snapshot = Snapshot::new(session, mode, destination, range)?;
    snapshot.write_with(backups, |output| {
        snapshot.commit(Some(session), &output, None)?;
        Ok(output)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Curve, Op};
    #[test]
    fn output_rate_preserves_duration_wav_metadata_and_repeat_save_without_changing_live_audio() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        write_wav(&source, &Pcm { channels: 1, sample_rate: 44100, samples: (0..44100).map(|i| (i as f32 * 0.05).sin() * 0.4).collect() }, Some(24), None).unwrap();
        for (ext, rate) in [("wav", 48000), ("mp3", 22050), ("ogg", 96000)] {
            let mut session = Session::open(1, &source, Some(24)).unwrap();
            session.apply(Op::SetLoop { start: 22050, end: 33075 });
            session.apply(Op::SetMarkers { markers: vec![Marker { frame: 4410, skip: false }] });
            let target = dir.path().join(format!("converted.{ext}"));
            let snapshot = Snapshot::new(&session, SaveMode::SaveAs, Some(&target), None).unwrap().with_sample_rate(Some(rate));
            snapshot.write_with(dir.path(), |output| { snapshot.commit(Some(&mut session), &output, None)?; Ok(()) }).unwrap();
            drop(snapshot);
            assert_eq!(session.export_sample_rate, Some(rate));
            assert_eq!(session.audio().sample_rate, 44100);
            assert_eq!(session.audio().frames(), 44100);
            assert_eq!(session.loop_points(), Some(Loop { start: 22050, end: 33075 }));
            session.apply(Op::Gain { start: 0, end: 44100, db: -3.0 });
            save_session(&mut session, dir.path(), SaveMode::Save, None, None).unwrap();
            let decoded = crate::audio::decode(&target, f64::INFINITY).unwrap();
            assert_eq!(decoded.sample_rate, rate);
            assert!((decoded.frames() as isize - rate as isize).unsigned_abs() < 1600);
            if ext == "wav" {
                assert_eq!(decoded.frames(), rate as usize);
                let reopened = Session::open(2, &target, Some(24)).unwrap();
                assert_eq!(reopened.loop_points(), Some(Loop { start: 24000, end: 36000 }));
                assert_eq!(reopened.markers(), &[Marker { frame: 4800, skip: false }]);
            }
        }
    }
    #[test]
    fn compressed_save_as_repeat_save_and_selection_preserve_source_and_session_history() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        let audio = Pcm { channels: 2, sample_rate: 44100, samples: (0..44100).flat_map(|i| {
            let value = (i as f32 * std::f32::consts::TAU * 440.0 / 44100.0).sin() * 0.4;
            [value, value]
        }).collect() };
        write_wav(&source, &audio, Some(24), None).unwrap();
        let original = fs::read(&source).unwrap();
        for ext in ["mp3", "ogg"] {
            let mut session = Session::open(1, &source, Some(24)).unwrap();
            session.apply(Op::Gain { start: 0, end: 44100, db: -3.0 });
            let target = dir.path().join(format!("edited.{ext}"));
            save_session(&mut session, &dir.path().join("backups"), SaveMode::SaveAs, Some(&target), None).unwrap();
            assert_eq!(session.path, target);
            assert!(session.state().can_undo && !session.state().dirty);
            let saved = fs::read(&target).unwrap();
            session.apply(Op::Gain { start: 0, end: 44100, db: -3.0 });
            let repeated = save_session(&mut session, &dir.path().join("backups"), SaveMode::Save, None, None).unwrap();
            assert_eq!(repeated.path, target);
            assert_eq!(fs::read(repeated.backup.unwrap()).unwrap(), saved);
            session.undo(); assert!(session.state().dirty);
            session.redo(); assert!(!session.state().dirty);
            let fragment = dir.path().join(format!("fragment.{ext}"));
            save_session(&mut session, dir.path(), SaveMode::Selection, Some(&fragment), Some(Span { start: 4410, end: 26460 })).unwrap();
            let decoded = crate::audio::decode(&fragment, f64::INFINITY).unwrap();
            assert!((decoded.frames() as isize - 22050).unsigned_abs() < 3000);
            assert_eq!(session.path, target);
            assert!(!session.state().dirty);
            assert_eq!(fs::read(&source).unwrap(), original);
        }
    }
    fn fixture(dir: &Path) -> Session {
        let path = dir.join("Kick.wav");
        write_wav(
            &path,
            &Pcm {
                samples: vec![0.5; 200],
                channels: 2,
                sample_rate: 1000,
            },
            Some(16),
            None,
        )
        .unwrap();
        Session::open(1, &path, Some(16)).unwrap()
    }
    #[test]
    fn first_save_protects_source_and_later_save_updates_the_same_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = fs::read(&s.path).unwrap();
        s.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        let saved = save_session(
            &mut s,
            &dir.path().join("backups"),
            SaveMode::Save,
            None,
            None,
        )
        .unwrap();
        assert_ne!(saved.path, s.original_path);
        assert_eq!(fs::read(&s.original_path).unwrap(), original);
        assert_eq!(s.path, saved.path);
        assert!(!s.state().dirty);
        assert!(s.state().can_undo);
        let first_version = fs::read(&saved.path).unwrap();
        s.apply(Op::FadeOut {
            start: 0,
            end: 100,
            curve: Curve::Linear,
        });
        let next = save_session(
            &mut s,
            &dir.path().join("backups"),
            SaveMode::Save,
            None,
            None,
        )
        .unwrap();
        assert_eq!(next.path, saved.path);
        assert_eq!(fs::read(next.backup.unwrap()).unwrap(), first_version);
        assert_ne!(fs::read(&s.path).unwrap(), first_version);
        assert!(!s.state().dirty);
        s.undo();
        assert!(s.state().dirty);
        s.redo();
        assert!(!s.state().dirty);
    }
    #[test]
    fn save_as_switches_destination_and_preserves_original_reference() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = s.path.clone();
        s.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -6.0,
        });
        let target = dir.path().join("Custom name.wav");
        save_session(
            &mut s,
            &dir.path().join("backups"),
            SaveMode::SaveAs,
            Some(&target),
            None,
        )
        .unwrap();
        assert_eq!(s.path, target);
        assert_eq!(s.original_path, original);
        assert!(!s.state().dirty);
    }
    #[test]
    fn selection_exports_only_selected_frames_and_retimes_metadata_without_marking_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        s.apply(Op::SetLoop { start: 30, end: 80 });
        s.apply(Op::SetMarkers {
            markers: vec![
                Marker {
                    frame: 10,
                    skip: false,
                },
                Marker {
                    frame: 40,
                    skip: false,
                },
                Marker {
                    frame: 90,
                    skip: false,
                },
            ],
        });
        let source = s.path.clone();
        let target = dir.path().join("Loop.wav");
        save_session(
            &mut s,
            &dir.path().join("backups"),
            SaveMode::Selection,
            Some(&target),
            Some(Span { start: 20, end: 70 }),
        )
        .unwrap();
        let result = Session::open(2, &target, Some(16)).unwrap();
        assert_eq!(result.state().frames, 50);
        assert_eq!(result.loop_points(), Some(Loop { start: 10, end: 50 }));
        assert_eq!(
            result.markers(),
            [Marker {
                frame: 20,
                skip: false
            }]
        );
        assert_eq!(s.path, source);
        assert!(s.state().dirty);
        assert!(!s.has_saved_target);
    }
    #[test]
    fn failed_writes_and_invalid_ranges_preserve_session_and_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        s.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -6.0,
        });
        let original = fs::read(&s.path).unwrap();
        let target = dir.path().join("missing/Loop.wav");
        assert!(save_session(&mut s, dir.path(), SaveMode::SaveAs, Some(&target), None).is_err());
        assert_eq!(fs::read(&s.path).unwrap(), original);
        assert!(s.state().dirty);
        assert!(!s.has_saved_target);
        let target = dir.path().join("Invalid.wav");
        assert!(save_session(
            &mut s,
            dir.path(),
            SaveMode::Selection,
            Some(&target),
            Some(Span { start: 60, end: 30 })
        )
        .is_err());
        assert!(!target.exists());
        let original_path = s.path.clone();
        assert!(save_session(
            &mut s,
            dir.path(),
            SaveMode::SaveAs,
            Some(&original_path),
            None
        )
        .is_err());
    }
    #[test]
    fn replacing_original_keeps_a_recoverable_backup_and_new_names_do_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = fs::read(&s.path).unwrap();
        fs::write(super::super::edited_path(&s.path), b"existing version").unwrap();
        s.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -6.0,
        });
        let saved = save_session(
            &mut s,
            &dir.path().join("backups"),
            SaveMode::Save,
            None,
            None,
        )
        .unwrap();
        assert!(saved.path.to_string_lossy().contains("edited 2"));
        let saved = save_session(
            &mut s,
            &dir.path().join("backups"),
            SaveMode::ReplaceOriginal,
            None,
            None,
        )
        .unwrap();
        assert_eq!(saved.path, s.original_path);
        assert_eq!(fs::read(saved.backup.unwrap()).unwrap(), original);
        assert!(!s.state().dirty);
    }
    #[test]
    fn non_wav_source_always_creates_a_wav_and_cannot_be_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let source = dir.path().join("Kick.mp3");
        fs::rename(&s.path, &source).unwrap();
        s.path = source.clone();
        s.original_path = source.clone();
        let original = fs::read(&source).unwrap();
        s.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        assert!(save_session(&mut s, dir.path(), SaveMode::ReplaceOriginal, None, None).is_err());
        let saved = save_session(&mut s, dir.path(), SaveMode::Save, None, None).unwrap();
        assert_eq!(saved.path.extension().unwrap(), "wav");
        assert_eq!(fs::read(source).unwrap(), original);
        assert!(!s.state().dirty);
    }
    #[test]
    fn backup_failure_keeps_the_existing_file_and_dirty_state_and_removes_temporary_output() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = fs::read(&s.path).unwrap();
        let blocked = dir.path().join("not-a-directory");
        fs::write(&blocked, b"blocked").unwrap();
        s.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        assert!(save_session(&mut s, &blocked, SaveMode::ReplaceOriginal, None, None).is_err());
        assert_eq!(fs::read(&s.path).unwrap(), original);
        assert!(s.state().dirty);
        assert!(!fs::read_dir(dir.path()).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn pending_write_leaves_editor_accessible_and_only_marks_its_captured_version_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut source = fixture(dir.path());
        source.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        source.apply(Op::SetLoop { start: 20, end: 80 });
        source.apply(Op::SetMarkers {
            markers: vec![Marker {
                frame: 10,
                skip: false,
            }],
        });
        let snapshot = Snapshot::new(&source, SaveMode::Save, None, None).unwrap();
        assert!(source
            .audio()
            .samples
            .same_snapshot(&snapshot.audio.samples));
        let before = source.audio().samples[0];
        let live = Arc::new(Mutex::new(source));
        let writer_session = live.clone();
        let backups = dir.path().join("backups");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let writer = std::thread::spawn(move || {
            snapshot.write_with(&backups, |output| {
                ready_tx.send(()).unwrap();
                resume_rx.recv().unwrap();
                let mut session = writer_session.lock().unwrap();
                snapshot.commit(Some(&mut session), &output, Some(42))?;
                Ok(output)
            })
        });
        ready_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        {
            let mut session = live
                .try_lock()
                .expect("file writer must not hold the live editor mutex");
            session.apply(Op::Gain {
                start: 0,
                end: 100,
                db: -6.0,
            });
            session.apply(Op::SetLoop { start: 1, end: 40 });
            session.apply(Op::SetMarkers {
                markers: vec![Marker {
                    frame: 70,
                    skip: true,
                }],
            });
        }
        resume_tx.send(()).unwrap();
        let output = writer.join().unwrap().unwrap();
        let written = Session::open(9, &output.path, Some(16)).unwrap();
        assert!((written.audio().samples[0] - before).abs() < 0.00004);
        assert_eq!(written.loop_points(), Some(Loop { start: 20, end: 80 }));
        assert_eq!(
            written.markers(),
            [Marker {
                frame: 10,
                skip: false
            }]
        );
        let mut session = live.lock().unwrap();
        assert_eq!(session.sample_id, 42);
        assert_eq!(session.path, output.path);
        assert!(session.state().dirty, "new edits must not be marked saved");
        assert_eq!(session.loop_points(), Some(Loop { start: 1, end: 40 }));
        assert_eq!(
            session.markers(),
            [Marker {
                frame: 70,
                skip: true
            }]
        );
        for _ in 0..3 {
            session.undo();
        }
        assert!(
            !session.state().dirty,
            "undo to the written version must reach the saved anchor"
        );
        for _ in 0..3 {
            session.redo();
        }
        assert!(session.state().dirty);
    }

    #[test]
    fn lease_rejects_overlapping_saves_and_is_released_after_validation_or_write_failure() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = fixture(dir.path());
        assert!(Snapshot::new(
            &session,
            SaveMode::Selection,
            None,
            Some(Span { start: 30, end: 10 })
        )
        .is_err());
        let pending = Snapshot::new(&session, SaveMode::Save, None, None).unwrap();
        assert!(Snapshot::new(&session, SaveMode::Save, None, None).is_err());
        drop(pending);
        let missing = dir.path().join("missing/Bad.wav");
        assert!(save_session(
            &mut session,
            dir.path(),
            SaveMode::SaveAs,
            Some(&missing),
            None
        )
        .is_err());
        save_session(
            &mut session,
            &dir.path().join("backups"),
            SaveMode::Save,
            None,
            None,
        )
        .unwrap();
        assert!(!session.state().dirty);
        let next = Snapshot::new(&session, SaveMode::Save, None, None).unwrap();
        drop(next);
    }

    #[test]
    fn closed_or_reopened_session_cannot_receive_an_older_save_result() {
        let dir = tempfile::tempdir().unwrap();
        let source = fixture(dir.path());
        let path = source.path.clone();
        let snapshot = Snapshot::new(&source, SaveMode::Save, None, None).unwrap();
        drop(source);
        let mut reopened = Session::open(1, &path, Some(16)).unwrap();
        let before = reopened.audio().clone();
        snapshot
            .write_with(&dir.path().join("backups"), |output| {
                assert!(output.path.exists());
                let error = snapshot
                    .commit(Some(&mut reopened), &output, Some(99))
                    .unwrap_err()
                    .to_string();
                assert!(
                    error.contains(output.path.to_str().unwrap()),
                    "a completed file must remain discoverable in the error"
                );
                assert!(snapshot.commit(None, &output, Some(99)).is_err());
                assert!(before.samples.same_snapshot(&reopened.audio().samples));
                assert_eq!(reopened.path, path);
                assert_eq!(reopened.sample_id, 1);
                assert!(!reopened.has_saved_target);
                Ok(())
            })
            .unwrap();
        assert!(Snapshot::new(&reopened, SaveMode::Save, None, None).is_ok());
    }

    #[test]
    fn simultaneous_windows_choose_distinct_first_save_paths_and_preserve_each_audio() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = fixture(dir.path());
        let mut second = Session::open(2, &first.path, Some(16)).unwrap();
        first.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        second.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -9.0,
        });
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let workers: Vec<_> = [first, second]
            .into_iter()
            .map(|mut session| {
                let barrier = barrier.clone();
                let backups = dir.path().join("backups");
                std::thread::spawn(move || {
                    let snapshot = Snapshot::new(&session, SaveMode::Save, None, None).unwrap();
                    barrier.wait();
                    snapshot
                        .write_with(&backups, |output| {
                            snapshot.commit(Some(&mut session), &output, None)?;
                            let written = Session::open(99, &output.path, Some(16))?;
                            assert!(
                                (written.audio().samples[0] - session.audio().samples[0]).abs()
                                    < 0.00004
                            );
                            Ok(output.path)
                        })
                        .unwrap()
                })
            })
            .collect();
        barrier.wait();
        let paths: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_ne!(paths[0], paths[1]);
        assert!(paths.iter().all(|path| path.exists()));
    }

    #[test]
    fn indexed_selection_never_switches_the_source_or_marks_new_changes_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = fixture(dir.path());
        session.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        let path = session.path.clone();
        let output = dir.path().join("Selected.wav");
        let snapshot = Snapshot::new(
            &session,
            SaveMode::Selection,
            Some(&output),
            Some(Span { start: 10, end: 80 }),
        )
        .unwrap();
        session.apply(Op::Delete { start: 0, end: 5 });
        snapshot
            .write_with(dir.path(), |saved| {
                let state = snapshot.commit(Some(&mut session), &saved, Some(99))?;
                assert_eq!(state.sample_id, 1);
                assert!(state.dirty);
                assert_eq!(state.frames, 95);
                Ok(())
            })
            .unwrap();
        assert_eq!(session.path, path);
        assert!(!session.has_saved_target);
        assert_eq!(
            Session::open(99, &output, Some(16))
                .unwrap()
                .audio()
                .frames(),
            70
        );
    }
}
