//! Loop points (the WAV `smpl` chunk) and zero-crossing search.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use serde::Serialize;

use super::Op;
use crate::audio::Pcm;
use crate::error::Result;

/// Loop points as `[start, end)` frames.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Loop {
    pub start: usize,
    pub end: usize,
}

/// Where the loop is after `op`, applied to a buffer of `frames` frames of
/// `channels` channels: cuts and pastes move it with the audio around it, and a
/// loop cut down to nothing is gone.
pub fn moved_loop(lp: Option<Loop>, op: &Op, frames: usize, channels: usize) -> Option<Loop> {
    let (start, end) = op.range();
    let (start, end) = (start.min(frames), end.min(frames).max(start.min(frames)));
    let lp = match *op {
        Op::SetLoop { .. } => Some(Loop { start, end }),
        Op::ClearLoop => None,
        Op::Crop { .. } => {
            lp.map(|l| Loop { start: l.start.clamp(start, end) - start, end: l.end.clamp(start, end) - start })
        }
        Op::Delete { .. } => {
            let map = |p: usize| {
                if p <= start {
                    p
                } else if p >= end {
                    p - (end - start)
                } else {
                    start
                }
            };
            lp.map(|l| Loop { start: map(l.start), end: map(l.end) })
        }
        // Points after the replaced range shift by the size difference; points inside
        // it go to its start.
        Op::Replace { ref samples, .. } => {
            let inserted = samples.len() / channels.max(1);
            let map = |p: usize| {
                if p <= start {
                    p
                } else if p >= end {
                    p - (end - start) + inserted
                } else {
                    start
                }
            };
            lp.map(|l| Loop { start: map(l.start), end: map(l.end) })
        }
        _ => lp,
    };
    lp.filter(|l| l.end > l.start)
}

/// The zero crossing of the mixed-down signal nearest to `frame`, within `radius`
/// frames; `frame` itself when there is none. Of the two frames around a sign
/// change, the one closer to zero. The end of the file counts as an edge of its own.
pub fn zero_crossing(pcm: &Pcm, frame: usize, radius: usize) -> usize {
    let ch = pcm.channels.max(1);
    let frames = pcm.frames();
    let frame = frame.min(frames);
    if frames < 2 || frame == frames {
        return frame;
    }
    let mix = |f: usize| pcm.samples[f * ch..(f + 1) * ch].iter().sum::<f32>();
    // A crossing between frames f and f + 1.
    let crossing = |f: usize| {
        let (a, b) = (mix(f), mix(f + 1));
        if a == 0.0 {
            Some(f)
        } else if b == 0.0 || (a < 0.0) != (b < 0.0) {
            Some(if a.abs() <= b.abs() { f } else { f + 1 })
        } else {
            None
        }
    };
    for d in 0..=radius {
        let candidates = [frame.checked_sub(d), (d > 0).then_some(frame + d)];
        for f in candidates.into_iter().flatten().filter(|&f| f + 1 < frames) {
            if let Some(z) = crossing(f) {
                return z;
            }
        }
    }
    frame
}

/// Appends a `smpl` chunk with one forward loop and fixes up the RIFF size.
pub fn append_smpl(path: &Path, lp: Loop, sample_rate: u32) -> Result<()> {
    let words: [u32; 15] = [
        0,                                  // manufacturer
        0,                                  // product
        1_000_000_000 / sample_rate.max(1), // sample period, ns
        60,                                 // MIDI unity note (C4)
        0,                                  // pitch fraction
        0,                                  // SMPTE format
        0,                                  // SMPTE offset
        1,                                  // number of loops
        0,                                  // sampler data size
        0,                                  // loop: cue point id
        0,                                  // loop: type, 0 = forward
        lp.start as u32,                    // loop: first frame
        lp.end.saturating_sub(1) as u32,    // loop: last frame (inclusive)
        0,                                  // loop: fraction
        0,                                  // loop: play count, 0 = forever
    ];
    let data: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    append_chunk(path, b"smpl", &data)
}

/// Appends a chunk to a RIFF file and fixes up the RIFF size.
pub fn append_chunk(path: &Path, id: &[u8; 4], data: &[u8]) -> Result<()> {
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    // Chunks start on even offsets.
    if f.seek(SeekFrom::End(0))? % 2 == 1 {
        f.write_all(&[0])?;
    }
    f.write_all(id)?;
    f.write_all(&(data.len() as u32).to_le_bytes())?;
    f.write_all(data)?;
    let riff_size = (f.stream_position()? - 8) as u32;
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&riff_size.to_le_bytes())?;
    Ok(())
}

