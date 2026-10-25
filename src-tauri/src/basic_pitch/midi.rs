//! Standard MIDI file format 0. Musical tempo changes the beat grid, not timing.
use super::{check, failure, notes::Note};
use crate::error::Result;
use std::{io::Write, path::Path, sync::atomic::AtomicBool};
fn variable(mut value: u32, bytes: &mut Vec<u8>) {
    let mut buffer = [0u8; 4];
    let mut index = 3;
    buffer[index] = (value & 127) as u8;
    while {
        value >>= 7;
        value != 0
    } {
        index -= 1;
        buffer[index] = (value & 127) as u8 | 128;
    }
    bytes.extend_from_slice(&buffer[index..]);
}
pub fn encode(notes: &[Note], bpm: Option<f64>, name: &str) -> Result<Vec<u8>> {
    let bpm = bpm
        .filter(|v| v.is_finite() && (20.0..=999.0).contains(v))
        .unwrap_or(120.0);
    let tempo = (60_000_000.0 / bpm).round() as u32;
    let ticks_per_second = 480.0 * 1_000_000.0 / tempo as f64;
    let mut events: Vec<(u32, u8, u8, u8)> = Vec::with_capacity(notes.len() * 2);
    for note in notes {
        if !note.start.is_finite()
            || !note.end.is_finite()
            || note.start < 0.0
            || note.end <= note.start
            || note.end > 600.1
            || note.pitch > 127
            || note.velocity == 0
            || note.velocity > 127
        {
            return Err(failure("invalid MIDI note"));
        }
        let start = (note.start * ticks_per_second).round() as u32;
        let end = ((note.end * ticks_per_second).round() as u32).max(start + 1);
        events.push((start, 0x90, note.pitch, note.velocity));
        events.push((end, 0x80, note.pitch, 0));
    }
    // Note-off before note-on at equal ticks avoids stuck repeated notes.
    events.sort_unstable();
    let mut track = vec![
        0,
        0xff,
        0x51,
        3,
        (tempo >> 16) as u8,
        (tempo >> 8) as u8,
        tempo as u8,
    ];
    let title: String = name.chars().take(80).collect();
    track.extend_from_slice(&[0, 0xff, 3]);
    variable(title.len() as u32, &mut track);
    track.extend_from_slice(title.as_bytes());
    track.extend_from_slice(&[0, 0xc0, 4]); // General MIDI Electric Piano 1, like Spotify's exporter.
    let mut previous = 0;
    for (tick, kind, pitch, velocity) in events {
        variable(tick - previous, &mut track);
        track.extend_from_slice(&[kind, pitch, velocity]);
        previous = tick;
    }
    track.extend_from_slice(&[0, 0xff, 0x2f, 0]);
    let mut data = b"MThd\0\0\0\x06\0\0\0\x01\x01\xe0MTrk".to_vec();
    data.extend_from_slice(&(track.len() as u32).to_be_bytes());
    data.extend(track);
    Ok(data)
}
pub fn validate_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mid"))
        || !path.parent().is_some_and(Path::is_dir)
    {
        return Err(failure("choose a .mid file in an existing folder"));
    }
    Ok(())
}
pub fn save(path: &Path, data: &[u8], cancel: &AtomicBool) -> Result<()> {
    validate_path(path)?;
    check(cancel)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(failure)?
        .as_nanos();
    let partial = path
        .parent()
        .unwrap()
        .join(format!(".fonicca-midi-{stamp}.part"));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        check(cancel)?;
        std::fs::rename(&partial, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn midi_has_valid_chunk_lengths_tempo_and_balanced_notes() {
        let notes = [
            Note {
                start: 0.0,
                end: 1.0,
                pitch: 60,
                velocity: 100,
            },
            Note {
                start: 1.0,
                end: 2.0,
                pitch: 60,
                velocity: 90,
            },
        ];
        let bytes = encode(&notes, Some(120.0), "Piano").unwrap();
        assert_eq!(&bytes[..4], b"MThd");
        assert_eq!(&bytes[14..18], b"MTrk");
        assert_eq!(
            u32::from_be_bytes(bytes[18..22].try_into().unwrap()) as usize,
            bytes.len() - 22
        );
        assert!(
            bytes
                .windows(7)
                .any(|b| b == [0x87, 0x40, 0x80, 60, 0, 0, 0x90])
        ); // 960 ticks, off then on.
        assert_eq!(&bytes[bytes.len() - 4..], &[0, 255, 47, 0]);
        let fast = encode(&notes, Some(240.0), "Piano").unwrap();
        assert!(fast.windows(4).any(|b| b == [3, 3, 208, 144])); // 250000 us per quarter.
    }
    #[test]
    fn cancellation_and_bad_destinations_preserve_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.wav");
        std::fs::write(&source, b"original").unwrap();
        assert!(save(&source, b"midi", &AtomicBool::new(false)).is_err());
        let target = dir.path().join("notes.mid");
        std::fs::write(&target, b"old midi").unwrap();
        assert!(save(&target, b"new midi", &AtomicBool::new(true)).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
        assert_eq!(std::fs::read(&target).unwrap(), b"old midi");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
        save(&target, b"new midi", &AtomicBool::new(false)).unwrap();
        assert_eq!(std::fs::read(target).unwrap(), b"new midi");
    }
}
