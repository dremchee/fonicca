//! Exact duplicates: library files with the same content. Candidates share the
//! head/tail fingerprint and size; files larger than the fingerprinted part are
//! confirmed with a full-content hash before anything can be moved to the Trash.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use rayon::prelude::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::library::{self, Filter, Sample};
use crate::similar::Ranking;

pub const VERIFY_EVENT: &str = "duplicates-verified";

/// Files up to this size are hashed whole by the fingerprint (two 64 KiB chunks).
const FINGERPRINTED_WHOLE: i64 = 128 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copy {
    #[serde(flatten)]
    pub sample: Sample,
    pub mtime: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Stable while the files are unchanged: fingerprint, size and, once known, hash.
    pub key: String,
    pub size: i64,
    /// False while some copies still wait for their full-content hash.
    pub verified: bool,
    pub copies: Vec<Copy>,
}

const CANDIDATES: &str = "SELECT fingerprint, size FROM samples WHERE fingerprint IS NOT NULL
                          GROUP BY fingerprint, size HAVING COUNT(*) > 1";

/// Number of candidate groups, for the sidebar; a few may turn out different once verified.
pub fn count(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row(&format!("SELECT COUNT(*) FROM ({CANDIDATES})"), [], |r| r.get(0))?)
}

pub fn groups(conn: &Connection) -> Result<Vec<Group>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.fingerprint, s.size, s.mtime, s.content_hash FROM samples s
         JOIN ({CANDIDATES}) g USING (fingerprint, size)
         ORDER BY s.fingerprint, s.size, s.id"
    ))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<Vec<u8>>>(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // (key, size, verified, [(id, mtime)])
    let mut found: Vec<(String, i64, bool, Vec<(i64, i64)>)> = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let (fp, size) = (rows[i].1, rows[i].2);
        let end = rows[i..].iter().position(|r| (r.1, r.2) != (fp, size)).map_or(rows.len(), |n| i + n);
        let members = &rows[i..end];
        i = end;
        let key = format!("{fp:x}:{size}");
        if size <= FINGERPRINTED_WHOLE || members.iter().any(|m| m.4.is_none()) {
            let verified = size <= FINGERPRINTED_WHOLE;
            found.push((key, size, verified, members.iter().map(|m| (m.0, m.3)).collect()));
            continue;
        }
        // Every copy hashed: split into groups of identical content.
        let mut by_hash: HashMap<&[u8], Vec<(i64, i64)>> = HashMap::new();
        for m in members {
            by_hash.entry(m.4.as_deref().unwrap()).or_default().push((m.0, m.3));
        }
        for (hash, copies) in by_hash {
            if copies.len() > 1 {
                let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
                found.push((format!("{key}:{hex}"), size, true, copies));
            }
        }
    }

    let ids: Vec<i64> = found.iter().flat_map(|g| g.3.iter().map(|c| c.0)).collect();
    let ranking = Ranking { ids, scores: HashMap::new() };
    let mut samples: HashMap<i64, Sample> = library::list_ranked(conn, &Filter::default(), Some(&ranking), 0, ranking.ids.len() as i64)?
        .into_iter()
        .map(|s| (s.id, s))
        .collect();

    let mut groups: Vec<Group> = found
        .into_iter()
        .filter_map(|(key, size, verified, members)| {
            let copies: Vec<Copy> = members
                .into_iter()
                .filter_map(|(id, mtime)| samples.remove(&id).map(|sample| Copy { sample, mtime }))
                .collect();
            (copies.len() > 1).then_some(Group { key, size, verified, copies })
        })
        .collect();
    groups.sort_by_cached_key(|g| (g.copies[0].sample.name.to_lowercase(), g.copies[0].sample.path.clone()));
    Ok(groups)
}

/// Large candidates still missing a full-content hash: (id, path, size, mtime).
fn unhashed(conn: &Connection) -> Result<Vec<(i64, String, i64, i64)>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.path, s.size, s.mtime FROM samples s
         JOIN ({CANDIDATES}) g USING (fingerprint, size)
         WHERE s.size > ?1 AND s.content_hash IS NULL"
    ))?;
    let rows = stmt
        .query_map([FINGERPRINTED_WHOLE], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn content_hash(path: &Path) -> Result<[u8; 16]> {
    let mut file = File::open(path)?;
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.digest128().to_le_bytes())
}

