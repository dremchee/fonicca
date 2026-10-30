//! Export immutable regions without holding the editor map during file I/O.
use super::{blocks::BlockAudio, write_wav, Session, Span};
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub struct Snapshot {
    audio: BlockAudio,
    path: PathBuf,
    bit_depth: Option<u32>,
    spans: Vec<Span>,
}
impl Snapshot {
    pub fn new(session: &Session, spans: Vec<Span>) -> Result<Self> {
        if spans.is_empty() {
            return Err(Error::Other("nothing to export".into()));
        }
        Ok(Self {
            audio: session.audio().clone(),
            path: session.path.clone(),
            bit_depth: session.bit_depth,
            spans,
        })
    }

    pub fn write(self) -> Result<(PathBuf, usize)> {
        let parent = self.path.parent().unwrap_or(Path::new("."));
        let stem = self
            .path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Atomic reservation: simultaneous windows must never share an output
        // directory. Recreate a missing parent as the previous exporter did.
        std::fs::create_dir_all(parent)?;
        let dir = (1..)
            .find_map(|n| {
                let path = parent.join(if n == 1 {
                    format!("{stem} slices")
                } else {
                    format!("{stem} slices {n}")
                });
                match std::fs::create_dir(&path) {
                    Ok(()) => Some(Ok(path)),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .expect("some name is free")?;
        let width = self.spans.len().to_string().len().max(2);
        for (i, span) in self.spans.iter().enumerate() {
            let (a, b) = (
                span.start.min(self.audio.frames()),
                span.end.min(self.audio.frames()),
            );
            let audio = self.audio.slice_frames(a, b.max(a));
            write_wav(
                &dir.join(format!("{stem} {:0width$}.wav", i + 1)),
                &audio,
                self.bit_depth,
                None,
            )?;
        }
        Ok((dir, self.spans.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio::{self, AudioRead, Pcm},
        editor::{Loop, Marker, Op},
    };

    fn fixture() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Drum loop.wav");
        let samples = (0..200).map(|n| n as f32 / 1000.0).collect();
        super::super::write_wav(
            &path,
            &Pcm {
                samples,
                channels: 2,
                sample_rate: 1000,
            },
            Some(32),
            None,
        )
        .unwrap();
        let session = Session::open(1, &path, Some(32)).unwrap();
        (dir, session)
    }

    #[test]
    fn export_uses_captured_edits_without_switching_source_or_copying_the_whole_audio() {
        let (_dir, mut session) = fixture();
        session.apply(Op::Gain {
            start: 20,
            end: 50,
            db: -3.0,
        });
        session.apply(Op::SetLoop { start: 25, end: 45 });
        session.apply(Op::SetMarkers {
            markers: vec![Marker {
                frame: 30,
                skip: true,
            }],
        });
        let source = session.audio().clone();
        let path = session.path.clone();
        let snapshot = Snapshot::new(
            &session,
            vec![
                Span { start: 20, end: 50 },
                Span {
                    start: 70,
                    end: 500,
                },
            ],
        )
        .unwrap();
        assert!(snapshot.audio.samples.same_snapshot(&source.samples));
        session.apply(Op::Silence { start: 0, end: 100 });
        let (dir, count) = snapshot.write().unwrap();
        assert_eq!(count, 2);
        assert_eq!(session.path, path);
        assert_eq!(session.sample_id, 1);
        assert!(session.state().dirty);
        assert!(!session.has_saved_target);
        for (i, (start, end)) in [(20, 50), (70, 100)].into_iter().enumerate() {
            let path = dir.join(format!("Drum loop {:02}.wav", i + 1));
            let exported = Session::open(2, &path, Some(32)).unwrap();
            assert_eq!(
                exported.audio().samples,
                source.range(start * 2..end * 2).into_owned()
            );
            assert_eq!(exported.loop_points(), None::<Loop>);
            assert!(exported.markers().is_empty());
        }
    }

    #[test]
    fn simultaneous_exports_reserve_distinct_directories_and_outlive_the_source() {
        let (_dir, session) = fixture();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let snapshots = [
            Snapshot::new(&session, vec![Span { start: 0, end: 100 }]).unwrap(),
            Snapshot::new(&session, vec![Span { start: 10, end: 90 }]).unwrap(),
        ];
        drop(session);
        let workers: Vec<_> = snapshots
            .into_iter()
            .map(|snapshot| {
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    snapshot.write().unwrap().0
                })
            })
            .collect();
        barrier.wait();
        let dirs: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_ne!(dirs[0], dirs[1]);
        assert_eq!(
            audio::decode(&dirs[0].join("Drum loop 01.wav"), 10.0)
                .unwrap()
                .frames(),
            100
        );
        assert_eq!(
            audio::decode(&dirs[1].join("Drum loop 01.wav"), 10.0)
                .unwrap()
                .frames(),
            80
        );
    }

    #[test]
    fn numbering_empty_export_and_clamped_empty_spans_keep_the_previous_policy() {
        let (_dir, session) = fixture();
        assert!(Snapshot::new(&session, vec![]).is_err());
        let mut spans = vec![Span { start: 0, end: 1 }; 99];
        spans.push(Span {
            start: usize::MAX,
            end: 0,
        });
        let (dir, count) = Snapshot::new(&session, spans).unwrap().write().unwrap();
        assert_eq!(count, 100);
        assert!(dir.join("Drum loop 001.wav").exists());
        let empty = hound::WavReader::open(dir.join("Drum loop 100.wav")).unwrap();
        assert_eq!(empty.duration(), 0);
    }

    #[test]
    fn failed_export_leaves_live_audio_and_history_untouched() {
        let (dir, mut session) = fixture();
        session.apply(Op::Gain {
            start: 0,
            end: 100,
            db: -3.0,
        });
        let audio = session.audio().clone();
        let state = serde_json::to_value(session.state()).unwrap();
        let blocked = dir.path().join("blocked");
        std::fs::write(&blocked, b"not a folder").unwrap();
        session.path = blocked.join("Track.wav");
        let snapshot = Snapshot::new(&session, vec![Span { start: 0, end: 50 }]).unwrap();
        assert!(snapshot.write().is_err());
        assert!(audio.samples.same_snapshot(&session.audio().samples));
        assert_eq!(serde_json::to_value(session.state()).unwrap(), state);
    }
}
