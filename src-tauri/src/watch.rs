//! Scan scheduling and file-system watching.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

use crate::audio;
use crate::error::{Error, Result};
use crate::scan;

/// Runs scans on background threads, at most one per source at a time.
/// A request for a source that is already scanning queues exactly one rerun.
pub struct Scanner {
    db_path: PathBuf,
    on_progress: Box<dyn Fn(scan::ScanProgress) + Send + Sync>,
    jobs: Mutex<HashMap<i64, Job>>,
}

#[derive(Default)]
struct Job {
    running: bool,
    rerun: bool,
}

impl Scanner {
    pub fn new(db_path: PathBuf, on_progress: impl Fn(scan::ScanProgress) + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self { db_path, on_progress: Box::new(on_progress), jobs: Mutex::new(HashMap::new()) })
    }

    pub fn request(self: &Arc<Self>, source_id: i64) {
        {
            let mut jobs = self.jobs.lock().unwrap();
            let job = jobs.entry(source_id).or_default();
            if job.running {
                job.rerun = true;
                return;
            }
            job.running = true;
        }
        let this = Arc::clone(self);
        std::thread::spawn(move || loop {
            scan::scan_source(&this.db_path, source_id, &this.on_progress);
            let mut jobs = this.jobs.lock().unwrap();
            let job = jobs.entry(source_id).or_default();
            if job.rerun {
                job.rerun = false;
            } else {
                job.running = false;
                break;
            }
        });
    }
}

/// Wait this long after the last change before rescanning, so a folder copy
/// triggers one scan instead of hundreds.
const QUIET_PERIOD: Duration = Duration::from_millis(800);
/// Rescan at least this often while changes keep arriving.
const MAX_DELAY: Duration = Duration::from_secs(5);

/// Watches every source folder and requests a rescan of the affected source on change.
pub struct LibraryWatcher {
    watcher: Mutex<Option<RecommendedWatcher>>,
    roots: Arc<Mutex<HashMap<i64, PathBuf>>>,
}

impl LibraryWatcher {
    pub fn new(scanner: Arc<Scanner>) -> Self {
        let roots: Arc<Mutex<HashMap<i64, PathBuf>>> = Arc::default();
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let watcher = match notify::recommended_watcher(tx) {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("file watching unavailable: {e}");
                None
            }
        };

        let event_roots = Arc::clone(&roots);
        std::thread::Builder::new()
            .name("library-watcher".into())
            .spawn(move || debounce_loop(rx, &event_roots, &scanner))
            .expect("spawn watcher thread");

        Self { watcher: Mutex::new(watcher), roots }
    }

    pub fn watch(&self, source_id: i64, path: &Path) -> Result<()> {
        if let Some(w) = self.watcher.lock().unwrap().as_mut() {
            w.watch(path, RecursiveMode::Recursive).map_err(|e| Error::Other(e.to_string()))?;
        }
        // Events carry resolved paths (e.g. /private/var for /var), so match on those.
        let root = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.roots.lock().unwrap().insert(source_id, root);
        Ok(())
    }

    pub fn unwatch(&self, source_id: i64, path: &Path) {
        self.roots.lock().unwrap().remove(&source_id);
        if let Some(w) = self.watcher.lock().unwrap().as_mut() {
            let _ = w.unwatch(path);
        }
    }
}

/// Changes that can affect the index: audio files and directories (renames, moves).
/// Sidecar files such as Ableton's `.asd` are written next to samples and ignored.
fn is_relevant(path: &Path) -> bool {
    if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("._")) {
        return false;
    }
    path.extension().is_none() || audio::is_audio_file(path)
}

fn debounce_loop(
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
    roots: &Mutex<HashMap<i64, PathBuf>>,
    scanner: &Arc<Scanner>,
) {
    let mut pending: HashSet<i64> = HashSet::new();
    let mut first_change: Option<Instant> = None;
    loop {
        let event = if pending.is_empty() {
            match rx.recv() {
                Ok(e) => Some(e),
                Err(_) => return,
            }
        } else {
            match rx.recv_timeout(QUIET_PERIOD) {
                Ok(e) => Some(e),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };

        let quiet = event.is_none();
        if let Some(Ok(event)) = event {
            if !matches!(event.kind, EventKind::Access(_)) {
                let roots = roots.lock().unwrap();
                for path in event.paths.iter().filter(|p| is_relevant(p)) {
                    if let Some((&id, _)) = roots.iter().find(|(_, root)| path.starts_with(root)) {
                        pending.insert(id);
                        first_change.get_or_insert_with(Instant::now);
                    }
                }
            }
        }

        let overdue = first_change.is_some_and(|t| t.elapsed() >= MAX_DELAY);
        if !pending.is_empty() && (quiet || overdue) {
            for id in pending.drain() {
                scanner.request(id);
            }
            first_change = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn new_files_trigger_a_rescan() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        std::fs::create_dir_all(&root).unwrap();
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

        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let scanner = Scanner::new(db_path.clone(), move |p: scan::ScanProgress| {
            if p.done {
                let _ = tx.lock().unwrap().send(p);
            }
        });
        let watcher = LibraryWatcher::new(Arc::clone(&scanner));
        watcher.watch(id, &root).unwrap();
        std::thread::sleep(Duration::from_millis(300));

        // Ignored sidecar file, then a real sample.
        std::fs::write(root.join("kick.asd"), b"ableton").unwrap();
        let spec = hound::WavSpec { channels: 1, sample_rate: 44100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(root.join("kick.wav"), spec).unwrap();
        for _ in 0..4410 {
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();

        let done = rx.recv_timeout(Duration::from_secs(10)).expect("rescan after change");
        assert_eq!(done.source_id, id);
        assert!(done.error.is_none());
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM samples", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