/// The first loop of a WAV's `smpl` chunk, if any.
pub fn read_smpl_loop(path: &Path) -> Option<Loop> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let mut chunk = [0u8; 8];
    while f.read_exact(&mut chunk).is_ok() {
        let size = u32::from_le_bytes(chunk[4..8].try_into().ok()?) as usize;
        if &chunk[0..4] != b"smpl" {
            f.seek(SeekFrom::Current((size + size % 2) as i64)).ok()?;
            continue;
        }
        let mut data = vec![0u8; size.min(1 << 16)];
        f.read_exact(&mut data).ok()?;
        let word = |i: usize| data.get(i * 4..i * 4 + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
        // 36-byte header (word 7: number of loops), then 24-byte loops:
        // id, type, start, end (inclusive), fraction, play count.
        if word(7)? == 0 {
            return None;
        }
        let (start, last) = (word(11)?, word(12)?);
        return (last >= start).then_some(Loop { start, end: last + 1 });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::{write_wav, Session};
    use super::*;
    use crate::audio;

    fn ramp(frames: usize) -> Pcm {
        Pcm { samples: (0..frames).map(|i| (i + 1) as f32 / 1000.0).collect(), channels: 1, sample_rate: 1000 }
    }

    #[test]
    fn loop_points_follow_cuts_and_undo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, &ramp(100), Some(16), None).unwrap();
        let mut s = Session::open(1, &path, Some(16)).unwrap();
        assert_eq!(s.loop_points(), None);

        s.apply(Op::SetLoop { start: 20, end: 60 });
        assert_eq!(s.loop_points(), Some(Loop { start: 20, end: 60 }));
        assert!(s.state().dirty);
        // A cut before the loop shifts it; a cut across its end shortens it.
        s.apply(Op::Delete { start: 0, end: 10 });
        assert_eq!(s.loop_points(), Some(Loop { start: 10, end: 50 }));
        s.apply(Op::Delete { start: 40, end: 70 });
        assert_eq!(s.loop_points(), Some(Loop { start: 10, end: 40 }));
        s.apply(Op::Crop { start: 15, end: 30 });
        assert_eq!(s.loop_points(), Some(Loop { start: 0, end: 15 }));
        // Cropped away entirely: gone.
        s.apply(Op::Crop { start: 20, end: 30 });
        assert_eq!(s.loop_points(), None);

        s.undo();
        assert_eq!(s.loop_points(), Some(Loop { start: 0, end: 15 }));
        s.apply(Op::ClearLoop);
        assert_eq!(s.loop_points(), None);
        for _ in 0..4 {
            s.undo();
        }
        assert_eq!(s.loop_points(), Some(Loop { start: 20, end: 60 }));
        s.redo();
        assert_eq!(s.loop_points(), Some(Loop { start: 10, end: 50 }));
    }

    #[test]
    fn pastes_replace_audio_and_shift_the_loop() {
        let lp = Some(Loop { start: 20, end: 40 });
        let ten = std::sync::Arc::new(vec![0.0f32; 10]);
        // Inserting 10 frames before the loop moves it by 10.
        assert_eq!(
            moved_loop(lp, &Op::Replace { start: 5, end: 5, samples: ten.clone() }, 100, 1),
            Some(Loop { start: 30, end: 50 })
        );
        // In stereo the same 10 samples are 5 frames.
        assert_eq!(
            moved_loop(lp, &Op::Replace { start: 0, end: 0, samples: ten.clone() }, 100, 2),
            Some(Loop { start: 25, end: 45 })
        );
        // Replacing 30..60 with 10 frames: the end, inside, goes to 30.
        assert_eq!(moved_loop(lp, &Op::Replace { start: 30, end: 60, samples: ten }, 100, 1), Some(Loop { start: 20, end: 30 }));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_wav(&path, &ramp(100), Some(16), None).unwrap();
        let mut s = Session::open(1, &path, Some(16)).unwrap();
        s.apply(Op::Replace { start: 10, end: 20, samples: std::sync::Arc::new(vec![0.5; 5]) });
        assert_eq!(s.state().frames, 95);
        assert_eq!(&s.audio().samples[9..16], &[s.audio().samples[9], 0.5, 0.5, 0.5, 0.5, 0.5, s.audio().samples[15]]);
        assert!((s.audio().samples[15] - 0.021).abs() < 1e-3, "frame 20 follows the pasted part");
        s.undo();
        assert_eq!(s.state().frames, 100);
        s.redo();
        assert_eq!(s.state().frames, 95);
    }

    #[test]
    fn smpl_loop_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loop.wav");
        // 24-bit mono, odd frame count: the data chunk has an odd size and needs padding.
        write_wav(&path, &ramp(101), Some(24), Some(Loop { start: 5, end: 80 })).unwrap();
        assert_eq!(read_smpl_loop(&path), Some(Loop { start: 5, end: 80 }));
        // Still a valid WAV with the same audio.
        assert_eq!(audio::decode(&path, 10.0).unwrap().frames(), 101);
        assert_eq!(hound::WavReader::open(&path).unwrap().len(), 101);
        // The editor picks the loop up.
        let s = Session::open(1, &path, Some(24)).unwrap();
        assert_eq!(s.state().loop_points, Some(Loop { start: 5, end: 80 }));

        write_wav(&path, &ramp(10), Some(16), None).unwrap();
        assert_eq!(read_smpl_loop(&path), None);
    }

    #[test]
    fn zero_crossings_are_found_nearby() {
        // Positive, then negative from frame 6 on.
        let samples = vec![0.5, 0.4, 0.3, 0.2, 0.1, 0.05, -0.2, -0.3, -0.4, -0.5];
        let p = Pcm { samples, channels: 1, sample_rate: 1000 };
        // The sign changes between 5 and 6; 5 is closer to zero.
        assert_eq!(zero_crossing(&p, 2, 10), 5);
        assert_eq!(zero_crossing(&p, 8, 10), 5);
        // Out of reach: unchanged.
        assert_eq!(zero_crossing(&p, 0, 2), 0);
        assert_eq!(zero_crossing(&p, 10, 3), 10);
        // Channels are mixed: opposite channels cancel out.
        let p = Pcm { samples: vec![0.5, -0.5, 0.3, -0.3, 0.2, -0.2], channels: 2, sample_rate: 1000 };
        assert_eq!(zero_crossing(&p, 1, 2), 1);
    }
}
