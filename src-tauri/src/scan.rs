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
    /// Rows added, updated or removed (set on the final event).
    pub changes: usize,
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
    fingerprint: Option<i64>,
}

/// Runs a full (incremental) scan of one source on the current thread.
/// Unchanged files (same size and mtime) are skipped; vanished files are removed.
/// `on_progress` is throttled and always receives a final `done` event.
pub fn scan_source(db_path: &Path, source_id: i64, on_progress: impl Fn(ScanProgress) + Sync) {
    let result = run(db_path, source_id, &on_progress);
    let (changes, error) = match result {
        Ok(n) => (n, None),
        Err(e) => (0, Some(e.to_string())),
    };
    on_progress(ScanProgress { source_id, processed: 0, total: 0, done: true, changes, error });
}

/// Returns the number of rows added, updated or removed.
fn run(db_path: &Path, source_id: i64, on_progress: &(impl Fn(ScanProgress) + Sync)) -> Result<usize> {
    let mut conn = db::open(db_path)?;
    let root: String =
        conn.query_row("SELECT path FROM sources WHERE id = ?1", [source_id], |r| r.get(0))?;

    let mut known: HashMap<String, (i64, i64, i64)> = HashMap::new();
    {
        // Rows without stream info or fingerprint get a mtime of -1 so they are probed
        // again (e.g. after a probe fallback or a new column was added).
        let mut stmt = conn.prepare(
            "SELECT id, path, size,
                    CASE WHEN sample_rate IS NULL OR fingerprint IS NULL THEN -1 ELSE mtime END
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
            on_progress(ScanProgress { source_id, processed: n, total, done: false, changes: 0, error: None });
        }
    };

    // Probing reads only headers, but is I/O-bound per file: parallelize it.
    let scanned: Vec<Scanned> = changed
        .into_par_iter()
        .map(|candidate| {
            let info = audio::probe(&candidate.path).unwrap_or_default();
            let fingerprint = audio::fingerprint(&candidate.path, candidate.size as u64).ok();
            emit(processed.fetch_add(1, Ordering::Relaxed) + 1);
            Scanned { candidate, info, fingerprint }
        })
        .collect();

    // Paths of new files, to tell vanished rows from rows being re-probed.
    let new_paths: HashSet<String> =
        scanned.iter().map(|s| s.candidate.path.to_string_lossy().into_owned()).collect();
    let mut changes = 0;

    let tx = conn.transaction()?;
    {
        // Delete vanished files first: the delete trigger detaches their tags so the
        // insert trigger can reattach them if the same file shows up at a new path.
        let mut delete = tx.prepare("DELETE FROM samples WHERE id = ?1")?;
        for (path, (id, _, _)) in &known {
            if !seen.contains(id) && !new_paths.contains(path) {
                delete.execute([id])?;
                changes += 1;
            }
        }

        let mut upsert = tx.prepare(
            "INSERT INTO samples (source_id, path, name, ext, size, mtime, duration, sample_rate,
                                  channels, bit_depth, fingerprint, peaks)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
             ON CONFLICT(path) DO UPDATE SET
                source_id = excluded.source_id, name = excluded.name, ext = excluded.ext,
                duration = excluded.duration, sample_rate = excluded.sample_rate,
                channels = excluded.channels, bit_depth = excluded.bit_depth,
                fingerprint = excluded.fingerprint,
                peaks = CASE WHEN size = excluded.size AND mtime = excluded.mtime THEN peaks END,
                analysis_version = CASE WHEN size = excluded.size AND mtime = excluded.mtime
                                        THEN analysis_version END,
                embedding_version = CASE WHEN size = excluded.size AND mtime = excluded.mtime
                                         THEN embedding_version END,
                size = excluded.size, mtime = excluded.mtime",
        )?;
        for s in &scanned {
            let c = &s.candidate;
            let name = c.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = c
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            upsert.execute(params![
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
                s.fingerprint,
            ])?;
            changes += 1;
        }
    }
    tx.commit()?;
    Ok(changes)
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

    fn add_source(db_path: &Path, root: &Path) -> i64 {
        db::open(db_path)
            .unwrap()
            .query_row(
                "INSERT INTO sources (path, added_at) VALUES (?1, 0) RETURNING id",
                [root.to_string_lossy()],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn sample_id(db_path: &Path, name: &str) -> Option<i64> {
        use rusqlite::OptionalExtension;
        db::open(db_path)
            .unwrap()
            .query_row("SELECT id FROM samples WHERE name = ?1", [name], |r| r.get(0))
            .optional()
            .unwrap()
    }

    /// (favorite, tag names) of the sample with this name.
    fn user_data(db_path: &Path, name: &str) -> (bool, Vec<String>) {
        let conn = db::open(db_path).unwrap();
        let id = sample_id(db_path, name).expect("sample exists");
        let fav: bool = conn.query_row("SELECT favorite FROM samples WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        let mut stmt = conn
            .prepare("SELECT t.name FROM sample_tags st JOIN tags t ON t.id = st.tag_id WHERE st.sample_id = ?1 ORDER BY t.name")
            .unwrap();
        let tags = stmt.query_map([id], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        (fav, tags)
    }

    fn tag_and_favorite(db_path: &Path, name: &str, tags: &[&str]) {
        let mut conn = db::open(db_path).unwrap();
        let id = sample_id(db_path, name).unwrap();
        for t in tags {
            let tag = crate::library::ensure_tag(&conn, t).unwrap();
            crate::library::tag_samples(&mut conn, &[id], tag.id).unwrap();
        }
        crate::library::set_favorite(&mut conn, &[id], true).unwrap();
    }

    fn detached_count(db_path: &Path) -> i64 {
        db::open(db_path).unwrap().query_row("SELECT COUNT(*) FROM detached", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn renamed_file_keeps_tags_and_favorite() {
        let (_dir, root, db_path, id) = setup();
        write_sine(&root.join("drums/kick.wav"), 44100, 1, 16, 0.2);
        write_sine(&root.join("drums/snare.wav"), 44100, 1, 16, 0.3);
        scan(&db_path, id);
        tag_and_favorite(&db_path, "kick", &["punchy", "808"]);

        std::fs::create_dir_all(root.join("favs")).unwrap();
        std::fs::rename(root.join("drums/kick.wav"), root.join("favs/Big Kick.wav")).unwrap();
        let events = scan(&db_path, id);
        assert_eq!(events.last().unwrap().changes, 2); // one delete + one insert

        assert_eq!(sample_id(&db_path, "kick"), None);
        assert_eq!(user_data(&db_path, "Big Kick"), (true, vec!["808".into(), "punchy".into()]));
        assert_eq!(user_data(&db_path, "snare"), (false, vec![]));
        assert_eq!(detached_count(&db_path), 0);
    }

    #[test]
    fn file_moved_to_another_source_keeps_tags() {
        let (dir, root, db_path, a) = setup();
        let other = dir.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        let b = add_source(&db_path, &other);
        write_sine(&root.join("pad.wav"), 48000, 2, 24, 0.5);
        scan(&db_path, a);
        tag_and_favorite(&db_path, "pad", &["warm"]);

        std::fs::rename(root.join("pad.wav"), other.join("pad.wav")).unwrap();
        scan(&db_path, a);
        assert_eq!(detached_count(&db_path), 1);
        scan(&db_path, b);
        assert_eq!(user_data(&db_path, "pad"), (true, vec!["warm".into()]));
        assert_eq!(detached_count(&db_path), 0);
    }

    #[test]
    fn removing_and_readding_a_source_restores_tags() {
        let (_dir, root, db_path, id) = setup();
        write_sine(&root.join("hat.wav"), 44100, 1, 16, 0.1);
        scan(&db_path, id);
        tag_and_favorite(&db_path, "hat", &["crisp"]);

        db::open(&db_path).unwrap().execute("DELETE FROM sources WHERE id = ?1", [id]).unwrap();
        assert_eq!(detached_count(&db_path), 1);
        let id = add_source(&db_path, &root);
        scan(&db_path, id);
        assert_eq!(user_data(&db_path, "hat"), (true, vec!["crisp".into()]));
    }

    #[test]
    fn edited_file_keeps_tags_and_untagged_deletes_leave_nothing_behind() {
        let (_dir, root, db_path, id) = setup();
        write_sine(&root.join("loop.wav"), 44100, 2, 16, 1.0);
        write_sine(&root.join("plain.wav"), 44100, 2, 16, 1.0);
        scan(&db_path, id);
        tag_and_favorite(&db_path, "loop", &["groove"]);

        // Same path, new content: the row is updated in place.
        write_sine(&root.join("loop.wav"), 44100, 2, 16, 2.0);
        std::fs::remove_file(root.join("plain.wav")).unwrap();
        scan(&db_path, id);
        assert_eq!(user_data(&db_path, "loop"), (true, vec!["groove".into()]));
        assert_eq!(detached_count(&db_path), 0);
    }

    #[test]
    fn detached_data_expires() {
        let (_dir, root, db_path, id) = setup();
        write_sine(&root.join("old.wav"), 44100, 1, 16, 0.1);
        scan(&db_path, id);
        tag_and_favorite(&db_path, "old", &["x"]);
        std::fs::remove_file(root.join("old.wav")).unwrap();
        scan(&db_path, id);
        let conn = db::open(&db_path).unwrap();
        db::prune_detached(&conn).unwrap();
        assert_eq!(detached_count(&db_path), 1);
        conn.execute("UPDATE detached SET detached_at = detached_at - 91 * 86400", []).unwrap();
        db::prune_detached(&conn).unwrap();
        assert_eq!(detached_count(&db_path), 0);
    }

    #[test]
    fn fingerprint_depends_on_content_not_name() {
        let (_dir, root, _db, _id) = setup();
        // Long enough that head and tail chunks are hashed separately.
        write_sine(&root.join("a.wav"), 44100, 2, 16, 3.0);
        std::fs::copy(root.join("a.wav"), root.join("b.wav")).unwrap();
        write_sine(&root.join("c.wav"), 44100, 2, 16, 3.01);
        let fp = |n: &str| {
            let p = root.join(n);
            audio::fingerprint(&p, std::fs::metadata(&p).unwrap().len()).unwrap()
        };
        assert_eq!(fp("a.wav"), fp("b.wav"));
        assert_ne!(fp("a.wav"), fp("c.wav"));

        // A change in the tail only is still detected.
        let mut bytes = std::fs::read(root.join("b.wav")).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        std::fs::write(root.join("b.wav"), bytes).unwrap();
        assert_ne!(fp("a.wav"), fp("b.wav"));
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
