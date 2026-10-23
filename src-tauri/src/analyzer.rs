//! Background analysis queue. Stages, each run in small batches with requested
//! samples first:
//! 1. categories from file and folder names (no decoding, very fast);
//! 2. signal analysis: loudness, one-shot/loop, BPM;
//! 3. with the sound model installed: CLAP embeddings, categories for unnamed
//!    samples from their labelled neighbours, and keys of tonal loops.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use rayon::prelude::*;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::analysis::{self, Analysis, ANALYSIS_VERSION};
use crate::category::{self, Classifier, NAME_RULES_VERSION};
use crate::clap::{self, EMBEDDING_VERSION};
use crate::db;
use crate::error::Result;

pub const PROGRESS_EVENT: &str = "analysis-progress";

/// Small batches keep requested samples responsive and progress fresh.
const BATCH: usize = 16;
const NAME_BATCH: i64 = 2000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Samples still waiting for signal analysis.
    pub pending: i64,
    /// Samples still waiting for an embedding (0 without the sound model).
    pub pending_embeddings: i64,
    pub model_ready: bool,
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
    threads: usize,
    model: Mutex<Option<Arc<clap::Model>>>,
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
            threads,
            model: Mutex::new(None),
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

    /// Loads the CLAP model and starts computing embeddings.
    pub fn load_model(&self, path: &Path) -> Result<()> {
        let model = clap::Model::load(path, self.threads)?;
        *self.model.lock().unwrap() = Some(model);
        self.wake();
        Ok(())
    }

    /// Computes and stores one sample's embedding right away (e.g. "find similar"
    /// on a sample the background queue has not reached). Returns false without
    /// the sound model or for unreadable audio.
    pub fn embed_now(&self, conn: &Connection, id: i64) -> Result<bool> {
        let Some(model) = self.model.lock().unwrap().clone() else { return Ok(false) };
        let path: String = conn.query_row("SELECT path FROM samples WHERE id = ?1", [id], |r| r.get(0))?;
        let Ok(pcm) = crate::audio::decode(Path::new(&path), 10.5) else { return Ok(false) };
        let features = clap::log_mel(&clap::prepare(&pcm)?);
        let Some(embedding) = model.embed(&[features])?.pop() else { return Ok(false) };
        conn.execute(
            "UPDATE samples SET embedding = ?1, embedding_version = ?2 WHERE id = ?3",
            params![clap::to_bytes(&embedding), EMBEDDING_VERSION, id],
        )?;
        Ok(true)
    }

    pub fn model_ready(&self) -> bool {
        self.model.lock().unwrap().is_some()
    }

    pub fn progress(&self, conn: &Connection, updated: Vec<i64>) -> Progress {
        let model_ready = self.model_ready();
        Progress {
            pending: pending_count(conn).unwrap_or(0),
            pending_embeddings: if model_ready { pending_embeddings(conn).unwrap_or(0) } else { 0 },
            model_ready,
            paused: self.is_paused(),
            updated,
        }
    }

    fn report(&self, conn: &Connection, updated: Vec<i64>) {
        (self.on_progress)(self.progress(conn, updated));
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
        let mut classifier: Option<Classifier> = None;
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

            match self.step(&conn, &priority, &mut classifier) {
                Ok(updated) if !updated.is_empty() => {
                    idle_reported = false;
                    self.report(&conn, updated);
                    // More may be waiting; keep going until every stage is empty.
                    self.state.lock().unwrap().wake = true;
                }
                Ok(_) => {
                    if !idle_reported {
                        self.report(&conn, Vec::new());
                        idle_reported = true;
                    }
                }
                Err(e) => eprintln!("analyzer: {e}"),
            }
        }
    }

    /// Runs one batch of the first stage that has work. Returns the ids updated.
    fn step(&self, conn: &Connection, priority: &[i64], classifier: &mut Option<Classifier>) -> Result<Vec<i64>> {
        let named = categorize_by_name(conn)?;
        if !named.is_empty() {
            return Ok(named);
        }

        let batch = next_batch(conn, priority)?;
        if !batch.is_empty() {
            let results: Vec<(i64, Option<Analysis>)> = self.pool.install(|| {
                batch
                    .par_iter()
                    .map(|(id, path, name)| (*id, analysis::analyze(Path::new(path), name).ok()))
                    .collect()
            });
            store(conn, &results)?;
            return Ok(results.iter().map(|(id, _)| *id).collect());
        }

        let Some(model) = self.model.lock().unwrap().clone() else { return Ok(Vec::new()) };
        let batch = next_embedding_batch(conn, priority)?;
        if batch.is_empty() {
            return Ok(Vec::new());
        }
        // Neighbours change as more labelled samples get embeddings; reload when stale.
        let labelled = labelled_count(conn)?;
        if classifier.as_ref().is_none_or(|c| c.labelled_count() * 21 / 20 < labelled as usize) {
            *classifier = Some(Classifier::new(labelled_embeddings(conn)?));
        }
        let decoded: Vec<(EmbeddingJob, Option<(Vec<f32>, Option<String>)>)> = self.pool.install(|| {
            batch
                .into_par_iter()
                .map(|job| {
                    let prepared = prepare_embedding(&job);
                    (job, prepared)
                })
                .collect()
        });
        let features: Vec<Vec<f32>> = decoded.iter().filter_map(|(_, p)| p.as_ref().map(|(f, _)| f.clone())).collect();
        // If the batch fails, retry one by one so a single bad input cannot block
        // the queue forever; inputs that still fail get no embedding.
        let embeddings: Vec<Option<Vec<f32>>> = match model.embed(&features) {
            Ok(e) => e.into_iter().map(Some).collect(),
            Err(e) => {
                eprintln!("analyzer: {e}; retrying samples one by one");
                features.iter().map(|f| model.embed(std::slice::from_ref(f)).ok().and_then(|mut v| v.pop())).collect()
            }
        };
        let mut embeddings = embeddings.into_iter();
        let results: Vec<EmbeddingResult> = decoded
            .into_iter()
            .map(|(job, prepared)| {
                let (embedding, key) = match prepared {
                    Some((_, key)) => (embeddings.next().flatten(), key),
                    None => (None, None),
                };
                let category = match (&job.category_source, &embedding) {
                    (Some(s), _) if s == "name" => None,
                    (_, Some(e)) => classifier.as_ref().and_then(|c| c.classify(e)),
                    _ => None,
                };
                EmbeddingResult { id: job.id, embedding, category, key }
            })
            .collect();
        store_embeddings(conn, &results)?;
        Ok(results.iter().map(|r| r.id).collect())
    }
}

