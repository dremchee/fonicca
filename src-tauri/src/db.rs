use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;

/// Each entry upgrades the schema by one version (tracked in `PRAGMA user_version`).
pub const MIGRATIONS: &[&str] = &[r#"
    CREATE TABLE sources (
        id       INTEGER PRIMARY KEY,
        path     TEXT NOT NULL UNIQUE,
        added_at INTEGER NOT NULL
    );

    CREATE TABLE samples (
        id          INTEGER PRIMARY KEY,
        source_id   INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
        path        TEXT NOT NULL UNIQUE,
        name        TEXT NOT NULL,
        ext         TEXT NOT NULL,
        size        INTEGER NOT NULL,
        mtime       INTEGER NOT NULL,
        duration    REAL,
        sample_rate INTEGER,
        channels    INTEGER,
        bit_depth   INTEGER,
        peaks       BLOB
    );

    CREATE INDEX samples_source ON samples(source_id);
    CREATE INDEX samples_name ON samples(name COLLATE NOCASE);
"#, r#"
    ALTER TABLE samples ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;
    CREATE INDEX samples_favorite ON samples(favorite) WHERE favorite = 1;

    CREATE TABLE tags (
        id   INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE COLLATE NOCASE
    );

    CREATE TABLE sample_tags (
        sample_id INTEGER NOT NULL REFERENCES samples(id) ON DELETE CASCADE,
        tag_id    INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (sample_id, tag_id)
    ) WITHOUT ROWID;
    CREATE INDEX sample_tags_tag ON sample_tags(tag_id);

    -- Search index: rowid = samples.id. `folder` holds the directories between the
    -- source root and the file, so a pack or folder name is searchable too.
    CREATE VIRTUAL TABLE samples_fts USING fts5(
        name, folder, tags,
        tokenize = "unicode61 remove_diacritics 2"
    );

    CREATE VIEW samples_fts_source AS
    SELECT x.id,
           x.name,
           replace(rtrim(rel, replace(rel, '/', '')), '/', ' ') AS folder,
           coalesce((SELECT group_concat(t.name, ' ') FROM sample_tags st
                     JOIN tags t ON t.id = st.tag_id WHERE st.sample_id = x.id), '') AS tags
    FROM (SELECT s.id, s.name, substr(s.path, length(src.path) + 2) AS rel
          FROM samples s JOIN sources src ON src.id = s.source_id) x;

    INSERT INTO samples_fts (rowid, name, folder, tags)
    SELECT id, name, folder, tags FROM samples_fts_source;

    CREATE TRIGGER samples_fts_insert AFTER INSERT ON samples BEGIN
        INSERT INTO samples_fts (rowid, name, folder, tags)
        SELECT id, name, folder, tags FROM samples_fts_source WHERE id = new.id;
    END;
    CREATE TRIGGER samples_fts_update AFTER UPDATE OF name, path, source_id ON samples BEGIN
        DELETE FROM samples_fts WHERE rowid = new.id;
        INSERT INTO samples_fts (rowid, name, folder, tags)
        SELECT id, name, folder, tags FROM samples_fts_source WHERE id = new.id;
    END;
    CREATE TRIGGER samples_fts_delete AFTER DELETE ON samples BEGIN
        DELETE FROM samples_fts WHERE rowid = old.id;
    END;

    CREATE TRIGGER sample_tags_fts_insert AFTER INSERT ON sample_tags BEGIN
        UPDATE samples_fts SET tags = (SELECT tags FROM samples_fts_source WHERE id = new.sample_id)
        WHERE rowid = new.sample_id;
    END;
    CREATE TRIGGER sample_tags_fts_delete AFTER DELETE ON sample_tags BEGIN
        UPDATE samples_fts SET tags = (SELECT tags FROM samples_fts_source WHERE id = old.sample_id)
        WHERE rowid = old.sample_id;
    END;
    CREATE TRIGGER tags_fts_rename AFTER UPDATE OF name ON tags BEGIN
        UPDATE samples_fts SET tags = (SELECT f.tags FROM samples_fts_source f WHERE f.id = samples_fts.rowid)
        WHERE rowid IN (SELECT sample_id FROM sample_tags WHERE tag_id = new.id);
    END;
"#, r#"
    -- Content fingerprint (size + head/tail hash) used to recognise moved or renamed files.
    ALTER TABLE samples ADD COLUMN fingerprint INTEGER;
    CREATE INDEX samples_fingerprint ON samples(fingerprint);

    -- User data of removed samples, waiting for the same file to reappear elsewhere.
    CREATE TABLE detached (
        fingerprint INTEGER NOT NULL,
        size        INTEGER NOT NULL,
        favorite    INTEGER NOT NULL,
        tag_ids     TEXT NOT NULL,
        detached_at INTEGER NOT NULL
    );
    CREATE INDEX detached_key ON detached(fingerprint, size);

    -- Runs before the FK cascade removes the sample's tags.
    CREATE TRIGGER samples_detach BEFORE DELETE ON samples
    WHEN old.fingerprint IS NOT NULL
     AND (old.favorite = 1 OR EXISTS (SELECT 1 FROM sample_tags WHERE sample_id = old.id))
    BEGIN
        INSERT INTO detached (fingerprint, size, favorite, tag_ids, detached_at)
        VALUES (old.fingerprint, old.size, old.favorite,
                (SELECT json_group_array(tag_id) FROM sample_tags WHERE sample_id = old.id),
                unixepoch());
    END;

    CREATE TRIGGER samples_reattach AFTER INSERT ON samples
    WHEN new.fingerprint IS NOT NULL
     AND EXISTS (SELECT 1 FROM detached WHERE fingerprint = new.fingerprint AND size = new.size)
    BEGIN
        UPDATE samples SET favorite = (
            SELECT favorite FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
            ORDER BY detached_at DESC, rowid DESC LIMIT 1)
        WHERE id = new.id;
        INSERT OR IGNORE INTO sample_tags (sample_id, tag_id)
        SELECT new.id, j.value FROM detached d, json_each(d.tag_ids) j
        WHERE d.rowid = (SELECT rowid FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
                         ORDER BY detached_at DESC, rowid DESC LIMIT 1)
          AND j.value IN (SELECT id FROM tags);
        DELETE FROM detached WHERE rowid = (
            SELECT rowid FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
            ORDER BY detached_at DESC, rowid DESC LIMIT 1);
    END;
"#, r#"
    CREATE TABLE collections (
        id         INTEGER PRIMARY KEY,
        name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
        created_at INTEGER NOT NULL
    );

    CREATE TABLE collection_items (
        collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
        sample_id     INTEGER NOT NULL REFERENCES samples(id) ON DELETE CASCADE,
        added_at      INTEGER NOT NULL,
        PRIMARY KEY (collection_id, sample_id)
    ) WITHOUT ROWID;
    CREATE INDEX collection_items_sample ON collection_items(sample_id);

    -- Collections membership follows moved files just like tags.
    ALTER TABLE detached ADD COLUMN collection_ids TEXT NOT NULL DEFAULT '[]';

    DROP TRIGGER samples_detach;
    CREATE TRIGGER samples_detach BEFORE DELETE ON samples
    WHEN old.fingerprint IS NOT NULL
     AND (old.favorite = 1
          OR EXISTS (SELECT 1 FROM sample_tags WHERE sample_id = old.id)
          OR EXISTS (SELECT 1 FROM collection_items WHERE sample_id = old.id))
    BEGIN
        INSERT INTO detached (fingerprint, size, favorite, tag_ids, collection_ids, detached_at)
        VALUES (old.fingerprint, old.size, old.favorite,
                (SELECT json_group_array(tag_id) FROM sample_tags WHERE sample_id = old.id),
                (SELECT json_group_array(collection_id) FROM collection_items WHERE sample_id = old.id),
                unixepoch());
    END;

    DROP TRIGGER samples_reattach;
    CREATE TRIGGER samples_reattach AFTER INSERT ON samples
    WHEN new.fingerprint IS NOT NULL
     AND EXISTS (SELECT 1 FROM detached WHERE fingerprint = new.fingerprint AND size = new.size)
    BEGIN
        UPDATE samples SET favorite = (SELECT favorite FROM detached WHERE rowid = (SELECT rowid FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
                       ORDER BY detached_at DESC, rowid DESC LIMIT 1))
        WHERE id = new.id;
        INSERT OR IGNORE INTO sample_tags (sample_id, tag_id)
        SELECT new.id, j.value FROM detached d, json_each(d.tag_ids) j
        WHERE d.rowid = (SELECT rowid FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
                       ORDER BY detached_at DESC, rowid DESC LIMIT 1)
          AND j.value IN (SELECT id FROM tags);
        INSERT OR IGNORE INTO collection_items (collection_id, sample_id, added_at)
        SELECT j.value, new.id, unixepoch() FROM detached d, json_each(d.collection_ids) j
        WHERE d.rowid = (SELECT rowid FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
                       ORDER BY detached_at DESC, rowid DESC LIMIT 1)
          AND j.value IN (SELECT id FROM collections);
        DELETE FROM detached WHERE rowid = (SELECT rowid FROM detached WHERE fingerprint = new.fingerprint AND size = new.size
                       ORDER BY detached_at DESC, rowid DESC LIMIT 1);
    END;
"#, r#"
    -- Results of audio analysis; analysis_version is NULL until analyzed and reset
    -- when the file content changes.
    ALTER TABLE samples ADD COLUMN kind TEXT;
    ALTER TABLE samples ADD COLUMN bpm REAL;
    ALTER TABLE samples ADD COLUMN bpm_source TEXT;
    ALTER TABLE samples ADD COLUMN musical_key TEXT;
    ALTER TABLE samples ADD COLUMN key_source TEXT;
    ALTER TABLE samples ADD COLUMN peak_db REAL;
    ALTER TABLE samples ADD COLUMN lufs REAL;
    ALTER TABLE samples ADD COLUMN analysis_version INTEGER;
    CREATE INDEX samples_analysis_version ON samples(analysis_version);
    CREATE INDEX samples_kind ON samples(kind);
    CREATE INDEX samples_bpm ON samples(bpm);
    CREATE INDEX samples_key ON samples(musical_key);
"#, r#"
    -- Instrument category: from file/folder names, or from the CLAP embedding.
    ALTER TABLE samples ADD COLUMN category TEXT;
    ALTER TABLE samples ADD COLUMN category_source TEXT;
    ALTER TABLE samples ADD COLUMN category_confidence REAL;
    -- Version of the name rules applied; NULL until categorized by name.
    ALTER TABLE samples ADD COLUMN category_version INTEGER;
    ALTER TABLE samples ADD COLUMN embedding BLOB;
    ALTER TABLE samples ADD COLUMN embedding_version INTEGER;
    CREATE INDEX samples_category ON samples(category);
    CREATE INDEX samples_category_version ON samples(category_version);
    CREATE INDEX samples_embedding_version ON samples(embedding_version);
"#, r#"
    -- Full-content hash (xxh3-128) confirming a duplicate that the head/tail fingerprint
    -- suggests; NULL until checked and reset when the file changes.
    ALTER TABLE samples ADD COLUMN content_hash BLOB;

    -- Files every scan skips until they are unhidden. Their tags wait in `detached`.
    CREATE TABLE hidden (
        path      TEXT PRIMARY KEY,
        source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
        hidden_at INTEGER NOT NULL
    );

    -- Folders whose copies are kept when resolving duplicates; the newest wins.
    CREATE TABLE preferred_folders (
        path     TEXT PRIMARY KEY,
        added_at INTEGER NOT NULL
    );
"#];

/// How long user data of removed files is kept for possible reattachment.
const DETACHED_TTL_DAYS: i64 = 90;

pub fn prune_detached(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM detached WHERE detached_at < unixepoch() - ?1 * 86400",
        [DETACHED_TTL_DAYS],
    )?;
    Ok(())
}

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(conn)
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
