//! Slice markers: each starts a region that runs to the next marker (or the end of
//! the file). Kept in the WAV as cue points, with a "skip" label on regions left out.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::loops::append_chunk;
use super::Op;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Marker {
    pub frame: usize,
    /// The region from here is left out when saving regions.
    #[serde(default)]
    pub skip: bool,
}

/// Sorted by frame, one marker per frame, all inside `[0, frames)`. Of markers on
/// the same frame the last one wins: it starts the region that follows.
pub fn normalized(mut markers: Vec<Marker>, frames: usize) -> Vec<Marker> {
    markers.retain(|m| m.frame < frames);
    markers.sort_by_key(|m| m.frame);
    let mut out: Vec<Marker> = Vec::with_capacity(markers.len());
    for m in markers {
        match out.last_mut() {
            Some(last) if last.frame == m.frame => *last = m,
            _ => out.push(m),
        }
    }
    out
}

/// Where the markers are after `op`, applied to a buffer of `frames` frames of
/// `channels` channels: they move with the audio around them; markers inside cut
/// audio go to the cut.
pub fn moved_markers(markers: &[Marker], op: &Op, frames: usize, channels: usize) -> Vec<Marker> {
    let (start, end) = op.range();
    let (start, end) = (start.min(frames), end.min(frames).max(start.min(frames)));
    let (map, new_frames): (Box<dyn Fn(usize) -> usize>, usize) = match *op {
        Op::SetMarkers { ref markers } => return normalized(markers.clone(), frames),
        Op::Crop { .. } => (Box::new(move |p: usize| p.clamp(start, end) - start), end - start),
        Op::Delete { .. } => (
            Box::new(move |p: usize| if p <= start { p } else if p >= end { p - (end - start) } else { start }),
            frames - (end - start),
        ),
        Op::Stretch { ref samples, .. } => {
            let inserted = samples.len() / channels.max(1);
            (Box::new(move |p| super::processing::retimed_frame(p, start, end, inserted)), frames - (end - start) + inserted)
        }
        Op::Replace { ref samples, .. } => {
            let inserted = samples.len() / channels.max(1);
            (
                Box::new(move |p: usize| if p <= start { p } else if p >= end { p - (end - start) + inserted } else { start }),
                frames - (end - start) + inserted,
            )
        }
        _ => return markers.to_vec(),
    };
    normalized(markers.iter().map(|m| Marker { frame: map(m.frame), ..*m }).collect(), new_frames)
}

/// Appends the markers as a `cue ` chunk, and a `LIST`/`adtl` chunk labelling the
/// skipped ones "skip".
pub fn append_markers(path: &Path, markers: &[Marker]) -> Result<()> {
    if markers.is_empty() {
        return Ok(());
    }
    let mut cue = (markers.len() as u32).to_le_bytes().to_vec();
    for (i, m) in markers.iter().enumerate() {
        let id = i as u32 + 1;
        // id, position, data chunk id, chunk start, block start, sample offset.
        for w in [id, m.frame as u32] {
            cue.extend_from_slice(&w.to_le_bytes());
        }
        cue.extend_from_slice(b"data");
        for w in [0u32, 0, m.frame as u32] {
            cue.extend_from_slice(&w.to_le_bytes());
        }
    }
    append_chunk(path, b"cue ", &cue)?;

    let mut adtl = b"adtl".to_vec();
    for (i, _) in markers.iter().enumerate().filter(|(_, m)| m.skip) {
        let text = b"skip\0";
        adtl.extend_from_slice(b"labl");
        adtl.extend_from_slice(&((4 + text.len()) as u32).to_le_bytes());
        adtl.extend_from_slice(&(i as u32 + 1).to_le_bytes());
        adtl.extend_from_slice(text);
        if text.len() % 2 == 1 {
            adtl.push(0);
        }
    }
    if adtl.len() > 4 {
        append_chunk(path, b"LIST", &adtl)?;
    }
    Ok(())
}

/// Cue points of a WAV as markers; those labelled "skip" are skipped.
pub fn read_markers(path: &Path) -> Vec<Marker> {
    read(path).unwrap_or_default()
}

