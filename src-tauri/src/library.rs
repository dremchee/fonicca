//! Queries over the sample index. Kept free of Tauri types so they can be tested
//! against a plain connection.

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::similar::Ranking;
#[cfg(test)]
mod benchmarks;

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortColumn {
    Tags, Category, Kind, Bpm, Key, Length, Type,
    #[default]
    #[serde(other)]
    Name,
}
#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Desc,
    #[default]
    #[serde(other)]
    Asc,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Filter {
    pub sort_by: SortColumn,
    pub sort_direction: SortDirection,
    pub query: Option<String>,
    pub source_id: Option<i64>,
    /// Include files in this directory and all its descendants.
    pub directory: Option<String>,
    pub tag_id: Option<i64>,
    pub favorites: bool,
    pub collection_id: Option<i64>,
    /// "oneshot", "loop" or "long".
    pub kind: Option<String>,
    pub bpm_min: Option<f64>,
    pub bpm_max: Option<f64>,
    /// Exact key name, e.g. "Am" or "F#".
    pub key: Option<String>,
    /// Category id, e.g. "kick" (see `category::CATEGORIES`).
    pub category: Option<String>,
    /// Show samples that sound like this one, most similar first.
    pub similar_to: Option<i64>,
    pub group_stems: bool,
    pub expanded_stems: Vec<i64>,
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
    pub kind: Option<String>,
    pub bpm: Option<f64>,
    pub bpm_source: Option<String>,
    pub key: Option<String>,
    pub key_source: Option<String>,
    pub peak_db: Option<f64>,
    pub lufs: Option<f64>,
    pub analyzed: bool,
    pub category: Option<String>,
    pub category_source: Option<String>,
    pub category_confidence: Option<f64>,
    /// Cosine similarity to the sample in `Filter::similar_to`.
    pub similarity: Option<f32>,
    pub stem_parent_id: Option<i64>,
    pub stem_kind: Option<String>,
    pub stem_count: i64,
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

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceFolder {
    pub path: String,
    pub sample_count: i64,
}

/// Directories containing indexed audio, including intermediate parents. Counts
/// include descendants; no filesystem walk is needed when expanding the sidebar.
pub fn source_folders(conn: &Connection, source_id: i64) -> Result<Vec<SourceFolder>> {
    let root: Option<String> = conn.query_row(
        "SELECT path FROM sources WHERE id = ?", [source_id], |r| r.get(0),
    ).optional()?;
    let Some(root) = root else { return Ok(Vec::new()) };
    let root = std::path::Path::new(&root);
    let mut counts = std::collections::BTreeMap::<String, i64>::new();
    let mut stmt = conn.prepare("SELECT path FROM samples WHERE source_id = ?")?;
    for path in stmt.query_map([source_id], |r| r.get::<_, String>(0))? {
        let path = path?;
        let mut parent = std::path::Path::new(&path).parent();
        while let Some(dir) = parent {
            if dir == root || !dir.starts_with(root) { break; }
            *counts.entry(dir.to_string_lossy().into_owned()).or_default() += 1;
            parent = dir.parent();
        }
    }
    Ok(counts.into_iter().map(|(path, sample_count)| SourceFolder { path, sample_count }).collect())
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
    if let Some(directory) = &f.directory {
        // Literal prefix with a separator: pack does not match pack-old, and
        // names containing SQL wildcard characters keep their literal meaning.
        let mut prefix = directory.clone();
        if !prefix.ends_with(std::path::MAIN_SEPARATOR) {
            prefix.push(std::path::MAIN_SEPARATOR);
        }
        clauses.push("substr(s.path, 1, length(?)) = ?");
        args.push(Box::new(prefix.clone()));
        args.push(Box::new(prefix));
    }
    if let Some(id) = f.tag_id {
        clauses.push("s.id IN (SELECT sample_id FROM sample_tags WHERE tag_id = ?)");
        args.push(Box::new(id));
    }
    if let Some(id) = f.collection_id {
        clauses.push("s.id IN (SELECT sample_id FROM collection_items WHERE collection_id = ?)");
        args.push(Box::new(id));
    }
    if let Some(kind) = &f.kind {
        clauses.push("s.kind = ?");
        args.push(Box::new(kind.clone()));
    }
    if let Some(min) = f.bpm_min {
        clauses.push("s.bpm >= ?");
        args.push(Box::new(min));
    }
    if let Some(max) = f.bpm_max {
        clauses.push("s.bpm <= ?");
        args.push(Box::new(max));
    }
    if let Some(category) = &f.category {
        clauses.push("s.category = ?");
        args.push(Box::new(category.clone()));
    }
    if let Some(key) = &f.key {
        clauses.push("s.musical_key = ?");
        args.push(Box::new(key.clone()));
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

/// Whitelisted expressions only: filter values never become SQL identifiers.
fn sort_order(f: &Filter, alias: &str) -> String {
    let value = match f.sort_by {
        SortColumn::Name => format!("{alias}.name"),
        SortColumn::Bpm => format!("{alias}.bpm"),
        SortColumn::Length => format!("{alias}.duration"),
        SortColumn::Category => format!("{alias}.category"),
        SortColumn::Kind => format!("{alias}.kind"),
        SortColumn::Key => format!("{alias}.musical_key"),
        SortColumn::Type => format!("{alias}.ext"),
        SortColumn::Tags => format!("(SELECT group_concat(name, char(31)) FROM (SELECT t.name FROM sample_tags st JOIN tags t ON t.id=st.tag_id WHERE st.sample_id={alias}.id ORDER BY t.name COLLATE FONICCA_NATURAL))"),
    };
    let direction = match f.sort_direction { SortDirection::Asc => "ASC", SortDirection::Desc => "DESC" };
    // Unknown/empty metadata stays last in either direction. Ties are stable
    // across pages, selection queries and repeated scans.
    let name_tie = if matches!(f.sort_by, SortColumn::Name) { String::new() }
        else { format!(", {alias}.name COLLATE FONICCA_NATURAL ASC") };
    format!("({value} IS NULL OR {value} = '') ASC, {value} COLLATE FONICCA_NATURAL {direction}{name_tie}, {alias}.path COLLATE FONICCA_NATURAL ASC, {alias}.id ASC")
}

/// `FROM … WHERE …` and `ORDER BY` for a filter. With a similarity ranking the
/// rows are limited to the ranked ids and keep the ranking's order.
fn source(f: &Filter, ranking: Option<&Ranking>) -> (String, Vec<Box<dyn ToSql>>, String) {
    let (filter, args) = where_clause(f);
    match ranking {
        Some(r) => {
            let mut all: Vec<Box<dyn ToSql>> = vec![Box::new(r.ids_json())];
            all.extend(args);
            (format!("FROM samples s JOIN json_each(?) r ON r.value = s.id {filter}"), all, "ORDER BY r.key".into())
        }
        None if f.group_stems && f.expanded_stems.is_empty() => {
            let from = if filter.is_empty() {
                // Without filters, walk the name index directly. A collapsed
                // root is an unlinked sample or the parent of an existing child.
                "FROM samples s WHERE
                    NOT EXISTS(SELECT 1 FROM sample_stems l WHERE l.child_id=s.id)
                    OR EXISTS(SELECT 1 FROM sample_stems l JOIN samples child ON child.id=l.child_id
                        WHERE l.parent_id=s.id)".to_owned()
            } else {
                // Resolve selective filters first (source, FTS, tags, etc.).
                // Scanning every root through correlated filters hurts small
                // folders/searches. IN deduplicates the matching root ids.
                let matching = filter.replace("s.", "matched.");
                format!("FROM samples s WHERE s.id IN (
                    SELECT coalesce(l.parent_id,matched.id) FROM samples matched
                    LEFT JOIN sample_stems l ON l.child_id=matched.id {matching})")
            };
            (from, args, format!("ORDER BY {}", sort_order(f, "s")))
        }
        None if f.group_stems => {
            let mut args = args;
            args.push(Box::new(serde_json::to_string(&f.expanded_stems).unwrap()));
            let from = format!("FROM (
              WITH matching AS (SELECT s.id FROM samples s {filter}),
              roots AS (SELECT DISTINCT coalesce(l.parent_id,m.id) id FROM matching m
                LEFT JOIN sample_stems l ON l.child_id=m.id),
              visible AS (SELECT id,id root_id,-1 position FROM roots UNION ALL
                SELECT l.child_id,l.parent_id,l.position FROM sample_stems l
                JOIN roots r ON r.id=l.parent_id
                WHERE l.parent_id IN (SELECT value FROM json_each(?)))
              SELECT * FROM visible
            ) v JOIN samples s ON s.id=v.id JOIN samples parent ON parent.id=v.root_id");
            (from,args,format!("ORDER BY {}, v.position, s.name COLLATE FONICCA_NATURAL, s.path, s.id", sort_order(f, "parent")))
        }
        None => (format!("FROM samples s {filter}"), args, format!("ORDER BY {}", sort_order(f, "s"))),
    }
}

#[cfg(test)]
pub fn count(conn: &Connection, f: &Filter) -> Result<i64> {
    count_ranked(conn, f, None)
}

pub fn count_ranked(conn: &Connection, f: &Filter, ranking: Option<&Ranking>) -> Result<i64> {
    let (from, args, _) = source(f, ranking);
    Ok(conn.query_row(&format!("SELECT COUNT(*) {from}"), params_from_iter(args_iter(&args)), |r| r.get(0))?)
}

#[cfg(test)]
pub fn list(conn: &Connection, f: &Filter, offset: i64, limit: i64) -> Result<Vec<Sample>> {
    list_ranked(conn, f, None, offset, limit)
}

pub fn list_ranked(
    conn: &Connection,
    f: &Filter,
    ranking: Option<&Ranking>,
    offset: i64,
    limit: i64,
) -> Result<Vec<Sample>> {
    let (from, mut args, order) = source(f, ranking);
    args.push(Box::new(limit));
    args.push(Box::new(offset));
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT s.id, s.source_id, s.path, s.name, s.ext, s.size, s.duration, s.sample_rate,
                s.channels, s.bit_depth, s.favorite,
                (SELECT json_group_array(json_object('id', t.id, 'name', t.name)
                                         ORDER BY t.name COLLATE NOCASE)
                 FROM sample_tags st JOIN tags t ON t.id = st.tag_id
                 WHERE st.sample_id = s.id),
                s.kind, s.bpm, s.bpm_source, s.musical_key, s.key_source, s.peak_db, s.lufs,
                s.analysis_version IS NOT NULL,
                s.category, s.category_source, s.category_confidence,
                (SELECT parent_id FROM sample_stems WHERE child_id=s.id),
                (SELECT kind FROM sample_stems WHERE child_id=s.id),
                (SELECT count(*) FROM sample_stems WHERE parent_id=s.id)
         {from} {order} LIMIT ? OFFSET ?"
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
                kind: r.get(12)?,
                bpm: r.get(13)?,
                bpm_source: r.get(14)?,
                key: r.get(15)?,
                key_source: r.get(16)?,
                peak_db: r.get(17)?,
                lufs: r.get(18)?,
                analyzed: r.get(19)?,
                category: r.get(20)?,
                category_source: r.get(21)?,
                category_confidence: r.get(22)?,
                similarity: None,
                stem_parent_id: r.get(23)?,
                stem_kind: r.get(24)?,
                stem_count: r.get(25)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut rows = rows;
    if let Some(r) = ranking {
        for s in &mut rows {
            s.similarity = r.scores.get(&s.id).copied();
        }
    }
    Ok(rows)
}

#[cfg(test)]
pub fn ids(conn: &Connection, f: &Filter) -> Result<Vec<i64>> {
    ids_ranked(conn, f, None)
}

/// Ids of all matching samples in list order (for select-all and range selection).
#[cfg(test)]
pub fn ids_ranked(conn: &Connection, f: &Filter, ranking: Option<&Ranking>) -> Result<Vec<i64>> {
    ids_range_ranked(conn, f, ranking, 0, None)
}

/// A shift selection needs IDs in visible order, not full row pages or every ID.
pub fn ids_range_ranked(conn: &Connection, f: &Filter, ranking: Option<&Ranking>, offset: i64, limit: Option<i64>) -> Result<Vec<i64>> {
    let (from, mut args, order) = source(f, ranking);
    args.push(Box::new(limit.map_or(-1, |n| n.max(0))));
    args.push(Box::new(offset.max(0)));
    let mut stmt = conn.prepare(&format!("SELECT s.id {from} {order} LIMIT ? OFFSET ?"))?;
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

const EXPORT_FORMAT: &str = "fonicca-user-data";
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
        return Err(Error::Other("not a Fonicca export or made by a newer version".into()));
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

    #[test]
    fn numeric_names_column_orders_and_pages_share_the_same_order() {
        let conn = setup();
        conn.execute("DELETE FROM samples", []).unwrap();
        for (id, name, bpm, duration) in [(1, "Track 10", Some(100), Some(12.0)),
            (2, "track 2", Some(90), Some(3.0)), (3, "Track 1", None, None),
            (4, "Track 02", Some(100), Some(2.0))] {
            conn.execute("INSERT INTO samples(id,source_id,path,name,ext,size,mtime,bpm,duration) VALUES(?1,1,?2,?3,'wav',0,0,?4,?5)",
                params![id,format!("/lib/{name}.wav"),name,bpm,duration]).unwrap();
        }
        let mut f = Filter::default();
        assert_eq!(ids(&conn,&f).unwrap(), vec![3,2,4,1]);
        f.sort_direction = SortDirection::Desc;
        assert_eq!(ids(&conn,&f).unwrap(), vec![1,4,2,3]);
        f.sort_by = SortColumn::Bpm;
        for (direction, expected) in [(SortDirection::Asc, vec![2,4,1,3]), (SortDirection::Desc, vec![4,1,2,3])] {
            f.sort_direction = direction;
            assert_eq!(ids(&conn,&f).unwrap(), expected);
            let paged: Vec<_> = (0..4).map(|offset| list(&conn,&f,offset,1).unwrap()[0].id).collect();
            assert_eq!(paged, expected);
        }
        f.sort_by = SortColumn::Length;
        f.sort_direction = SortDirection::Asc;
        assert_eq!(ids(&conn,&f).unwrap(), vec![4,2,1,3]);
        // Similarity remains relevance-ranked even if a column preference exists.
        let ranking = Ranking { ids: vec![1,3,2], scores: Default::default() };
        assert_eq!(ids_ranked(&conn,&f,Some(&ranking)).unwrap(), vec![1,3,2]);
        assert_eq!(ids_range_ranked(&conn,&f,Some(&ranking),1,Some(2)).unwrap(),vec![3,2]);
        assert!(ids_range_ranked(&conn,&f,None,0,Some(0)).unwrap().is_empty());
        assert!(ids_range_ranked(&conn,&f,None,100,Some(1)).unwrap().is_empty());
    }

    #[test]
    fn sorting_roots_keeps_children_below_their_parent_in_both_directions() {
        let conn = setup();
        conn.execute("UPDATE samples SET bpm=id*10",[]).unwrap();
        for (id,kind,position) in [(10,"vocals",0),(11,"drums",1)] {
            conn.execute("INSERT INTO samples(id,source_id,path,name,ext,size,mtime,bpm) VALUES(?1,1,?2,?3,'wav',0,0,1)",
                params![id,format!("/lib/{kind}.wav"),kind]).unwrap();
            conn.execute("INSERT INTO sample_stems(child_id,parent_id,kind,position) VALUES(?1,1,?2,?3)",params![id,kind,position]).unwrap();
        }
        let mut f = Filter {group_stems:true, expanded_stems:vec![1], sort_by:SortColumn::Bpm, ..Default::default()};
        assert_eq!(ids(&conn,&f).unwrap(), vec![1,10,11,2,3,4]);
        f.sort_direction=SortDirection::Desc;
        assert_eq!(ids(&conn,&f).unwrap(), vec![4,3,2,1,10,11]);
        assert_eq!(list(&conn,&f,4,1).unwrap()[0].id,10);
        // Every metadata column, including correlated tag sorting, works for groups.
        let t=ensure_tag(&conn,"Tag 10").unwrap();
        conn.execute("INSERT INTO sample_tags VALUES(1,?1)",[t.id]).unwrap();
        for column in [SortColumn::Name,SortColumn::Tags,SortColumn::Category,SortColumn::Kind,SortColumn::Key,SortColumn::Length,SortColumn::Type] {
            f.sort_by=column;
            let visible=ids(&conn,&f).unwrap();
            let i=visible.iter().position(|id| *id==1).unwrap();
            assert_eq!(&visible[i..i+3], &[1,10,11]);
        }
    }

    #[test]
    fn collapsed_roots_match_the_expanded_query_without_visible_children() {
        let conn = setup();
        conn.execute_batch("INSERT INTO sources VALUES(2,'/other',0);
            INSERT INTO samples(id,source_id,path,name,ext,size,mtime,category,kind,bpm,favorite)
            VALUES(10,1,'/lib/Song.wav','Song 10','wav',0,0,'guitar','long',90,0),
                  (11,2,'/other/vocals.wav','Vocals 2','wav',0,0,'vocal','loop',120,1),
                  (12,2,'/other/drums.wav','Drums 1','wav',0,0,'drums','oneshot',100,0),
                  (13,1,'/lib/empty.wav','','wav',0,0,NULL,NULL,NULL,0);
            INSERT INTO sample_stems VALUES(11,10,'vocals',0),(12,11,'drums',0);
            INSERT INTO tags(id,name) VALUES(1,'special');
            INSERT INTO sample_tags VALUES(11,1);
            INSERT INTO collections(id,name,created_at) VALUES(1,'Selection',0);
            INSERT INTO collection_items VALUES(1,12,0);").unwrap();
        let cases = [Filter::default(),
            Filter { query: Some("vocals".into()), ..Default::default() },
            Filter { category: Some("vocal".into()), ..Default::default() },
            Filter { favorites: true, ..Default::default() },
            Filter { tag_id: Some(1), ..Default::default() },
            Filter { collection_id: Some(1), ..Default::default() },
            Filter { source_id: Some(2), directory: Some("/other".into()), ..Default::default() },
            Filter { kind: Some("loop".into()), bpm_min: Some(110.0), ..Default::default() }];
        for case in cases {
            for by in [SortColumn::Name, SortColumn::Tags, SortColumn::Category, SortColumn::Kind,
                SortColumn::Bpm, SortColumn::Key, SortColumn::Length, SortColumn::Type] {
                for direction in [SortDirection::Asc, SortDirection::Desc] {
                    let closed = Filter { group_stems: true, sort_by: by, sort_direction: direction, ..case.clone() };
                    // The existing expanded CTE remains the reference. No group
                    // has id -1, so its child union contributes no rows.
                    let reference = Filter { expanded_stems: vec![-1], ..closed.clone() };
                    let expected = ids(&conn, &reference).unwrap();
                    assert_eq!(ids(&conn, &closed).unwrap(), expected);
                    assert_eq!(count(&conn, &closed).unwrap(), expected.len() as i64);
                    let paged: Vec<_> = (0..expected.len()).map(|offset| {
                        let selected = ids_range_ranked(&conn, &closed, None, offset as i64, Some(1)).unwrap();
                        assert_eq!(selected.len(), 1);
                        let row = list(&conn, &closed, offset as i64, 1).unwrap()[0].id;
                        assert_eq!(selected[0], row);
                        row
                    }).collect();
                    assert_eq!(paged, expected);
                }
            }
        }
    }

    #[test]
    fn stem_groups_paginate_filter_and_select_in_visible_order() {
        let conn = setup();
        for (id, kind, position) in [(10, "vocals", 0), (11, "drums", 1)] {
            conn.execute("INSERT INTO samples(id,source_id,path,name,ext,size,mtime,favorite) VALUES(?1,1,?2,?3,'wav',0,0,1)",
                params![id,format!("/elsewhere/Kick Hard - {kind}.wav"),kind]).unwrap();
            conn.execute("INSERT INTO sample_stems(child_id,parent_id,kind,position) VALUES(?1,1,?2,?3)",params![id,kind,position]).unwrap();
        }
        let mut filter = Filter { group_stems: true, ..Default::default() };
        assert_eq!(count(&conn, &filter).unwrap(), 4);
        let parent = list(&conn, &filter, 0, 20).unwrap().into_iter().find(|s| s.id==1).unwrap();
        assert_eq!(parent.stem_count, 2);
        filter.expanded_stems = vec![1];
        let visible = ids(&conn, &filter).unwrap();
        let index = visible.iter().position(|id| *id==1).unwrap();
        assert_eq!(&visible[index..index+3], &[1,10,11]);
        assert_eq!(count(&conn, &filter).unwrap(), 6);
        let page = list(&conn, &filter, index as i64+1, 1).unwrap();
        assert_eq!((page[0].id,page[0].stem_parent_id,page[0].stem_kind.as_deref()),(10,Some(1),Some("vocals")));
        assert_eq!(ids_range_ranked(&conn, &filter, None, index as i64, Some(3)).unwrap(), vec![1,10,11]);
        filter.query = Some("vocals".into());
        assert_eq!(ids(&conn, &filter).unwrap(), vec![1,10,11]);
        filter.expanded_stems.clear();
        assert_eq!(ids(&conn, &filter).unwrap(), vec![1]);
        filter.query = None; filter.favorites = true;
        assert_eq!(ids(&conn, &filter).unwrap(), vec![1]);
        conn.execute("DELETE FROM samples WHERE id=1",[]).unwrap();
        assert_eq!(ids(&conn, &filter).unwrap(), vec![11,10]);
    }

    fn search(conn: &Connection, q: &str) -> Vec<i64> {
        ids(conn, &Filter { query: Some(q.into()), ..Default::default() }).unwrap()
    }

    #[test]
    fn folder_tree_counts_descendants_and_omits_the_source_root() {
        let conn = setup();
        let folders = source_folders(&conn, 1).unwrap();
        let entries: Vec<(&str, i64)> = folders.iter().map(|f| (f.path.as_str(), f.sample_count)).collect();
        assert_eq!(entries, vec![
            ("/lib/Ambient", 1), ("/lib/Café Sounds", 1),
            ("/lib/Trap Pack", 2), ("/lib/Trap Pack/Drums", 2),
        ]);
        assert!(source_folders(&conn, 999).unwrap().is_empty());
        conn.execute("DELETE FROM samples WHERE id = 1", []).unwrap();
        assert_eq!(source_folders(&conn, 1).unwrap().last().unwrap().sample_count, 1);
    }

    #[test]
    fn directory_filters_use_literal_component_boundaries_and_combine_with_search() {
        let conn = setup();
        for (id, path) in [(5, "/lib/Trap Pack-old/Other.wav"), (6, "/lib/100%_Pack/Hit.wav"), (7, "/lib/100xxPack/Hit.wav")] {
            conn.execute(
                "INSERT INTO samples (id, source_id, path, name, ext, size, mtime) VALUES (?, 1, ?, 'Hit', 'wav', 0, 0)",
                params![id, path],
            ).unwrap();
        }
        let mut filter = Filter { source_id: Some(1), directory: Some("/lib/Trap Pack".into()), ..Default::default() };
        assert_eq!(ids(&conn, &filter).unwrap(), vec![1, 2]);
        assert_eq!(count(&conn, &filter).unwrap(), 2);
        filter.query = Some("snare".into());
        assert_eq!(ids(&conn, &filter).unwrap(), vec![2]);
        filter.query = None;
        filter.directory = Some("/lib/100%_Pack/".into());
        assert_eq!(ids(&conn, &filter).unwrap(), vec![6]);
        filter.directory = Some("/lib/Café Sounds".into());
        assert_eq!(ids(&conn, &filter).unwrap(), vec![4]);
        filter.source_id = Some(999);
        assert!(ids(&conn, &filter).unwrap().is_empty());
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
    fn similarity_ranking_orders_rows_and_combines_with_filters() {
        let conn = setup();
        conn.execute("UPDATE samples SET kind = 'oneshot'", []).unwrap();
        conn.execute("UPDATE samples SET kind = 'loop' WHERE id = 2", []).unwrap();
        let ranking = Ranking {
            ids: vec![3, 1, 2],
            scores: [(3, 1.0), (1, 0.8), (2, 0.5)].into_iter().collect(),
        };
        let all = Filter::default();
        assert_eq!(ids_ranked(&conn, &all, Some(&ranking)).unwrap(), vec![3, 1, 2]);
        assert_eq!(count_ranked(&conn, &all, Some(&ranking)).unwrap(), 3);
        let rows = list_ranked(&conn, &all, Some(&ranking), 1, 10).unwrap();
        assert_eq!(rows.iter().map(|s| (s.id, s.similarity)).collect::<Vec<_>>(), vec![(1, Some(0.8)), (2, Some(0.5))]);
        let oneshots = Filter { kind: Some("oneshot".into()), ..Default::default() };
        assert_eq!(ids_ranked(&conn, &oneshots, Some(&ranking)).unwrap(), vec![3, 1]);
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