/// Hashes every large candidate that has no full-content hash yet. Returns how many
/// were hashed. Unreadable files stay unverified and are never trashed.
pub fn verify(db_path: &Path) -> Result<usize> {
    let pending = unhashed(&crate::db::open(db_path)?)?;
    let hashed: Vec<(i64, i64, i64, [u8; 16])> = pending
        .into_par_iter()
        .filter_map(|(id, path, size, mtime)| content_hash(Path::new(&path)).ok().map(|h| (id, size, mtime, h)))
        .collect();
    let mut conn = crate::db::open(db_path)?;
    let tx = conn.transaction()?;
    {
        // Size and mtime guard against a file that changed while it was being read.
        let mut stmt = tx.prepare("UPDATE samples SET content_hash = ?1 WHERE id = ?2 AND size = ?3 AND mtime = ?4")?;
        for (id, size, mtime, hash) in &hashed {
            stmt.execute(params![hash.as_slice(), id, size, mtime])?;
        }
    }
    tx.commit()?;
    Ok(hashed.len())
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Move the extra copies to the system Trash.
    Trash,
    /// Leave them on disk but keep them out of the library.
    Hide,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub keep: i64,
    pub extras: Vec<i64>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveReport {
    /// Extra copies trashed or hidden.
    pub removed: usize,
    pub bytes: i64,
    /// Why some copies were left alone, one line each.
    pub skipped: Vec<String>,
}

struct Row {
    path: String,
    source_id: i64,
    fingerprint: Option<i64>,
    size: i64,
    hash: Option<Vec<u8>>,
}

fn row(conn: &Connection, id: i64) -> Result<Option<Row>> {
    Ok(conn
        .query_row(
            "SELECT path, source_id, fingerprint, size, content_hash FROM samples WHERE id = ?1",
            [id],
            |r| Ok(Row { path: r.get(0)?, source_id: r.get(1)?, fingerprint: r.get(2)?, size: r.get(3)?, hash: r.get(4)? }),
        )
        .optional()?)
}

/// Same content as the kept copy, as far as the database knows.
fn same_content(keep: &Row, extra: &Row, action: Action) -> bool {
    if keep.fingerprint.is_none() || keep.fingerprint != extra.fingerprint || keep.size != extra.size {
        return false;
    }
    // Hiding is undoable, so an unverified large file may be hidden; trashing needs proof.
    keep.size <= FINGERPRINTED_WHOLE || action == Action::Hide || (keep.hash.is_some() && keep.hash == extra.hash)
}

/// Gives the kept copy the favorite flag, tags and collections of the extras.
fn merge_user_data(conn: &Connection, keep: i64, extras: &[i64]) -> Result<()> {
    let list = serde_json::to_string(extras).expect("ids serialize");
    conn.execute(
        "UPDATE samples SET favorite = 1 WHERE id = ?1
         AND EXISTS (SELECT 1 FROM samples WHERE favorite = 1 AND id IN (SELECT value FROM json_each(?2)))",
        params![keep, list],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO sample_tags (sample_id, tag_id)
         SELECT ?1, tag_id FROM sample_tags WHERE sample_id IN (SELECT value FROM json_each(?2))",
        params![keep, list],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO collection_items (collection_id, sample_id, added_at)
         SELECT collection_id, ?1, MIN(added_at) FROM collection_items
         WHERE sample_id IN (SELECT value FROM json_each(?2)) GROUP BY collection_id",
        params![keep, list],
    )?;
    Ok(())
}

/// Trashes or hides the extra copies of each group after merging their user data
/// into the kept copy. Copies that are not proven duplicates of an existing kept
/// file are skipped, never removed.
pub fn resolve(conn: &mut Connection, resolutions: &[Resolution], action: Action) -> Result<ResolveReport> {
    let mut report = ResolveReport::default();
    for res in resolutions {
        let Some(keep) = row(conn, res.keep)? else {
            report.skipped.push(format!("sample {} is no longer in the library", res.keep));
            continue;
        };
        if !Path::new(&keep.path).is_file() {
            report.skipped.push(format!("{}: the copy to keep is missing on disk", keep.path));
            continue;
        }
        let mut extras: Vec<(i64, Row)> = Vec::new();
        for &id in res.extras.iter().filter(|&&id| id != res.keep) {
            match row(conn, id)? {
                Some(r) if same_content(&keep, &r, action) => extras.push((id, r)),
                Some(r) => report.skipped.push(format!("{}: not verified as a copy of {}", r.path, keep.path)),
                None => {}
            }
        }
        if extras.is_empty() {
            continue;
        }

        let ids: Vec<i64> = extras.iter().map(|e| e.0).collect();
        let tx = conn.transaction()?;
        merge_user_data(&tx, res.keep, &ids)?;
        for (id, extra) in &extras {
            if action == Action::Trash {
                // A copy already gone from disk only needs its row removed.
                if Path::new(&extra.path).exists() {
                    if let Err(e) = trash::delete(&extra.path) {
                        report.skipped.push(format!("{}: {e}", extra.path));
                        continue;
                    }
                }
            } else {
                tx.execute(
                    "INSERT OR REPLACE INTO hidden (path, source_id, hidden_at) VALUES (?1, ?2, unixepoch())",
                    params![extra.path, extra.source_id],
                )?;
            }
            tx.execute("DELETE FROM samples WHERE id = ?1", [id])?;
            report.removed += 1;
            report.bytes += extra.size;
        }
        tx.commit()?;
    }
    Ok(report)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hidden {
    pub path: String,
    pub source_id: i64,
    pub hidden_at: i64,
}

pub fn list_hidden(conn: &Connection) -> Result<Vec<Hidden>> {
    let mut stmt = conn.prepare("SELECT path, source_id, hidden_at FROM hidden ORDER BY hidden_at DESC, path")?;
    let rows = stmt
        .query_map([], |r| Ok(Hidden { path: r.get(0)?, source_id: r.get(1)?, hidden_at: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Hidden paths of one source, for the scanner to skip.
pub fn hidden_paths(conn: &Connection, source_id: i64) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT path FROM hidden WHERE source_id = ?1")?;
    let paths = stmt.query_map([source_id], |r| r.get(0))?.collect::<rusqlite::Result<HashSet<_>>>()?;
    Ok(paths)
}

/// Unhides paths (all of them for `None`). Returns the sources to rescan so the
/// files come back, with their tags reattached from `detached`.
pub fn unhide(conn: &Connection, paths: Option<&[String]>) -> Result<Vec<i64>> {
    let sources: Vec<i64> = match paths {
        Some(paths) => {
            let list = serde_json::to_string(paths).expect("paths serialize");
            let mut stmt = conn.prepare(
                "DELETE FROM hidden WHERE path IN (SELECT value FROM json_each(?1)) RETURNING source_id",
            )?;
            let ids = stmt.query_map([list], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        }
        None => {
            let mut stmt = conn.prepare("DELETE FROM hidden RETURNING source_id")?;
            let ids = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            ids
        }
    };
    let mut unique: Vec<i64> = sources.into_iter().collect::<HashSet<_>>().into_iter().collect();
    unique.sort_unstable();
    Ok(unique)
}

pub fn preferred_folders(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM preferred_folders ORDER BY added_at DESC, rowid DESC")?;
    let rows = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn set_preferred_folder(conn: &Connection, path: &str, preferred: bool) -> Result<()> {
    if path.trim().is_empty() {
        return Err(Error::Other("empty folder path".into()));
    }
    if preferred {
        conn.execute(
            "INSERT INTO preferred_folders (path, added_at) VALUES (?1, unixepoch())
             ON CONFLICT(path) DO UPDATE SET added_at = excluded.added_at",
            [path],
        )?;
    } else {
        conn.execute("DELETE FROM preferred_folders WHERE path = ?1", [path])?;
    }
    Ok(())
}

#[cfg(test)]
fn group_ids(groups: &[Group]) -> Vec<Vec<i64>> {
    groups.iter().map(|g| g.copies.iter().map(|c| c.sample.id).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open(&dir.path().join("library.db")).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO sources (id, path, added_at) VALUES (1, ?1, 0)", [dir.path().to_string_lossy()])
            .unwrap();
        (dir, conn)
    }

    /// Writes a file and its row, fingerprinted the way the scanner does.
    fn add(conn: &Connection, dir: &Path, rel: &str, content: &[u8]) -> i64 {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let fp = crate::audio::fingerprint(&path, content.len() as u64).unwrap();
        conn.query_row(
            "INSERT INTO samples (source_id, path, name, ext, size, mtime, fingerprint)
             VALUES (1, ?1, ?2, 'wav', ?3, 0, ?4) RETURNING id",
            params![path.to_string_lossy(), rel, content.len() as i64, fp],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// LIBRARY_DB=/path/to/copy.db cargo test --lib duplicates::tests::real_library -- --ignored --nocapture
    /// (writes content hashes into that database: use a copy).
    #[test]
    #[ignore]
    fn real_library() {
        let db_path = std::path::PathBuf::from(std::env::var("LIBRARY_DB").unwrap());
        let mut conn = crate::db::open(&db_path).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        let summary = |gs: &[Group]| {
            let extra: usize = gs.iter().map(|g| g.copies.len() - 1).sum();
            let bytes: i64 = gs.iter().map(|g| g.size * (g.copies.len() as i64 - 1)).sum();
            let unverified = gs.iter().filter(|g| !g.verified).count();
            format!("{} groups, {extra} extra copies, {} MB, {unverified} unverified", gs.len(), bytes / 1_048_576)
        };
        let t = std::time::Instant::now();
        let before = groups(&conn).unwrap();
        println!("count {} · groups {} in {:?}", count(&conn).unwrap(), summary(&before), t.elapsed());
        let t = std::time::Instant::now();
        let hashed = verify(&db_path).unwrap();
        println!("hashed {hashed} files in {:?}", t.elapsed());
        let after = groups(&conn).unwrap();
        println!("after: {}", summary(&after));
    }

    #[test]
    fn large_lookalikes_split_after_verification() {
        let (dir, conn) = library();
        let small_a = add(&conn, dir.path(), "a/kick.wav", b"kick");
        let small_b = add(&conn, dir.path(), "b/kick.wav", b"kick");
        // Same head, tail and size but a different middle: the fingerprint cannot tell.
        let big = |middle: u8| {
            let mut v = vec![1u8; 300 * 1024];
            v[150 * 1024] = middle;
            v
        };
        let big_a = add(&conn, dir.path(), "a/loop.wav", &big(1));
        let big_b = add(&conn, dir.path(), "b/loop.wav", &big(1));
        let big_c = add(&conn, dir.path(), "c/loop.wav", &big(2));

        let before = groups(&conn).unwrap();
        assert_eq!(group_ids(&before), vec![vec![small_a, small_b], vec![big_a, big_b, big_c]]);
        assert!(before[0].verified && !before[1].verified);

        assert_eq!(verify(&dir.path().join("library.db")).unwrap(), 3);
        let after = groups(&conn).unwrap();
        assert_eq!(group_ids(&after), vec![vec![small_a, small_b], vec![big_a, big_b]]);
        assert!(after.iter().all(|g| g.verified));
    }

    #[test]
    fn trash_merges_user_data_and_refuses_unverified() {
        let (dir, mut conn) = library();
        let keep = add(&conn, dir.path(), "a/snare.wav", b"snare");
        let extra = add(&conn, dir.path(), "b/snare.wav", b"snare");
        let tag = library::ensure_tag(&conn, "crisp").unwrap();
        library::tag_samples(&mut conn, &[extra], tag.id).unwrap();
        conn.execute("UPDATE samples SET favorite = 1 WHERE id = ?1", [extra]).unwrap();

        let big = vec![7u8; 200 * 1024];
        let big_keep = add(&conn, dir.path(), "a/pad.wav", &big);
        let big_extra = add(&conn, dir.path(), "b/pad.wav", &big);

        let report = resolve(
            &mut conn,
            &[Resolution { keep, extras: vec![extra] }, Resolution { keep: big_keep, extras: vec![big_extra] }],
            Action::Hide,
        )
        .unwrap();
        // Hiding works without verification and keeps files on disk.
        assert_eq!(report.removed, 2);
        assert!(dir.path().join("b/snare.wav").exists());
        let (fav, tags): (bool, i64) = conn
            .query_row(
                "SELECT favorite, (SELECT COUNT(*) FROM sample_tags WHERE sample_id = ?1) FROM samples WHERE id = ?1",
                [keep],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(fav && tags == 1);
        assert_eq!(hidden_paths(&conn, 1).unwrap().len(), 2);

        // Unhiding returns the source to rescan and clears the list.
        assert_eq!(unhide(&conn, None).unwrap(), vec![1]);
        assert!(list_hidden(&conn).unwrap().is_empty());

        // Trash needs a full-content hash for large files.
        let again = add(&conn, dir.path(), "c/pad.wav", &big);
        let report = resolve(&mut conn, &[Resolution { keep: big_keep, extras: vec![again] }], Action::Trash).unwrap();
        assert_eq!(report.removed, 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(dir.path().join("c/pad.wav").exists());
    }
}