struct EmbeddingJob {
    id: i64,
    path: String,
    kind: Option<String>,
    category: Option<String>,
    category_source: Option<String>,
    has_key: bool,
}

struct EmbeddingResult {
    id: i64,
    embedding: Option<Vec<f32>>,
    category: Option<(&'static str, f32)>,
    key: Option<String>,
}

/// Decodes a sample once for both the CLAP features and, for tonal loops whose
/// category is already known from the name, key detection.
fn prepare_embedding(job: &EmbeddingJob) -> Option<(Vec<f32>, Option<String>)> {
    let pcm = crate::audio::decode(Path::new(&job.path), 30.0).ok()?;
    let features = clap::log_mel(&clap::prepare(&pcm).ok()?);
    let tonal_loop = job.kind.as_deref().is_some_and(|k| k != "oneshot")
        && job.category.as_deref().is_some_and(category::is_tonal);
    let key = (tonal_loop && !job.has_key).then(|| analysis::key_from_audio(&pcm.mono(), pcm.sample_rate)).flatten();
    Some((features, key))
}

fn pending_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM samples WHERE analysis_version IS NULL OR analysis_version < ?1",
        [ANALYSIS_VERSION],
        |r| r.get(0),
    )?)
}

fn pending_embeddings(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM samples
         WHERE analysis_version IS NOT NULL AND (embedding_version IS NULL OR embedding_version < ?1)",
        [EMBEDDING_VERSION],
        |r| r.get(0),
    )?)
}

