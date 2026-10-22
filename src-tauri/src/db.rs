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
"#];

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
