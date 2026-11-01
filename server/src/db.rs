use anyhow::Result;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Db {
    path: PathBuf,
}

impl Db {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn open(&self) -> Result<Connection> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(&self.path)?;
        conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(conn)
    }

    pub fn init(&self) -> Result<()> {
        self.open()?.execute_batch(SCHEMA)?;
        Ok(())
    }
}

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tracks (
  id           INTEGER PRIMARY KEY,
  path         TEXT NOT NULL UNIQUE,
  size         INTEGER NOT NULL,
  mtime        INTEGER NOT NULL,
  sha          TEXT,
  title        TEXT NOT NULL,
  artist       TEXT,
  album        TEXT,
  album_artist TEXT,
  track_no     INTEGER,
  disc_no      INTEGER,
  year         INTEGER,
  duration_ms  INTEGER,
  codec        TEXT,
  bitrate      INTEGER,
  sample_rate  INTEGER,
  bit_depth    INTEGER,
  channels     INTEGER,
  rev          INTEGER NOT NULL,
  added_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS tracks_rev    ON tracks(rev);
CREATE INDEX IF NOT EXISTS tracks_artist ON tracks(album_artist, album, disc_no, track_no);

CREATE TABLE IF NOT EXISTS tombstones (
  id       INTEGER PRIMARY KEY,
  track_id INTEGER NOT NULL,
  rev      INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
  title, artist, album,
  content='tracks', content_rowid='id',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
  INSERT INTO tracks_fts(rowid, title, artist, album)
  VALUES (new.id, new.title, new.artist, new.album);
END;

CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
  INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album)
  VALUES ('delete', old.id, old.title, old.artist, old.album);
END;

CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
  INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album)
  VALUES ('delete', old.id, old.title, old.artist, old.album);
  INSERT INTO tracks_fts(rowid, title, artist, album)
  VALUES (new.id, new.title, new.artist, new.album);
END;
"#;

pub fn rev(conn: &Connection) -> Result<i64> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key='rev'", [], |r| r.get(0))
        .ok();
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

pub fn bump_rev(conn: &Connection) -> Result<i64> {
    let next = rev(conn)? + 1;
    conn.execute(
        "INSERT INTO meta(key, value) VALUES('rev', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [next.to_string()],
    )?;
    Ok(next)
}
