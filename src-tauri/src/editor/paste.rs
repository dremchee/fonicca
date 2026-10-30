//! Prepare clipboard conversion and piece splicing outside the session mutex.
use super::{blocks::BlockAudio, clipboard::Clip, Op, Session};
use crate::error::{Error, Result};

pub struct Snapshot {
    audio: BlockAudio,
    clip: Clip,
    revision: u64,
    start: usize,
    end: usize,
}

pub struct Rendered {
    pub audio: BlockAudio,
    pub op: Op,
    pub revision: u64,
    pub start: usize,
    pub end: usize,
}

impl Snapshot {
    pub fn new(session: &Session, clip: Clip, start: usize, end: usize) -> Self {
        let frames = session.audio().frames();
        let start = start.min(frames);
        Self {
            audio: session.audio().clone(),
            clip,
            revision: session.revision(),
            start,
            end: end.clamp(start, frames),
        }
    }

    pub fn render(mut self) -> Result<Rendered> {
        let channels = self.audio.channels;
        let samples = self.clip.render(channels, self.audio.sample_rate)?;
        let frames = samples.len() / channels.max(1);
        let result_frames = self
            .audio
            .frames()
            .checked_sub(self.end - self.start)
            .and_then(|n| n.checked_add(frames))
            .ok_or_else(|| Error::Other("audio length is too large".into()))?;
        super::memory::check_growth(self.audio.frames(), result_frames, channels)?;
        let op = Op::Replace {
            start: self.start,
            end: self.end,
            samples,
        };
        self.audio.try_apply(&op)?;
        Ok(Rendered {
            audio: self.audio,
            op,
            revision: self.revision,
            start: self.start,
            end: self.start + frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioRead, Pcm};
    use crate::editor::{clipboard, Curve, Loop, Marker};
    use std::sync::Arc;

    fn session(channels: usize, rate: u32) -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("paste.wav");
        let pcm = Pcm {
            samples: (0..1000 * channels)
                .map(|n| (n as f32 * 0.013).sin())
                .collect(),
            channels,
            sample_rate: rate,
        };
        crate::editor::write_wav(&path, &pcm, Some(32), None).unwrap();
        let session = Session::open(1, &path, Some(32)).unwrap();
        (dir, session)
    }

    fn clipboard(channels: usize, rate: u32) -> (Pcm, Clip) {
        let pcm = Pcm {
            samples: (0..83 * channels).map(|n| n as f32 / 200.0).collect(),
            channels,
            sample_rate: rate,
        };
        let clip = Clip::from_range(&pcm, 0, pcm.frames()).unwrap();
        (pcm, clip)
    }

    #[test]
    fn worker_paste_matches_reference_selection_metadata_and_single_step_history() {
        for channels in [1, 2, 4] {
            for (clip_channels, clip_rate) in [(1, 22050), (2, 44100), (4, 48000)] {
                for (start, end) in [
                    (0, 0),
                    (100, 400),
                    (250, 100),
                    (800, usize::MAX),
                    (usize::MAX, 0),
                ] {
                    let (_dir, mut actual) = session(channels, 44100);
                    actual.apply(Op::Delete { start: 10, end: 20 });
                    actual.apply(Op::FadeIn {
                        start: 500,
                        end: 700,
                        curve: Curve::Smooth,
                    });
                    actual.apply(Op::SetLoop {
                        start: 200,
                        end: 600,
                    });
                    actual.apply(Op::SetMarkers {
                        markers: vec![
                            Marker {
                                frame: 80,
                                skip: false,
                            },
                            Marker {
                                frame: 500,
                                skip: true,
                            },
                            Marker {
                                frame: 900,
                                skip: false,
                            },
                        ],
                    });
                    let source = actual.audio().clone();
                    let before = actual.state();
                    let history = actual.history.len();
                    let (_pcm, clip) = clipboard(clip_channels, clip_rate);
                    let samples = clipboard::reference_convert(&clip, channels, 44100);
                    let pasted_frames = samples.len() / channels;
                    let snapshot = Snapshot::new(&actual, clip, start, end);
                    let start = start.min(source.frames());
                    let end = end.clamp(start, source.frames());
                    let expected_op = Op::Replace {
                        start,
                        end,
                        samples: Arc::new(samples),
                    };
                    let mut expected = Pcm {
                        samples: source.iter_samples().copied().collect(),
                        channels,
                        sample_rate: 44100,
                    };
                    crate::editor::apply(&mut expected, &expected_op);
                    let expected_loop = crate::editor::loops::moved_loop(
                        before.loop_points,
                        &expected_op,
                        source.frames(),
                        channels,
                    );
                    let expected_markers = crate::editor::markers::moved_markers(
                        &before.markers,
                        &expected_op,
                        source.frames(),
                        channels,
                    );
                    assert!(source.samples.same_snapshot(&snapshot.audio.samples));
                    let rendered = snapshot.render().unwrap();
                    assert!(
                        source.samples.same_snapshot(&actual.audio().samples),
                        "worker must leave live audio intact"
                    );
                    assert_eq!(
                        (rendered.start, rendered.end),
                        (start, start + pasted_frames)
                    );
                    actual
                        .apply_rendered(rendered.revision, rendered.op, rendered.audio)
                        .unwrap();
                    assert_eq!(actual.audio().samples, expected.samples);
                    assert_eq!(actual.loop_points(), expected_loop);
                    assert_eq!(actual.markers(), expected_markers);
                    assert_eq!(actual.history.len(), history + 1);
                    let pasted = actual.audio().clone();
                    actual.undo();
                    assert!(source.samples.same_snapshot(&actual.audio().samples));
                    assert_eq!(
                        actual.loop_points(),
                        Some(Loop {
                            start: 200,
                            end: 600
                        })
                    );
                    assert_eq!(actual.markers(), before.markers);
                    actual.redo();
                    assert!(pasted.samples.same_snapshot(&actual.audio().samples));
                }
            }
        }
    }

    #[test]
    fn repeated_same_format_pastes_share_one_clipboard_buffer() {
        let (_dir, mut session) = session(2, 44100);
        let (_pcm, clip) = clipboard(2, 44100);
        let samples = clip.render(2, 44100).unwrap();
        let mut ram = None;
        for position in [0, 300, 800] {
            let output = Snapshot::new(&session, clip.clone(), position, position)
                .render()
                .unwrap();
            assert!(
                matches!(&output.op, Op::Replace { samples: inserted, .. } if Arc::ptr_eq(inserted, &samples))
            );
            session
                .apply_rendered(output.revision, output.op, output.audio)
                .unwrap();
            let bytes = serde_json::to_value(session.diagnostics()).unwrap()["ramBytes"]
                .as_u64()
                .unwrap();
            if let Some(before) = ram {
                assert_eq!(
                    bytes, before,
                    "another paste must not duplicate the stored PCM"
                );
            }
            ram = Some(bytes);
        }
    }

    #[test]
    fn newer_edit_undo_and_reopened_session_reject_late_paste_results() {
        let (dir, mut session) = session(1, 44100);
        let (_pcm, clip) = clipboard(2, 48000);
        for undo in [false, true] {
            let pending = Snapshot::new(&session, clip.clone(), 20, 100);
            session.apply(Op::Delete {
                start: 300,
                end: 400,
            });
            if undo {
                session.undo();
            }
            let current = session.audio().clone();
            let revision = session.revision();
            let history = session.history.len();
            let output = pending.render().unwrap();
            assert!(session
                .apply_rendered(output.revision, output.op, output.audio)
                .is_err());
            assert!(current.samples.same_snapshot(&session.audio().samples));
            assert_eq!(session.revision(), revision);
            assert_eq!(session.history.len(), history);
        }
        let pending = Snapshot::new(&session, clip, 10, 100);
        drop(session);
        let output = pending.render().unwrap();
        let mut reopened = Session::open(1, &dir.path().join("paste.wav"), Some(32)).unwrap();
        assert!(reopened
            .apply_rendered(output.revision, output.op, output.audio)
            .is_err());
        assert!(!reopened.state().dirty);
    }
}
