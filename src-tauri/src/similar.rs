//! "Find similar": nearest neighbours by CLAP embedding.
//!
//! All embeddings are kept in memory (2 KB each, ~64 MB for 32k samples) and
//! searched by brute force, which takes a few milliseconds at this size.

use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::Connection;

use crate::clap::{self, DIM};
use crate::error::Result;

/// How many neighbours a similarity search returns.
pub const LIMIT: usize = 500;

/// Result of one search: ids best first, with cosine similarity.
pub struct Ranking {
    pub ids: Vec<i64>,
    pub scores: HashMap<i64, f32>,
}

impl Ranking {
    /// The ids as a JSON array for `json_each`, which keeps their order in `key`.
    pub fn ids_json(&self) -> String {
        serde_json::to_string(&self.ids).expect("ids serialize")
    }
}

struct Index {
    ids: Vec<i64>,
    vectors: Vec<f32>,
}

impl Index {
    fn load(conn: &Connection) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT id, embedding FROM samples WHERE embedding IS NOT NULL")?;
        let mut ids = Vec::new();
        let mut vectors = Vec::new();
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
        for row in rows {
            let (id, bytes) = row?;
            if bytes.len() == DIM * 4 {
                ids.push(id);
                vectors.extend(clap::from_bytes(&bytes));
            }
        }
        Ok(Self { ids, vectors })
    }

    fn vector(&self, id: i64) -> Option<&[f32]> {
        let i = self.ids.iter().position(|&x| x == id)?;
        Some(&self.vectors[i * DIM..(i + 1) * DIM])
    }

    fn nearest(&self, query: &[f32], n: usize) -> Ranking {
        let mut scored: Vec<(f32, i64)> =
            self.vectors.chunks_exact(DIM).zip(&self.ids).map(|(v, &id)| (clap::dot(query, v), id)).collect();
        let n = n.min(scored.len());
        if n < scored.len() {
            scored.select_nth_unstable_by(n, |a, b| b.0.total_cmp(&a.0));
            scored.truncate(n);
        }
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        Ranking { ids: scored.iter().map(|s| s.1).collect(), scores: scored.iter().map(|s| (s.1, s.0)).collect() }
    }
}

/// Embedding index plus the last search, so paging through results does not
/// search again. Reloads when the number of stored embeddings changes.
#[derive(Default)]
pub struct Similar {
    index: Option<(i64, Index)>,
    last: Option<(i64, Arc<Ranking>)>,
}

fn embedding_count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM samples WHERE embedding IS NOT NULL", [], |r| r.get(0))?)
}

impl Similar {
    /// Samples most similar to `id`, or `None` if it has no embedding yet.
    pub fn search(&mut self, conn: &Connection, id: i64) -> Result<Option<Arc<Ranking>>> {
        let count = embedding_count(conn)?;
        if self.index.as_ref().is_none_or(|(c, _)| *c != count) {
            self.index = Some((count, Index::load(conn)?));
            self.last = None;
        }
        if let Some((last_id, ranking)) = &self.last {
            if *last_id == id {
                return Ok(Some(Arc::clone(ranking)));
            }
        }
        let (_, index) = self.index.as_ref().expect("index loaded");
        let Some(query) = index.vector(id) else { return Ok(None) };
        let ranking = Arc::new(index.nearest(query, LIMIT));
        self.last = Some((id, Arc::clone(&ranking)));
        Ok(Some(ranking))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use rusqlite::params;

    fn vector(angle: f32) -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[0] = angle.cos();
        v[1] = angle.sin();
        v
    }

    #[test]
    fn ranks_by_cosine_and_reloads_when_embeddings_change() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO sources (id, path, added_at) VALUES (1, '/lib', 0)", []).unwrap();
        for (id, angle) in [(1, 0.0f32), (2, 0.1), (3, 1.5), (4, 0.5)] {
            conn.execute(
                "INSERT INTO samples (id, source_id, path, name, ext, size, mtime, embedding)
                 VALUES (?1, 1, ?2, 'x', 'wav', 0, 0, ?3)",
                params![id, format!("/lib/{id}.wav"), clap::to_bytes(&vector(angle))],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO samples (id, source_id, path, name, ext, size, mtime) VALUES (5, 1, '/lib/5.wav', 'x', 'wav', 0, 0)",
            [],
        )
        .unwrap();

        let mut similar = Similar::default();
        let r = similar.search(&conn, 1).unwrap().unwrap();
        assert_eq!(r.ids, vec![1, 2, 4, 3]);
        assert!((r.scores[&1] - 1.0).abs() < 1e-6);
        assert!(similar.search(&conn, 5).unwrap().is_none());

        // A new embedding invalidates the cached index and search.
        conn.execute("UPDATE samples SET embedding = ?1 WHERE id = 5", [clap::to_bytes(&vector(0.05))]).unwrap();
        let r = similar.search(&conn, 1).unwrap().unwrap();
        assert_eq!(r.ids[..3], [1, 5, 2]);
    }
}

#[cfg(test)]
mod real_library {
    use super::*;
    use crate::analyzer::Analyzer;
    use std::path::Path;
    use std::time::Duration;

    /// Embeds part of a real library copy for `SECONDS`, then prints the nearest
    /// neighbours of a few samples:
    /// `LIBRARY_DB=… CLAP_MODEL=… SECONDS=120 cargo test --release -- --ignored --nocapture neighbours_report`
    #[test]
    #[ignore]
    fn neighbours_report() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("library.db");
        Connection::open(std::env::var("LIBRARY_DB").unwrap())
            .unwrap()
            .execute("VACUUM INTO ?1", [db_path.to_string_lossy()])
            .unwrap();
        let mut conn = crate::db::open(&db_path).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        let analyzer = Analyzer::start(db_path.clone(), |_| {});
        analyzer.load_model(Path::new(&std::env::var("CLAP_MODEL").unwrap())).unwrap();
        let seconds: u64 = std::env::var("SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(90);
        std::thread::sleep(Duration::from_secs(seconds));
        analyzer.set_paused(true);

        let mut similar = Similar::default();
        let t = std::time::Instant::now();
        for category in ["kick", "hihat", "bass", "vocal", "keys"] {
            let Ok((id, name)) = conn.query_row(
                "SELECT id, name FROM samples WHERE category = ?1 AND embedding IS NOT NULL ORDER BY random() LIMIT 1",
                [category],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
            ) else {
                continue;
            };
            let ranking = similar.search(&conn, id).unwrap().unwrap();
            println!("\n{name} [{category}]");
            for nid in ranking.ids.iter().skip(1).take(8) {
                let (n, c): (String, Option<String>) = conn
                    .query_row("SELECT name, category FROM samples WHERE id = ?1", [nid], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap();
                println!("  {:.2}  {n} [{}]", ranking.scores[nid], c.unwrap_or_default());
            }
        }
        println!("\nsearches took {:?} (incl. index load)", t.elapsed());
    }
}