/// Applies the file/folder name rules to samples not categorized by the
/// current rules. Audio-based categories are kept unless a name now matches.
fn categorize_by_name(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare_cached(
        "SELECT s.id, s.name, substr(s.path, length(src.path) + 2), s.category_source
         FROM samples s JOIN sources src ON src.id = s.source_id
         WHERE s.category_version IS NULL OR s.category_version < ?1
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![NAME_RULES_VERSION, NAME_BATCH], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let tx = conn.unchecked_transaction()?;
    {
        let mut named = tx.prepare_cached(
            "UPDATE samples SET category = ?1, category_source = 'name', category_confidence = 1, category_version = ?2
             WHERE id = ?3",
        )?;
        let mut unnamed = tx.prepare_cached(
            "UPDATE samples SET category_version = ?1,
                    category = CASE WHEN category_source = 'name' THEN NULL ELSE category END,
                    category_confidence = CASE WHEN category_source = 'name' THEN NULL ELSE category_confidence END,
                    category_source = CASE WHEN category_source = 'name' THEN NULL ELSE category_source END
             WHERE id = ?2",
        )?;
        for (id, name, rel, _) in &rows {
            let parts: Vec<&str> = rel.split('/').collect();
            let folders = &parts[..parts.len().saturating_sub(1)];
            match category::from_names(name, folders) {
                Some(c) => named.execute(params![c, NAME_RULES_VERSION, id])?,
                None => unnamed.execute(params![NAME_RULES_VERSION, id])?,
            };
        }
    }
    tx.commit()?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Samples needing an embedding: requested ones first, then name-labelled ones
/// (they become the neighbours that classify the rest), then the others.
fn next_embedding_batch(conn: &Connection, priority: &[i64]) -> Result<Vec<EmbeddingJob>> {
    const COLUMNS: &str = "id, path, kind, category, category_source, musical_key IS NOT NULL";
    const PENDING: &str = "analysis_version IS NOT NULL AND (embedding_version IS NULL OR embedding_version < ?1)";
    let map = |r: &rusqlite::Row| -> rusqlite::Result<EmbeddingJob> {
        Ok(EmbeddingJob {
            id: r.get(0)?,
            path: r.get(1)?,
            kind: r.get(2)?,
            category: r.get(3)?,
            category_source: r.get(4)?,
            has_key: r.get(5)?,
        })
    };
    let mut batch = Vec::new();
    let mut by_id = conn.prepare_cached(&format!("SELECT {COLUMNS} FROM samples WHERE id = ?2 AND {PENDING}"))?;
    for id in priority {
        if let Some(job) = by_id.query_map(params![EMBEDDING_VERSION, id], map)?.next() {
            batch.push(job?);
        }
    }
    if batch.len() < BATCH {
        let mut rest = conn.prepare_cached(&format!(
            "SELECT {COLUMNS} FROM samples WHERE {PENDING}
             ORDER BY category_source = 'name' DESC, id LIMIT ?2"
        ))?;
        for job in rest.query_map(params![EMBEDDING_VERSION, BATCH as i64], map)? {
            let job = job?;
            if batch.len() < BATCH && !batch.iter().any(|b| b.id == job.id) {
                batch.push(job);
            }
        }
    }
    Ok(batch)
}

fn labelled_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM samples WHERE category_source = 'name' AND embedding IS NOT NULL",
        [],
        |r| r.get(0),
    )?)
}

fn labelled_embeddings(conn: &Connection) -> Result<Vec<(String, Vec<f32>)>> {
    let mut stmt =
        conn.prepare("SELECT category, embedding FROM samples WHERE category_source = 'name' AND embedding IS NOT NULL")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, clap::from_bytes(&r.get::<_, Vec<u8>>(1)?))))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Unreadable files get an empty embedding with the current version, so they
