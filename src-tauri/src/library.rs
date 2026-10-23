//! Queries over the sample index. Kept free of Tauri types so they can be tested
//! against a plain connection.

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filter {
    pub query: Option<String>,
    pub source_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub favorites: bool,
    pub collection_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub id: i64,
    pub source_id: i64,
    pub path: String,
    pub name: String,
    pub ext: String,
    pub size: i64,
    pub duration: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bit_depth: Option<i64>,
    pub favorite: bool,
    pub tags: Vec<TagRef>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub sample_count: i64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub sample_count: i64,
}

/// Turns free text into an FTS5 query: every word must match as a prefix
/// in the name, folder or tags.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

/// Builds the shared WHERE clause for listing, counting and id selection.
fn where_clause(f: &Filter) -> (String, Vec<Box<dyn ToSql>>) {
    let mut clauses: Vec<&str> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(id) = f.source_id {
        clauses.push("s.source_id = ?");
        args.push(Box::new(id));
    }
    if let Some(id) = f.tag_id {
        clauses.push("s.id IN (SELECT sample_id FROM sample_tags WHERE tag_id = ?)");
        args.push(Box::new(id));
    }
    if let Some(id) = f.collection_id {
        clauses.push("s.id IN (SELECT sample_id FROM collection_items WHERE collection_id = ?)");
        args.push(Box::new(id));
    }
    if f.favorites {
        clauses.push("s.favorite = 1");
    }
    if let Some(q) = f.query.as_deref().and_then(fts_query) {
        clauses.push("s.id IN (SELECT rowid FROM samples_fts WHERE samples_fts MATCH ?)");
        args.push(Box::new(q));
    }
    let sql = if clauses.is_empty() { String::new() } else { format!("WHERE {}", clauses.join(" AND ")) };
    (sql, args)
}

fn args_iter(args: &[Box<dyn ToSql>]) -> impl Iterator<Item = &dyn ToSql> {
    args.iter().map(|a| a.as_ref())
}

pub fn count(conn: &Connection, f: &Filter) -> Result<i64> {
    let (filter, args) = where_clause(f);
    Ok(conn.query_row(
        &format!("SELECT COUNT(*) FROM samples s {filter}"),
        params_from_iter(args_iter(&args)),
        |r| r.get(0),
    )?)
}

const ORDER: &str = "ORDER BY s.name COLLATE NOCASE, s.path";

