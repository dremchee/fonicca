//! Background analysis queue: analyzes samples that have no (current) analysis,
//! a small batch at a time, with requested samples first.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use rayon::prelude::*;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::analysis::{self, Analysis, ANALYSIS_VERSION};
use crate::db;
use crate::error::Result;

pub const PROGRESS_EVENT: &str = "analysis-progress";

/// Small batches keep requested samples responsive and progress fresh.
const BATCH: usize = 16;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Samples still waiting for analysis.
    pub pending: i64,
    pub paused: bool,
    /// Ids analyzed in the last batch.
    pub updated: Vec<i64>,
}

#[derive(Default)]
struct State {
    paused: bool,
    priority: VecDeque<i64>,
    wake: bool,
}

pub struct Analyzer {
    db_path: PathBuf,
    on_progress: Box<dyn Fn(Progress) + Send + Sync>,
    state: Mutex<State>,
    signal: Condvar,
    pool: rayon::ThreadPool,
}

impl Analyzer {
    pub fn start(db_path: PathBuf, on_progress: impl Fn(Progress) + Send + Sync + 'static) -> Arc<Self> {
        // Leave half the cores for the UI, playback and scanning.
        let threads = (std::thread::available_parallelism().map_or(2, |n| n.get()) / 2).max(1);
        let this = Arc::new(Self {
            db_path,
            on_progress: Box::new(on_progress),
            state: Mutex::new(State { wake: true, ..Default::default() }),
            signal: Condvar::new(),
            pool: rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|i| format!("analysis-{i}"))
                .build()
                .expect("analysis thread pool"),
        });
        let worker = Arc::clone(&this);
        std::thread::Builder::new()
            .name("analyzer".into())
            .spawn(move || worker.run())
            .expect("spawn analyzer");
        this
    }

    /// New or changed samples may be waiting.
    pub fn wake(&self) {
        self.state.lock().unwrap().wake = true;
        self.signal.notify_all();
    }

    /// Analyze these samples next (e.g. the one being previewed).
    pub fn prioritize(&self, ids: &[i64]) {
        let mut state = self.state.lock().unwrap();
        for &id in ids.iter().rev() {
            state.priority.retain(|&x| x != id);
            state.priority.push_front(id);
        }
        state.wake = true;
        self.signal.notify_all();
    }

    pub fn set_paused(&self, paused: bool) {
        self.state.lock().unwrap().paused = paused;
        self.signal.notify_all();
        if let Ok(conn) = db::open(&self.db_path) {
            self.report(&conn, Vec::new());
        }
    }

    pub fn is_paused(&self) -> bool {
        self.state.lock().unwrap().paused
    }

    fn report(&self, conn: &Connection, updated: Vec<i64>) {
        let pending = pending_count(conn).unwrap_or(0);
        (self.on_progress)(Progress { pending, paused: self.is_paused(), updated });
    }

    fn run(&self) {
        let conn = match db::open(&self.db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("analyzer: cannot open database: {e}");
                return;
            }
        };
        let mut idle_reported = false;
        loop {
            let priority: Vec<i64> = {
                let mut state = self.state.lock().unwrap();
                while state.paused || (!state.wake && state.priority.is_empty()) {
                    // Periodic recheck in case a wake-up was missed.
                    let (s, _) = self.signal.wait_timeout(state, Duration::from_secs(30)).unwrap();
                    state = s;
                    if !state.paused {
                        state.wake = true;
                    }
                }
                state.wake = false;
                let n = state.priority.len().min(BATCH);
                state.priority.drain(..n).collect()
            };

            let batch = match next_batch(&conn, &priority) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("analyzer: {e}");
                    continue;
                }
            };
            if batch.is_empty() {
                if !idle_reported {
                    self.report(&conn, Vec::new());
                    idle_reported = true;
                }
                continue;
            }
            idle_reported = false;

            let results: Vec<(i64, Option<Analysis>)> = self.pool.install(|| {
                batch
                    .par_iter()
                    .map(|(id, path, name)| (*id, analysis::analyze(Path::new(path), name).ok()))
                    .collect()
            });
            if let Err(e) = store(&conn, &results) {
                eprintln!("analyzer: cannot store results: {e}");
            }
            self.report(&conn, results.iter().map(|(id, _)| *id).collect());
            // More may be waiting; keep going until a batch comes back empty.
            self.state.lock().unwrap().wake = true;
        }
    }
}

fn pending_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM samples WHERE analysis_version IS NULL OR analysis_version < ?1",
        [ANALYSIS_VERSION],
        |r| r.get(0),
    )?)
}

