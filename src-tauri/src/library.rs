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

fn clean_tag_name(name: &str) -> Result<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(Error::Other("tag name is empty".into()));
    }
    Ok(name)
}

/// Returns the tag with this name (case-insensitive), creating it if needed.
pub fn ensure_tag(conn: &Connection, name: &str) -> Result<TagRef> {
    let name = clean_tag_name(name)?;
    conn.execute("INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING", [&name])?;
    Ok(conn.query_row("SELECT id, name FROM tags WHERE name = ?1", [&name], |r| {
        Ok(TagRef { id: r.get(0)?, name: r.get(1)? })
    })?)
}

pub fn rename_tag(conn: &Connection, id: i64, name: &str) -> Result<()> {
    let name = clean_tag_name(name)?;
    conn.execute("UPDATE tags SET name = ?1 WHERE id = ?2", params![name, id]).map_err(|e| {
        match e {
            rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                Error::Other(format!("tag \"{name}\" already exists"))
            }
            e => e.into(),
        }
    })?;
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
}