pub fn list(conn: &Connection, f: &Filter, offset: i64, limit: i64) -> Result<Vec<Sample>> {
    let (filter, mut args) = where_clause(f);
    args.push(Box::new(limit));
    args.push(Box::new(offset));
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT s.id, s.source_id, s.path, s.name, s.ext, s.size, s.duration, s.sample_rate,
                s.channels, s.bit_depth, s.favorite,
                (SELECT json_group_array(json_object('id', t.id, 'name', t.name)
                                         ORDER BY t.name COLLATE NOCASE)
                 FROM sample_tags st JOIN tags t ON t.id = st.tag_id
                 WHERE st.sample_id = s.id)
         FROM samples s {filter} {ORDER} LIMIT ? OFFSET ?"
    ))?;
    let rows = stmt
        .query_map(params_from_iter(args_iter(&args)), |r| {
            let tags: String = r.get(11)?;
            Ok(Sample {
                id: r.get(0)?,
                source_id: r.get(1)?,
                path: r.get(2)?,
                name: r.get(3)?,
                ext: r.get(4)?,
                size: r.get(5)?,
                duration: r.get(6)?,
                sample_rate: r.get(7)?,
                channels: r.get(8)?,
                bit_depth: r.get(9)?,
                favorite: r.get(10)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Ids of all matching samples in list order (for select-all and range selection).
pub fn ids(conn: &Connection, f: &Filter) -> Result<Vec<i64>> {
    let (filter, args) = where_clause(f);
    let mut stmt = conn.prepare(&format!("SELECT s.id FROM samples s {filter} {ORDER}"))?;
    let ids = stmt
        .query_map(params_from_iter(args_iter(&args)), |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

pub fn paths(conn: &Connection, ids: &[i64]) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached("SELECT path FROM samples WHERE id = ?1")?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(p) = stmt.query_row([id], |r| r.get(0)).optional()? {
            out.push(p);
        }
    }
    Ok(out)
}

pub fn set_favorite(conn: &mut Connection, ids: &[i64], favorite: bool) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("UPDATE samples SET favorite = ?1 WHERE id = ?2")?;
        for id in ids {
            stmt.execute(params![favorite, id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn list_tags(conn: &Connection) -> Result<Vec<Tag>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, COUNT(st.sample_id) FROM tags t
         LEFT JOIN sample_tags st ON st.tag_id = t.id
         GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
    )?;
    let tags = stmt
        .query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, sample_count: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tags)
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(Error::Other("name is empty".into()));
    }
    Ok(name)
}

/// Returns the tag with this name (case-insensitive), creating it if needed.
pub fn ensure_tag(conn: &Connection, name: &str) -> Result<TagRef> {
    let name = clean_name(name)?;
    conn.execute("INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING", [&name])?;
    Ok(conn.query_row("SELECT id, name FROM tags WHERE name = ?1", [&name], |r| {
        Ok(TagRef { id: r.get(0)?, name: r.get(1)? })
    })?)
}

pub fn rename_tag(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = clean_name(name)?;
    conn.execute("UPDATE tags SET name = ?1 WHERE id = ?2", params![name, id])
        .map_err(|e| name_taken(e, "tag", &name))?;
    Ok(())
}

pub fn delete_tag(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM tags WHERE id = ?1", [id])?;
    Ok(())
}

pub fn tag_samples(conn: &mut Connection, ids: &[i64], tag_id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("INSERT OR IGNORE INTO sample_tags (sample_id, tag_id) VALUES (?1, ?2)")?;
        for id in ids {
            stmt.execute(params![id, tag_id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn untag_samples(conn: &mut Connection, ids: &[i64], tag_id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("DELETE FROM sample_tags WHERE sample_id = ?1 AND tag_id = ?2")?;
        for id in ids {
            stmt.execute(params![id, tag_id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn name_taken(e: rusqlite::Error, what: &str, name: &str) -> Error {
    match e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
            Error::Other(format!("{what} \"{name}\" already exists"))
        }
        e => e.into(),
    }
}

// --- Collections -----------------------------------------------------------

pub fn list_collections(conn: &Connection) -> Result<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, COUNT(ci.sample_id) FROM collections c
         LEFT JOIN collection_items ci ON ci.collection_id = c.id
         GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| Ok(Collection { id: r.get(0)?, name: r.get(1)?, sample_count: r.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn create_collection(conn: &Connection, name: &str) -> Result<i64> {
    let name = clean_name(name)?;
    conn.query_row(
        "INSERT INTO collections (name, created_at) VALUES (?1, unixepoch()) RETURNING id",
        [&name],
        |r| r.get(0),
    )
    .map_err(|e| name_taken(e, "collection", &name))
}

/// Returns the collection with this name (case-insensitive), creating it if needed.
fn ensure_collection(conn: &Connection, name: &str) -> Result<i64> {
    let name = clean_name(name)?;
    conn.execute(
        "INSERT INTO collections (name, created_at) VALUES (?1, unixepoch()) ON CONFLICT(name) DO NOTHING",
        [&name],
    )?;
    Ok(conn.query_row("SELECT id FROM collections WHERE name = ?1", [&name], |r| r.get(0))?)
}

pub fn rename_collection(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = clean_name(name)?;
    conn.execute("UPDATE collections SET name = ?1 WHERE id = ?2", params![name, id])
        .map_err(|e| name_taken(e, "collection", &name))?;
    Ok(())
}

pub fn delete_collection(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

pub fn add_to_collection(conn: &mut Connection, ids: &[i64], collection_id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO collection_items (collection_id, sample_id, added_at) VALUES (?1, ?2, unixepoch())",
        )?;
        for id in ids {
            stmt.execute(params![collection_id, id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn remove_from_collection(conn: &mut Connection, ids: &[i64], collection_id: i64) -> Result<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("DELETE FROM collection_items WHERE collection_id = ?1 AND sample_id = ?2")?;
        for id in ids {
            stmt.execute(params![collection_id, id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

// --- Export / import of user data ------------------------------------------

const EXPORT_FORMAT: &str = "samplette-user-data";
const EXPORT_VERSION: u32 = 1;

/// Portable copy of everything the user added: favorites, tags and collections.
/// Samples are identified by path, with the content fingerprint as a fallback.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserData {
    pub format: String,
    pub version: u32,
    pub exported_at: i64,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub samples: Vec<SampleUserData>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleUserData {
    pub path: String,
    pub size: i64,
    pub fingerprint: Option<i64>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub collections: Vec<String>,
}

#[derive(Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub matched: usize,
    pub unmatched: usize,
}

fn names(conn: &Connection, sql: &str, id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(sql)?;
    let names = stmt.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(names)
}

pub fn export_user_data(conn: &Connection) -> Result<UserData> {
    let all = |sql: &str| -> Result<Vec<String>> {
        let mut stmt = conn.prepare(sql)?;
        let names = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(names)
    };
    let mut stmt = conn.prepare(
        "SELECT id, path, size, fingerprint, favorite FROM samples s
         WHERE favorite = 1
            OR EXISTS (SELECT 1 FROM sample_tags WHERE sample_id = s.id)
            OR EXISTS (SELECT 1 FROM collection_items WHERE sample_id = s.id)
         ORDER BY path",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<(i64, String, i64, Option<i64>, bool)>>>()?;

    let mut samples = Vec::with_capacity(rows.len());
    for (id, path, size, fingerprint, favorite) in rows {
        samples.push(SampleUserData {
            path,
            size,
            fingerprint,
            favorite,
            tags: names(
                conn,
                "SELECT t.name FROM sample_tags st JOIN tags t ON t.id = st.tag_id
                 WHERE st.sample_id = ?1 ORDER BY t.name COLLATE NOCASE",
                id,
            )?,
            collections: names(
                conn,
                "SELECT c.name FROM collection_items ci JOIN collections c ON c.id = ci.collection_id
                 WHERE ci.sample_id = ?1 ORDER BY c.name COLLATE NOCASE",
                id,
            )?,
        });
    }
    Ok(UserData {
        format: EXPORT_FORMAT.into(),
        version: EXPORT_VERSION,
        exported_at: conn.query_row("SELECT unixepoch()", [], |r| r.get(0))?,
        tags: all("SELECT name FROM tags ORDER BY name COLLATE NOCASE")?,
        collections: all("SELECT name FROM collections ORDER BY name COLLATE NOCASE")?,
        samples,
    })
}

/// Merges exported user data into the library: adds tags, collections and
/// favorites, never removes anything.
pub fn import_user_data(conn: &mut Connection, data: &UserData) -> Result<ImportReport> {
    if data.format != EXPORT_FORMAT || data.version > EXPORT_VERSION {
        return Err(Error::Other("not a Samplette export or made by a newer version".into()));
    }
    let tx = conn.transaction()?;
    let mut report = ImportReport::default();
    {
        for name in &data.tags {
            ensure_tag(&tx, name)?;
        }
        for name in &data.collections {
            ensure_collection(&tx, name)?;
        }
        let mut by_path = tx.prepare("SELECT id FROM samples WHERE path = ?1")?;
        let mut by_content =
            tx.prepare("SELECT id FROM samples WHERE fingerprint = ?1 AND size = ?2 ORDER BY id LIMIT 1")?;
        let mut favorite = tx.prepare("UPDATE samples SET favorite = 1 WHERE id = ?1")?;
        let mut tag = tx.prepare("INSERT OR IGNORE INTO sample_tags (sample_id, tag_id) VALUES (?1, ?2)")?;
        let mut collect = tx.prepare(
            "INSERT OR IGNORE INTO collection_items (collection_id, sample_id, added_at) VALUES (?1, ?2, unixepoch())",
        )?;

        for entry in &data.samples {
            let mut id: Option<i64> = by_path.query_row([&entry.path], |r| r.get(0)).optional()?;
            if id.is_none() {
                if let Some(fp) = entry.fingerprint {
                    id = by_content.query_row(params![fp, entry.size], |r| r.get(0)).optional()?;
                }
            }
            let Some(id) = id else {
                report.unmatched += 1;
                continue;
            };
            report.matched += 1;
            if entry.favorite {
                favorite.execute([id])?;
            }
            for name in &entry.tags {
                let t = ensure_tag(&tx, name)?;
                tag.execute(params![id, t.id])?;
            }
            for name in &entry.collections {
                let c = ensure_collection(&tx, name)?;
                collect.execute(params![c, id])?;
            }
        }
    }
    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    /// In-memory library: source "/lib" with a few samples.
    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO sources (id, path, added_at) VALUES (1, '/lib', 0)", []).unwrap();
        for (id, path) in [
            (1, "/lib/Trap Pack/Drums/Kick Hard.wav"),
            (2, "/lib/Trap Pack/Drums/Snare_Tight.wav"),
            (3, "/lib/Ambient/Pad Warm.wav"),
            (4, "/lib/Café Sounds/Kicker.wav"),
        ] {
            let name = path.rsplit('/').next().unwrap().trim_end_matches(".wav");
            conn.execute(
                "INSERT INTO samples (id, source_id, path, name, ext, size, mtime) VALUES (?1, 1, ?2, ?3, 'wav', 0, 0)",
                params![id, path, name],
            )
            .unwrap();
        }
        conn
    }

    fn search(conn: &Connection, q: &str) -> Vec<i64> {
        ids(conn, &Filter { query: Some(q.into()), ..Default::default() }).unwrap()
    }

    #[test]
    fn search_matches_name_prefixes_folders_and_diacritics() {
        let conn = setup();
        assert_eq!(search(&conn, "kick"), vec![1, 4]);
        assert_eq!(search(&conn, "kick hard"), vec![1]);
        assert_eq!(search(&conn, "tight"), vec![2]); // underscore splits words
        assert_eq!(search(&conn, "trap"), vec![1, 2]); // folder name
        assert_eq!(search(&conn, "cafe"), vec![4]);
        assert_eq!(search(&conn, "lib"), Vec::<i64>::new()); // source root is not indexed
        assert_eq!(search(&conn, "\"*"), vec![1, 4, 3, 2]); // no terms → no filter, name order
    }

    #[test]
    fn tags_are_searchable_and_follow_renames_and_deletes() {
        let mut conn = setup();
        let tag = ensure_tag(&conn, "  dark   vibe ").unwrap();
        assert_eq!(tag.name, "dark vibe");
        assert_eq!(ensure_tag(&conn, "DARK VIBE").unwrap().id, tag.id);

        tag_samples(&mut conn, &[1, 3], tag.id).unwrap();
        tag_samples(&mut conn, &[1], tag.id).unwrap(); // idempotent
        assert_eq!(search(&conn, "vibe"), vec![1, 3]);
        assert_eq!(
            ids(&conn, &Filter { tag_id: Some(tag.id), ..Default::default() }).unwrap(),
            vec![1, 3]
        );
        assert_eq!(list_tags(&conn).unwrap()[0].sample_count, 2);

        rename_tag(&conn, tag.id, "moody").unwrap();
        assert_eq!(search(&conn, "vibe"), Vec::<i64>::new());
        assert_eq!(search(&conn, "moody"), vec![1, 3]);

        untag_samples(&mut conn, &[3], tag.id).unwrap();
        assert_eq!(search(&conn, "moody"), vec![1]);

        delete_tag(&conn, tag.id).unwrap();
        assert_eq!(search(&conn, "moody"), Vec::<i64>::new());
        assert!(list(&conn, &Filter::default(), 0, 10).unwrap().iter().all(|s| s.tags.is_empty()));
    }

    #[test]
    fn rename_to_existing_name_fails() {
        let conn = setup();
        ensure_tag(&conn, "a").unwrap();
        let b = ensure_tag(&conn, "b").unwrap();
        assert!(rename_tag(&conn, b.id, "A").is_err());
    }

    #[test]
    fn list_includes_tags_and_favorites() {
        let mut conn = setup();
        let t1 = ensure_tag(&conn, "zeta").unwrap();
        let t2 = ensure_tag(&conn, "alpha").unwrap();
        tag_samples(&mut conn, &[1], t1.id).unwrap();
        tag_samples(&mut conn, &[1], t2.id).unwrap();
        set_favorite(&mut conn, &[1, 2], true).unwrap();
        set_favorite(&mut conn, &[2], false).unwrap();

        let rows = list(&conn, &Filter::default(), 0, 10).unwrap();
        let kick = rows.iter().find(|s| s.id == 1).unwrap();
        assert!(kick.favorite);
        assert_eq!(kick.tags, vec![t2.clone(), t1.clone()]);
        assert_eq!(ids(&conn, &Filter { favorites: true, ..Default::default() }).unwrap(), vec![1]);
        assert_eq!(count(&conn, &Filter { favorites: true, query: Some("kick".into()), ..Default::default() }).unwrap(), 1);
    }

    #[test]
    fn moving_or_deleting_a_sample_updates_the_index() {
        let conn = setup();
        conn.execute("UPDATE samples SET path = '/lib/Lofi/Kick Hard.wav' WHERE id = 1", []).unwrap();
        assert_eq!(search(&conn, "lofi"), vec![1]);
        assert_eq!(search(&conn, "trap"), vec![2]);
        conn.execute("DELETE FROM samples WHERE id = 2", []).unwrap();
        assert_eq!(search(&conn, "trap"), Vec::<i64>::new());
    }

    #[test]
    fn migration_backfills_existing_rows() {
        // Simulate a v1 database with data, then upgrade.
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO sources (id, path, added_at) VALUES (1, '/lib', 0)", []).unwrap();
        conn.execute(
            "INSERT INTO samples (source_id, path, name, ext, size, mtime) VALUES (1, '/lib/Old/Clap.wav', 'Clap', 'wav', 0, 0)",
            [],
        )
        .unwrap();
        db::migrate(&mut conn).unwrap();
        assert_eq!(search(&conn, "clap old"), vec![1]);
    }

    #[test]
    fn collections_filter_and_survive_name_clashes() {
        let mut conn = setup();
        let a = create_collection(&conn, "Best kicks").unwrap();
        assert!(create_collection(&conn, "best KICKS").is_err());
        add_to_collection(&mut conn, &[1, 4], a).unwrap();
        add_to_collection(&mut conn, &[1], a).unwrap(); // idempotent
        let f = Filter { collection_id: Some(a), ..Default::default() };
        assert_eq!(ids(&conn, &f).unwrap(), vec![1, 4]);
        assert_eq!(list_collections(&conn).unwrap()[0].sample_count, 2);

        remove_from_collection(&mut conn, &[4], a).unwrap();
        assert_eq!(ids(&conn, &f).unwrap(), vec![1]);
        rename_collection(&conn, a, "Kicks").unwrap();
        assert_eq!(list_collections(&conn).unwrap()[0].name, "Kicks");
        delete_collection(&conn, a).unwrap();
        assert!(list_collections(&conn).unwrap().is_empty());
    }

    #[test]
    fn collection_membership_follows_moved_files() {
        let mut conn = setup();
        conn.execute("UPDATE samples SET fingerprint = id * 1000", []).unwrap();
        let c = create_collection(&conn, "Set").unwrap();
        add_to_collection(&mut conn, &[3], c).unwrap();
        conn.execute("DELETE FROM samples WHERE id = 3", []).unwrap();
        conn.execute(
            "INSERT INTO samples (id, source_id, path, name, ext, size, mtime, fingerprint)
             VALUES (30, 1, '/lib/Moved/Pad Warm.wav', 'Pad Warm', 'wav', 0, 0, 3000)",
            [],
        )
        .unwrap();
        assert_eq!(ids(&conn, &Filter { collection_id: Some(c), ..Default::default() }).unwrap(), vec![30]);
    }

    #[test]
    fn export_import_round_trip() {
        let mut conn = setup();
        conn.execute("UPDATE samples SET fingerprint = id * 1000, size = id", []).unwrap();
        let t = ensure_tag(&conn, "dark").unwrap();
        ensure_tag(&conn, "unused").unwrap();
        tag_samples(&mut conn, &[1, 2], t.id).unwrap();
        set_favorite(&mut conn, &[3], true).unwrap();
        let c = create_collection(&conn, "Set A").unwrap();
        add_to_collection(&mut conn, &[2], c).unwrap();

        let data = export_user_data(&conn).unwrap();
        assert_eq!(data.samples.len(), 3);
        assert_eq!(data.tags, vec!["dark", "unused"]);
        let json = serde_json::to_string(&data).unwrap();

        // Fresh library where sample 2 has moved (new path, same content) and 1 is gone.
        let mut other = setup();
        other.execute("UPDATE samples SET fingerprint = id * 1000, size = id", []).unwrap();
        other.execute("DELETE FROM samples WHERE id = 1", []).unwrap();
        other.execute("UPDATE samples SET path = '/lib/New/Snare_Tight.wav' WHERE id = 2", []).unwrap();

        let parsed: UserData = serde_json::from_str(&json).unwrap();
        let report = import_user_data(&mut other, &parsed).unwrap();
        assert_eq!(report, ImportReport { matched: 2, unmatched: 1 });

        let rows = list(&other, &Filter::default(), 0, 10).unwrap();
        let snare = rows.iter().find(|s| s.id == 2).unwrap();
        assert_eq!(snare.tags.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), vec!["dark"]);
        assert!(rows.iter().find(|s| s.id == 3).unwrap().favorite);
        let cols = list_collections(&other).unwrap();
        assert_eq!((cols[0].name.as_str(), cols[0].sample_count), ("Set A", 1));
        assert!(list_tags(&other).unwrap().iter().any(|t| t.name == "unused"));

        // Importing twice changes nothing.
        assert_eq!(import_user_data(&mut other, &parsed).unwrap(), report);
        assert_eq!(list_collections(&other).unwrap()[0].sample_count, 1);
    }

    #[test]
    fn import_rejects_foreign_files() {
        let mut conn = setup();
        let data: UserData = serde_json::from_str(
            r#"{"format":"other","version":1,"exportedAt":0,"tags":[],"collections":[],"samples":[]}"#,
        )
        .unwrap();
        assert!(import_user_data(&mut conn, &data).is_err());
    }
}