/// Requested samples that still need analysis, topped up with other pending ones.
fn next_batch(conn: &Connection, priority: &[i64]) -> Result<Vec<(i64, String, String)>> {
    let mut batch = Vec::new();
    let mut by_id = conn.prepare_cached(
        "SELECT id, path, name FROM samples
         WHERE id = ?1 AND (analysis_version IS NULL OR analysis_version < ?2)",
    )?;
    for id in priority {
        if let Some(row) = by_id
            .query_map(params![id, ANALYSIS_VERSION], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .next()
        {
            batch.push(row?);
        }
    }
    if batch.len() < BATCH {
        let mut rest = conn.prepare_cached(
            "SELECT id, path, name FROM samples
             WHERE analysis_version IS NULL OR analysis_version < ?1
             ORDER BY id LIMIT ?2",
        )?;
        let rows = rest.query_map(params![ANALYSIS_VERSION, BATCH as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        for row in rows {
            let row: (i64, String, String) = row?;
            if !batch.iter().any(|b| b.0 == row.0) && batch.len() < BATCH {
                batch.push(row);
            }
        }
    }
    Ok(batch)
}

/// Failed analyses are stored with empty fields so they are not retried until
/// the file changes or the analysis version is bumped.
fn store(conn: &Connection, results: &[(i64, Option<Analysis>)]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut update = tx.prepare_cached(
            "UPDATE samples SET kind = ?1, bpm = ?2, bpm_source = ?3, musical_key = ?4, key_source = ?5,
                                peak_db = ?6, lufs = ?7, analysis_version = ?8
             WHERE id = ?9",
        )?;
        for (id, a) in results {
            let a = a.as_ref();
            update.execute(params![
                a.map(|a| a.kind.as_str()),
                a.and_then(|a| a.bpm.as_ref().map(|b| b.0)),
                a.and_then(|a| a.bpm.as_ref().map(|b| b.1.as_str())),
                a.and_then(|a| a.key.as_ref().map(|k| k.0.clone())),
                a.and_then(|a| a.key.as_ref().map(|k| k.1.as_str())),
                a.and_then(|a| a.peak_db),
                a.and_then(|a| a.lufs),
                ANALYSIS_VERSION,
                id,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn analyzes_pending_samples_and_prioritizes_requests() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
        let db_path = dir.path().join("test.db");
        let mut conn = db::open(&db_path).unwrap();
        db::migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO sources (id, path, added_at) VALUES (1, ?1, 0)", [root.to_string_lossy()]).unwrap();

        let spec = hound::WavSpec { channels: 1, sample_rate: 22050, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        for (i, name) in ["Kick", "Groove 90bpm", "Broken"].iter().enumerate() {
            let path = root.join(format!("{name}.wav"));
            if *name == "Broken" {
                std::fs::write(&path, b"not audio").unwrap();
            } else {
                let mut w = hound::WavWriter::create(&path, spec).unwrap();
                for n in 0..11025 {
                    let t = n as f32 / 22050.0;
                    w.write_sample(((t * 440.0 * 6.283).sin() * (-t * 8.0).exp() * 16000.0) as i16).unwrap();
                }
                w.finalize().unwrap();
            }
            conn.execute(
                "INSERT INTO samples (id, source_id, path, name, ext, size, mtime) VALUES (?1, 1, ?2, ?3, 'wav', 0, 0)",
                params![i as i64 + 1, path.to_string_lossy(), name],
            )
            .unwrap();
        }

        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let analyzer = Analyzer::start(db_path.clone(), move |p| {
            let _ = tx.lock().unwrap().send(p);
        });
        analyzer.prioritize(&[3]);
        loop {
            let p = rx.recv_timeout(Duration::from_secs(20)).expect("progress");
            if p.pending == 0 {
                break;
            }
        }

        let row = |id: i64| -> (Option<String>, Option<f64>, Option<String>, Option<i64>) {
            conn.query_row(
                "SELECT kind, bpm, bpm_source, analysis_version FROM samples WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap()
        };
        assert_eq!(row(1), (Some("oneshot".into()), None, None, Some(ANALYSIS_VERSION)));
        assert_eq!(row(2), (Some("loop".into()), Some(90.0), Some("name".into()), Some(ANALYSIS_VERSION)));
        // Unreadable files are marked as analyzed so they are not retried forever.
        assert_eq!(row(3), (None, None, None, Some(ANALYSIS_VERSION)));
    }
}
