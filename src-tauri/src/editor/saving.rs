//! File saving policy shared by the editor commands and filesystem tests.
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::{append_markers, write_wav, Loop, Marker, Session, Span};
#[cfg(test)] use crate::audio::Pcm;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveMode { Save, SaveAs, ReplaceOriginal, Selection }

pub struct SavedFile { pub path: PathBuf, pub backup: Option<PathBuf> }
static SERIAL: AtomicU64 = AtomicU64::new(0);
fn unique_suffix() -> String {
    format!("{}-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(), SERIAL.fetch_add(1, Ordering::Relaxed))
}
fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// The first Save creates a version; later Saves update that version. Exports
/// never switch sessions or mark the whole file saved.
pub fn save_session(session: &mut Session, backups: &Path, mode: SaveMode, destination: Option<&Path>, range: Option<Span>) -> Result<SavedFile> {
    let target = match mode {
        SaveMode::Save if session.has_saved_target => session.path.clone(),
        SaveMode::Save => super::edited_path(&session.original_path),
        SaveMode::ReplaceOriginal => {
            if !session.state().can_replace_original { return Err(Error::Other("only WAV originals can be replaced".into())); }
            session.original_path.clone()
        }
        SaveMode::SaveAs | SaveMode::Selection => destination.ok_or_else(|| Error::Other("choose a destination file".into()))?.to_path_buf(),
    };
    if !target.is_absolute() || !target.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("wav")) {
        return Err(Error::Other("choose an absolute WAV destination".into()));
    }
    if matches!(mode, SaveMode::SaveAs | SaveMode::Selection) && (same_file(&target, &session.original_path) || same_file(&target, &session.path)) {
        return Err(Error::Other("choose a different file; use Save or Replace original to update the current file".into()));
    }
    let selected;
    let (pcm, lp, markers) = if mode == SaveMode::Selection {
        let Span { start, end } = range.ok_or_else(|| Error::Other("select a non-empty fragment".into()))?;
        let pcm = session.audio();
        if start >= end || end > pcm.frames() { return Err(Error::Other("select a non-empty fragment inside the file".into())); }
        selected = pcm.slice_frames(start,end);
        let lp = session.loop_points().map(|lp| Loop { start: lp.start.clamp(start, end) - start, end: lp.end.clamp(start, end) - start }).filter(|lp| lp.start < lp.end);
        let markers = session.markers().iter().filter(|m| m.frame >= start && m.frame < end).map(|m| Marker { frame: m.frame - start, skip: m.skip }).collect::<Vec<_>>();
        (&selected, lp, markers)
    } else { (session.audio(), session.loop_points(), session.markers().to_vec()) };
    let parent = target.parent().ok_or_else(|| Error::Other("missing destination folder".into()))?;
    let tmp = parent.join(format!(".fonicca-{}.wav.tmp", unique_suffix()));
    OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let result: Result<SavedFile> = (|| {
        write_wav(&tmp, pcm, session.bit_depth, lp)?;
        append_markers(&tmp, &markers)?;
        let backup = if target.exists() {
            fs::create_dir_all(backups)?;
            let name = target.file_name().unwrap().to_string_lossy();
            let backup = backups.join(format!("{}-{name}", unique_suffix()));
            fs::copy(&target, &backup)?;
            Some(backup)
        } else { None };
        // A failed write leaves the existing file and session untouched.
        fs::rename(&tmp, &target)?;
        Ok(SavedFile { path: target.clone(), backup })
    })();
    if result.is_err() { let _ = fs::remove_file(&tmp); }
    let saved = result?;
    if mode != SaveMode::Selection {
        session.path = target;
        session.has_saved_target = true;
        session.mark_saved();
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Op, Curve};
    fn fixture(dir: &Path) -> Session {
        let path = dir.join("Kick.wav");
        write_wav(&path, &Pcm { samples: vec![0.5; 200], channels: 2, sample_rate: 1000 }, Some(16), None).unwrap();
        Session::open(1, &path, Some(16)).unwrap()
    }
    #[test]
    fn first_save_protects_source_and_later_save_updates_the_same_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = fs::read(&s.path).unwrap();
        s.apply(Op::Gain { start: 0, end: 100, db: -3.0 });
        let saved = save_session(&mut s, &dir.path().join("backups"), SaveMode::Save, None, None).unwrap();
        assert_ne!(saved.path, s.original_path);
        assert_eq!(fs::read(&s.original_path).unwrap(), original);
        assert_eq!(s.path, saved.path);
        assert!(!s.state().dirty);
        assert!(s.state().can_undo);
        let first_version = fs::read(&saved.path).unwrap();
        s.apply(Op::FadeOut { start: 0, end: 100, curve: Curve::Linear });
        let next = save_session(&mut s, &dir.path().join("backups"), SaveMode::Save, None, None).unwrap();
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
        s.apply(Op::Gain { start: 0, end: 100, db: -6.0 });
        let target = dir.path().join("Custom name.wav");
        save_session(&mut s, &dir.path().join("backups"), SaveMode::SaveAs, Some(&target), None).unwrap();
        assert_eq!(s.path, target);
        assert_eq!(s.original_path, original);
        assert!(!s.state().dirty);
    }
    #[test]
    fn selection_exports_only_selected_frames_and_retimes_metadata_without_marking_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        s.apply(Op::SetLoop { start: 30, end: 80 });
        s.apply(Op::SetMarkers { markers: vec![Marker { frame: 10, skip: false }, Marker { frame: 40, skip: false }, Marker { frame: 90, skip: false }] });
        let source = s.path.clone();
        let target = dir.path().join("Loop.wav");
        save_session(&mut s, &dir.path().join("backups"), SaveMode::Selection, Some(&target), Some(Span { start: 20, end: 70 })).unwrap();
        let result = Session::open(2, &target, Some(16)).unwrap();
        assert_eq!(result.state().frames, 50);
        assert_eq!(result.loop_points(), Some(Loop { start: 10, end: 50 }));
        assert_eq!(result.markers(), [Marker { frame: 20, skip: false }]);
        assert_eq!(s.path, source);
        assert!(s.state().dirty);
        assert!(!s.has_saved_target);
    }
    #[test]
    fn failed_writes_and_invalid_ranges_preserve_session_and_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        s.apply(Op::Gain { start: 0, end: 100, db: -6.0 });
        let original = fs::read(&s.path).unwrap();
        let target = dir.path().join("missing/Loop.wav");
        assert!(save_session(&mut s, dir.path(), SaveMode::SaveAs, Some(&target), None).is_err());
        assert_eq!(fs::read(&s.path).unwrap(), original);
        assert!(s.state().dirty);
        assert!(!s.has_saved_target);
        let target = dir.path().join("Invalid.wav");
        assert!(save_session(&mut s, dir.path(), SaveMode::Selection, Some(&target), Some(Span { start: 60, end: 30 })).is_err());
        assert!(!target.exists());
        let original_path = s.path.clone();
        assert!(save_session(&mut s, dir.path(), SaveMode::SaveAs, Some(&original_path), None).is_err());
    }
    #[test]
    fn replacing_original_keeps_a_recoverable_backup_and_new_names_do_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = fixture(dir.path());
        let original = fs::read(&s.path).unwrap();
        fs::write(super::super::edited_path(&s.path), b"existing version").unwrap();
        s.apply(Op::Gain { start: 0, end: 100, db: -6.0 });
        let saved = save_session(&mut s, &dir.path().join("backups"), SaveMode::Save, None, None).unwrap();
        assert!(saved.path.to_string_lossy().contains("edited 2"));
        let saved = save_session(&mut s, &dir.path().join("backups"), SaveMode::ReplaceOriginal, None, None).unwrap();
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
        s.apply(Op::Gain { start: 0, end: 100, db: -3.0 });
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
        s.apply(Op::Gain { start: 0, end: 100, db: -3.0 });
        assert!(save_session(&mut s, &blocked, SaveMode::ReplaceOriginal, None, None).is_err());
        assert_eq!(fs::read(&s.path).unwrap(), original);
        assert!(s.state().dirty);
        assert!(!fs::read_dir(dir.path()).unwrap().any(|entry| entry.unwrap().path().to_string_lossy().ends_with(".tmp")));
    }
}
