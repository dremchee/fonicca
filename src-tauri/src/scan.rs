use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use rayon::prelude::*;
use rusqlite::params;
use serde::Serialize;
use walkdir::WalkDir;

use crate::audio::{self, AudioInfo};
use crate::db;
use crate::error::Result;

pub const PROGRESS_EVENT: &str = "scan-progress";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub source_id: i64,
    pub processed: usize,
    pub total: usize,
    pub done: bool,
    pub error: Option<String>,
}

struct Candidate {
    path: PathBuf,
    size: i64,
    mtime: i64,
}

struct Scanned {
    candidate: Candidate,
    info: AudioInfo,
}

/// Runs a full (incremental) scan of one source on the current thread.
/// Unchanged files (same size and mtime) are skipped; vanished files are removed.
/// `on_progress` is throttled and always receives a final `done` event.
pub fn scan_source(db_path: &Path, source_id: i64, on_progress: impl Fn(ScanProgress) + Sync) {
    let result = run(db_path, source_id, &on_progress);
    on_progress(ScanProgress {
        source_id,
        processed: 0,
        total: 0,
        done: true,
        error: result.err().map(|e| e.to_string()),
    });
}

fn run(db_path: &Path, source_id: i64, on_progress: &(impl Fn(ScanProgress) + Sync)) -> Result<()> {
    let mut conn = db::open(db_path)?;
    let root: String =
        conn.query_row("SELECT path FROM sources WHERE id = ?1", [source_id], |r| r.get(0))?;

    let mut known: HashMap<String, (i64, i64, i64)> = HashMap::new();
    {
        // Rows without stream info get a mtime of -1 so they are probed again
        // (e.g. after a probe fallback is added for their codec).
        let mut stmt = conn.prepare(
            "SELECT id, path, size, CASE WHEN sample_rate IS NULL THEN -1 ELSE mtime END
             FROM samples WHERE source_id = ?1",
        )?;
        let rows = stmt.query_map([source_id], |r| {
            Ok((r.get::<_, String>(1)?, (r.get(0)?, r.get(2)?, r.get(3)?)))
        })?;
        for row in rows {
            let (path, v) = row?;
            known.insert(path, v);
        }
    }

    let mut seen: HashSet<i64> = HashSet::new();
    let mut changed: Vec<Candidate> = Vec::new();
    for entry in WalkDir::new(&root).follow_links(false).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() || !audio::is_audio_file(entry.path()) {
            continue;
        }
        // Skip macOS resource-fork companions like "._kick.wav".
        if entry.file_name().to_string_lossy().starts_with("._") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let size = meta.len() as i64;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        let path = entry.path().to_string_lossy().into_owned();
        match known.get(&path) {
            Some(&(id, s, m)) if s == size && m == mtime => {
                seen.insert(id);
            }
            _ => changed.push(Candidate { path: entry.into_path(), size, mtime }),
        }
    }

    let total = changed.len();
    let processed = AtomicUsize::new(0);
    let last_emit = std::sync::Mutex::new(Instant::now());
    let emit = |n: usize| {
        let mut last = last_emit.lock().unwrap();
        if last.elapsed() >= Duration::from_millis(100) {
            *last = Instant::now();
            on_progress(ScanProgress { source_id, processed: n, total, done: false, error: None });
        }
    };

    // Probing reads only headers, but is I/O-bound per file: parallelize it.
    let scanned: Vec<Scanned> = changed
        .into_par_iter()
        .map(|candidate| {
            let info = audio::probe(&candidate.path).unwrap_or_default();
            emit(processed.fetch_add(1, Ordering::Relaxed) + 1);
            Scanned { candidate, info }
        })
        .collect();

    let tx = conn.transaction()?;
    {
        let mut upsert = tx.prepare(
            "INSERT INTO samples (source_id, path, name, ext, size, mtime, duration, sample_rate, channels, bit_depth, peaks)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL)
             ON CONFLICT(path) DO UPDATE SET
                source_id = excluded.source_id, name = excluded.name, ext = excluded.ext,
                size = excluded.size, mtime = excluded.mtime, duration = excluded.duration,
                sample_rate = excluded.sample_rate, channels = excluded.channels,
                bit_depth = excluded.bit_depth, peaks = NULL
             RETURNING id",
        )?;
        for s in &scanned {
            let c = &s.candidate;
            let name = c.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = c
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let id: i64 = upsert.query_row(
                params![
                    source_id,
                    c.path.to_string_lossy(),
                    name,
                    ext,
                    c.size,
                    c.mtime,
                    s.info.duration,
                    s.info.sample_rate,
                    s.info.channels,
                    s.info.bit_depth,
                ],
                |r| r.get(0),
            )?;
            seen.insert(id);
        }

        let mut delete = tx.prepare("DELETE FROM samples WHERE id = ?1")?;
        for (id, _, _) in known.values() {
            if !seen.contains(id) {
                delete.execute([id])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn write_sine(path: &Path, sample_rate: u32, channels: u16, bits: u16, seconds: f32) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: bits,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        let amp = ((1i64 << (bits - 1)) - 1) as f32 * 0.5;
        for i in 0..(sample_rate as f32 * seconds) as u32 {
            let v = (i as f32 / sample_rate as f32 * 440.0 * std::f32::consts::TAU).sin() * amp;
            for _ in 0..channels {
                w.write_sample(v as i32).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, i64) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(root.join("drums")).unwrap();
        let db_path = dir.path().join("test.db");
        let mut conn = db::open(&db_path).unwrap();
        db::migrate(&mut conn).unwrap();
        let id: i64 = conn
            .query_row(
                "INSERT INTO sources (path, added_at) VALUES (?1, 0) RETURNING id",
                [root.to_string_lossy()],
                |r| r.get(0),
            )
            .unwrap();
        (dir, root, db_path, id)
    }

    fn scan(db_path: &Path, id: i64) -> Vec<ScanProgress> {
        let events = Mutex::new(Vec::new());
        scan_source(db_path, id, |p| events.lock().unwrap().push(p));
        events.into_inner().unwrap()
    }

    fn rows(db_path: &Path) -> Vec<(String, Option<f64>, Option<i64>, Option<i64>, Option<i64>)> {
        let conn = db::open(db_path).unwrap();
        let mut stmt = conn
            .prepare("SELECT name, duration, sample_rate, channels, bit_depth FROM samples ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn indexes_audio_with_metadata_and_skips_other_files() {
        let (_dir, root, db_path, id) = setup();
        write_sine(&root.join("drums/kick.wav"), 44100, 1, 16, 0.5);
        write_sine(&root.join("pad.WAV"), 48000, 2, 24, 2.0);
        std::fs::write(root.join("notes.txt"), "not audio").unwrap();
        std::fs::write(root.join("._kick.wav"), "resource fork").unwrap();

        let events = scan(&db_path, id);
        let last = events.last().unwrap();
        assert!(last.done && last.error.is_none(), "{:?}", last.error);

        let r = rows(&db_path);
        assert_eq!(r.len(), 2);
        let (name, dur, sr, ch, bits) = &r[0];
        assert_eq!(name, "kick");
        assert!((dur.unwrap() - 0.5).abs() < 0.01);
        assert_eq!((*sr, *ch, *bits), (Some(44100), Some(1), Some(16)));
        let (name, dur, sr, ch, bits) = &r[1];
        assert_eq!(name, "pad");
        assert!((dur.unwrap() - 2.0).abs() < 0.01);
        assert_eq!((*sr, *ch, *bits), (Some(48000), Some(2), Some(24)));
    }

    #[test]
    fn rescan_keeps_unchanged_and_drops_deleted() {
        let (_dir, root, db_path, id) = setup();
        write_sine(&root.join("a.wav"), 44100, 1, 16, 0.1);
        write_sine(&root.join("b.wav"), 44100, 1, 16, 0.1);
        scan(&db_path, id);
        let conn = db::open(&db_path).unwrap();
        let id_a: i64 = conn.query_row("SELECT id FROM samples WHERE name = 'a'", [], |r| r.get(0)).unwrap();
        conn.execute("UPDATE samples SET peaks = x'01' WHERE id = ?1", [id_a]).unwrap();

        std::fs::remove_file(root.join("b.wav")).unwrap();
        scan(&db_path, id);

        let names: Vec<String> = rows(&db_path).into_iter().map(|r| r.0).collect();
        assert_eq!(names, vec!["a"]);
        // Unchanged file keeps its row (and cached peaks).
        let (same_id, peaks): (i64, Option<Vec<u8>>) = conn
            .query_row("SELECT id, peaks FROM samples WHERE name = 'a'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(same_id, id_a);
        assert_eq!(peaks, Some(vec![1]));
    }

    #[test]
    fn missing_source_reports_error() {
        let (_dir, _root, db_path, _id) = setup();
        let events = scan(&db_path, 999);
        assert!(events.last().unwrap().error.is_some());
    }

    #[test]
    fn peaks_follow_the_signal() {
        let (_dir, root, _db_path, _id) = setup();
        let path = root.join("sine.wav");
        write_sine(&path, 44100, 2, 16, 1.0);
        let peaks = audio::compute_peaks(&path).unwrap();
        assert_eq!(peaks.len(), audio::PEAK_BUCKETS * 2);
        // Sine at half amplitude: every bucket spans roughly -64..64.
        for pair in peaks.chunks(2) {
            assert!(pair[0] < -55 && pair[1] > 55, "{pair:?}");
        }
    }

    #[test]
    fn falls_back_to_riff_header_for_unsupported_codecs() {
        let (_dir, root, _db, _id) = setup();
        // IMA ADPCM-style header with an unknown codec tag: 2 ch, 44.1 kHz, fact = 44100 frames.
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt ");
        b.extend_from_slice(&20u32.to_le_bytes());
        b.extend_from_slice(&0x674fu16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&44100u32.to_le_bytes());
        b.extend_from_slice(&9000u32.to_le_bytes());
        b.extend_from_slice(&2048u16.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&[0u8; 4]);
        b.extend_from_slice(b"fact");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&44100u32.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&[0u8; 16]);
        let path = root.join("odd.wav");
        std::fs::write(&path, b).unwrap();

        let info = audio::probe(&path).unwrap();
        assert_eq!(info.sample_rate, Some(44100));
        assert_eq!(info.channels, Some(2));
        assert!((info.duration.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn probes_system_aiff() {
        let p = Path::new("/System/Library/Sounds/Ping.aiff");
        if !p.exists() {
            return;
        }
        let info = audio::probe(p).unwrap();
        assert!(info.duration.unwrap() > 0.0);
        assert!(info.sample_rate.is_some());
        assert_eq!(audio::compute_peaks(p).unwrap().len(), audio::PEAK_BUCKETS * 2);
    }

    /// `cargo test --release -- --ignored --nocapture scan_speed`
    #[test]
    #[ignore]
    fn scan_speed() {
        let (_dir, root, db_path, id) = setup();
        let n = 20_000;
        let template = root.join("template.wav");
        write_sine(&template, 44100, 2, 16, 0.3);
        for i in 0..n {
            let sub = root.join(format!("pack{}", i / 500));
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::copy(&template, sub.join(format!("hit_{i}.wav"))).unwrap();
        }
        let t = Instant::now();
        scan(&db_path, id);
        println!("first scan of {} files: {:?}", n + 1, t.elapsed());
        let t = Instant::now();
        scan(&db_path, id);
        println!("rescan (unchanged): {:?}", t.elapsed());
        assert_eq!(rows(&db_path).len(), n + 1);
    }
}