fn read(path: &Path) -> Option<Vec<Marker>> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut header = [0u8; 12];
    f.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }
    let mut cues: Vec<(u32, usize)> = Vec::new();
    let mut skipped: Vec<u32> = Vec::new();
    let mut chunk = [0u8; 8];
    while f.read_exact(&mut chunk).is_ok() {
        let size = u32::from_le_bytes(chunk[4..8].try_into().ok()?) as usize;
        let id = &chunk[0..4];
        if id != b"cue " && id != b"LIST" {
            f.seek(SeekFrom::Current((size + size % 2) as i64)).ok()?;
            continue;
        }
        let mut data = vec![0u8; size.min(1 << 20)];
        f.read_exact(&mut data).ok()?;
        if size % 2 == 1 {
            f.seek(SeekFrom::Current(1)).ok()?;
        }
        let word = |at: usize| data.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        if id == b"cue " {
            let n = word(0)? as usize;
            cues = (0..n).filter_map(|i| Some((word(4 + i * 24)?, word(4 + i * 24 + 20)? as usize))).collect();
        } else if data.starts_with(b"adtl") {
            let mut at = 4;
            while at + 12 <= data.len() {
                let len = word(at + 4)? as usize;
                if &data[at..at + 4] == b"labl" && data.get(at + 12..at + 8 + len)?.starts_with(b"skip") {
                    skipped.push(word(at + 8)?);
                }
                at += 8 + len + len % 2;
            }
        }
    }
    Some(cues.into_iter().map(|(id, frame)| Marker { frame, skip: skipped.contains(&id) }).collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::{write_wav, Loop, Session};
    use super::*;
    use crate::audio::{self, Pcm};

    fn m(frame: usize) -> Marker {
        Marker { frame, skip: false }
    }

    #[test]
    fn markers_follow_edits() {
        let marks = [m(10), m(50), m(90)];
        // Deleting 40..60 pulls the later marker in; the one inside goes to the cut.
        assert_eq!(moved_markers(&marks, &Op::Delete { start: 40, end: 60 }, 100, 1), [m(10), m(40), m(70)]);
        // Cropping to 20..95 drops what falls outside and shifts the rest.
        assert_eq!(moved_markers(&marks, &Op::Crop { start: 20, end: 95 }, 100, 1), [m(0), m(30), m(70)]);
        // Pasting 10 frames at 30 moves the markers after it.
        let paste = Op::Replace { start: 30, end: 30, samples: Arc::new(vec![0.0; 10]) };
        assert_eq!(moved_markers(&marks, &paste, 100, 1), [m(10), m(60), m(100)]);
        // Markers that land on one frame merge; the later one wins.
        let two = [m(40), Marker { frame: 55, skip: true }];
        assert_eq!(moved_markers(&two, &Op::Delete { start: 40, end: 60 }, 100, 1), [Marker { frame: 40, skip: true }]);
    }

    #[test]
    fn markers_are_edits_and_survive_a_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        let pcm = Pcm { samples: vec![0.1; 1000], channels: 1, sample_rate: 1000 };
        write_wav(&path, &pcm, Some(16), None).unwrap();
        let mut s = Session::open(1, &path, Some(16)).unwrap();
        assert!(s.markers().is_empty());

        // Out of order, a duplicate and one past the end: cleaned up.
        let set = vec![m(500), Marker { frame: 0, skip: true }, m(250), m(250), m(5000)];
        s.apply(Op::SetMarkers { markers: set });
        assert_eq!(s.markers(), [Marker { frame: 0, skip: true }, m(250), m(500)]);
        assert!(s.state().dirty);
        s.apply(Op::Delete { start: 0, end: 100 });
        assert_eq!(s.markers(), [Marker { frame: 0, skip: true }, m(150), m(400)]);
        s.undo();
        s.undo();
        assert!(s.markers().is_empty());
        s.redo();

        // Written next to a loop, read back with the skip label.
        write_wav(&path, &pcm, Some(16), Some(Loop { start: 100, end: 900 })).unwrap();
        append_markers(&path, s.markers()).unwrap();
        assert_eq!(read_markers(&path), s.markers());
        assert_eq!(audio::decode(&path, 10.0).unwrap().frames(), 1000);
        let reopened = Session::open(1, &path, Some(16)).unwrap();
        assert_eq!(reopened.markers(), s.markers());
        assert_eq!(reopened.loop_points(), Some(Loop { start: 100, end: 900 }));
    }
}