/// are not retried until they change.
fn store_embeddings(conn: &Connection, results: &[EmbeddingResult]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut embedding =
            tx.prepare_cached("UPDATE samples SET embedding = ?1, embedding_version = ?2 WHERE id = ?3")?;
        let mut category = tx.prepare_cached(
            "UPDATE samples SET category = ?1, category_source = 'audio', category_confidence = ?2
             WHERE id = ?3 AND (category_source IS NULL OR category_source = 'audio')",
        )?;
        let mut key = tx.prepare_cached(
            "UPDATE samples SET musical_key = ?1, key_source = 'audio' WHERE id = ?2 AND musical_key IS NULL",
        )?;
        for r in results {
            embedding.execute(params![r.embedding.as_deref().map(clap::to_bytes), EMBEDDING_VERSION, r.id])?;
            if let Some((c, confidence)) = r.category {
                category.execute(params![c, confidence, r.id])?;
            }
            if let Some(k) = &r.key {
                key.execute(params![k, r.id])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
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
            if p.pending == 0 && p.updated.is_empty() {
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

        // Name stage: "Kick" and "Groove" match name rules, "Broken" doesn't.
        let category = |id: i64| -> (Option<String>, Option<String>, Option<i64>) {
            conn.query_row(
                "SELECT category, category_source, category_version FROM samples WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
        };
        assert_eq!(category(1), (Some("kick".into()), Some("name".into()), Some(NAME_RULES_VERSION)));
        assert_eq!(category(2), (Some("drums".into()), Some("name".into()), Some(NAME_RULES_VERSION)));
        assert_eq!(category(3), (None, None, Some(NAME_RULES_VERSION)));
        // Without the sound model, no embeddings are computed.
        let p = analyzer.progress(&conn, Vec::new());
        assert!(!p.model_ready && p.pending_embeddings == 0);
    }
}

#[cfg(test)]
mod real_library {
    use super::*;
    use std::sync::mpsc;

    /// Runs the embedding stage on a copy of a real library for `SECONDS`:
    /// `LIBRARY_DB=… CLAP_MODEL=… SECONDS=120 cargo test --release -- --ignored --nocapture embedding_throughput`
    #[test]
    #[ignore]
    fn embedding_throughput() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        rusqlite::Connection::open(std::env::var("LIBRARY_DB").unwrap())
            .unwrap()
            .execute("VACUUM INTO ?1", [db_path.to_string_lossy()])
            .unwrap();
        let mut conn = db::open(&db_path).unwrap();
        db::migrate(&mut conn).unwrap();
        let seconds: u64 = std::env::var("SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);

        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let analyzer = Analyzer::start(db_path.clone(), move |p| {
            let _ = tx.lock().unwrap().send(p);
        });
        analyzer.load_model(Path::new(&std::env::var("CLAP_MODEL").unwrap())).unwrap();
        let start = std::time::Instant::now();
        let embedded = |c: &Connection| -> i64 {
            c.query_row("SELECT COUNT(*) FROM samples WHERE embedding_version IS NOT NULL", [], |r| r.get(0)).unwrap()
        };
        let mut first: Option<(std::time::Instant, i64)> = None;
        while start.elapsed().as_secs() < seconds {
            if let Ok(p) = rx.recv_timeout(Duration::from_secs(5)) {
                if p.model_ready && p.pending == 0 && first.is_none() {
                    first = Some((std::time::Instant::now(), embedded(&conn)));
                }
                if p.model_ready && p.pending_embeddings == 0 && p.updated.is_empty() {
                    break;
                }
            }
        }
        analyzer.set_paused(true);
        let (t0, n0) = first.expect("embedding stage started");
        let n = embedded(&conn) - n0;
        let rate = n as f64 / t0.elapsed().as_secs_f64();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM samples", [], |r| r.get(0)).unwrap();
        println!("embedded {n} samples at {rate:.1}/s; whole library ≈ {:.0} min", total as f64 / rate / 60.0);
        let mut stmt = conn
            .prepare("SELECT category_source, category, COUNT(*) FROM samples WHERE embedding_version IS NOT NULL GROUP BY 1, 2 ORDER BY 1, 3 DESC")
            .unwrap();
        for row in stmt.query_map([], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, i64>(2)?))).unwrap() {
            println!("  {:?}", row.unwrap());
        }
        let keys: i64 = conn.query_row("SELECT COUNT(*) FROM samples WHERE key_source = 'audio'", [], |r| r.get(0)).unwrap();
        println!("  audio keys: {keys}");
    }
}
