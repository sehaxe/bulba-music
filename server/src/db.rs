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
        let conn = self.open()?;
        conn.execute_batch(SCHEMA)?;
        // Databases created before covers existed get the column added in place.
        let _ = conn.execute("ALTER TABLE album ADD COLUMN cover_path TEXT", []);
        Ok(())
    }
}

/// The catalogue is split the way the plan requires: a `master` is a file on disk and
/// is never mutated, a `track` is the musical unit and may have several masters.
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user (
  id         INTEGER PRIMARY KEY,
  ts_login   TEXT NOT NULL UNIQUE,
  name       TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS master (
  id          INTEGER PRIMARY KEY,
  path        TEXT NOT NULL UNIQUE,
  bytes       INTEGER NOT NULL,
  mtime       INTEGER NOT NULL,
  sha         TEXT,
  codec       TEXT NOT NULL,
  duration_ms INTEGER,
  sample_rate INTEGER,
  channels    INTEGER,
  bits        INTEGER,
  bitrate     INTEGER,
  tag_raw     TEXT,
  ingested_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS master_sha ON master(sha);

CREATE TABLE IF NOT EXISTS artist (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  norm TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS album (
  id         INTEGER PRIMARY KEY,
  title      TEXT NOT NULL,
  artist_id  INTEGER NOT NULL REFERENCES artist(id),
  year       INTEGER,
  disc_total INTEGER,
  cover_path TEXT,
  norm       TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS track (
  id          INTEGER PRIMARY KEY,
  title       TEXT NOT NULL,
  album_id    INTEGER NOT NULL REFERENCES album(id),
  disc        INTEGER,
  track_no    INTEGER,
  duration_ms INTEGER,
  loudness_db REAL,
  rev         INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS track_album ON track(album_id, disc, track_no);
CREATE INDEX IF NOT EXISTS track_rev   ON track(rev);

CREATE TABLE IF NOT EXISTS track_master (
  track_id  INTEGER NOT NULL REFERENCES track(id),
  master_id INTEGER NOT NULL REFERENCES master(id),
  role      TEXT NOT NULL,
  PRIMARY KEY (track_id, master_id)
);

CREATE TABLE IF NOT EXISTS render (
  id          INTEGER PRIMARY KEY,
  track_id    INTEGER NOT NULL REFERENCES track(id),
  quality     TEXT NOT NULL,
  path        TEXT NOT NULL UNIQUE,
  bytes       INTEGER NOT NULL,
  duration_ms INTEGER,
  created_at  INTEGER NOT NULL,
  UNIQUE (track_id, quality)
);

CREATE TABLE IF NOT EXISTS embed (
  model    TEXT NOT NULL,
  track_id INTEGER NOT NULL REFERENCES track(id),
  dim      INTEGER NOT NULL,
  vec      BLOB NOT NULL,
  PRIMARY KEY (model, track_id)
);

CREATE TABLE IF NOT EXISTS play_event (
  id             INTEGER PRIMARY KEY,
  ts             INTEGER NOT NULL,
  user_id        INTEGER NOT NULL REFERENCES user(id),
  track_id       INTEGER NOT NULL REFERENCES track(id),
  session_id     INTEGER NOT NULL,
  event          INTEGER NOT NULL,
  position_ms    INTEGER NOT NULL,
  position_ratio REAL NOT NULL,
  source         INTEGER NOT NULL,
  prev_track_id  INTEGER REFERENCES track(id),
  device         INTEGER NOT NULL,
  offline        INTEGER NOT NULL,
  quality        INTEGER NOT NULL,
  client_id      TEXT
);
CREATE INDEX IF NOT EXISTS play_event_user_ts ON play_event(user_id, ts);
CREATE INDEX IF NOT EXISTS play_event_track   ON play_event(track_id);
CREATE INDEX IF NOT EXISTS play_event_session ON play_event(session_id);

CREATE TABLE IF NOT EXISTS tombstones (
  id       INTEGER PRIMARY KEY,
  track_id INTEGER NOT NULL,
  rev      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS peaks (
  track_id INTEGER PRIMARY KEY REFERENCES track(id),
  data     BLOB NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS track_fts USING fts5(
  title, artist, album,
  tokenize='unicode61 remove_diacritics 2'
);

CREATE VIEW IF NOT EXISTS v_track AS
SELECT
  t.id, t.title,
  ar.name  AS artist,
  al.title AS album,
  al.year,
  t.disc, t.track_no, t.duration_ms,
  al.id AS album_id,
  (al.cover_path IS NOT NULL) AS has_cover,
  (SELECT m.codec FROM track_master tm JOIN master m ON m.id = tm.master_id
    WHERE tm.track_id = t.id ORDER BY (m.codec = 'flac') DESC, m.bytes DESC LIMIT 1) AS codec,
  (SELECT m.bitrate FROM track_master tm JOIN master m ON m.id = tm.master_id
    WHERE tm.track_id = t.id ORDER BY (m.codec = 'flac') DESC, m.bytes DESC LIMIT 1) AS bitrate,
  (SELECT m.sample_rate FROM track_master tm JOIN master m ON m.id = tm.master_id
    WHERE tm.track_id = t.id ORDER BY (m.codec = 'flac') DESC, m.bytes DESC LIMIT 1) AS sample_rate,
  (SELECT m.bits FROM track_master tm JOIN master m ON m.id = tm.master_id
    WHERE tm.track_id = t.id ORDER BY (m.codec = 'flac') DESC, m.bytes DESC LIMIT 1) AS bits,
  (SELECT m.channels FROM track_master tm JOIN master m ON m.id = tm.master_id
    WHERE tm.track_id = t.id ORDER BY (m.codec = 'flac') DESC, m.bytes DESC LIMIT 1) AS channels,
  (SELECT m.bytes FROM track_master tm JOIN master m ON m.id = tm.master_id
    WHERE tm.track_id = t.id ORDER BY (m.codec = 'flac') DESC, m.bytes DESC LIMIT 1) AS bytes,
  (SELECT count(*) FROM track_master tm WHERE tm.track_id = t.id) AS master_count,
  t.rev
FROM track t
JOIN album  al ON al.id = t.album_id
JOIN artist ar ON ar.id = al.artist_id;
"#;

pub fn rev(conn: &Connection) -> Result<i64> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key='rev'", [], |r| r.get(0))
        .ok();
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0))
}

pub fn default_user(conn: &Connection) -> Result<i64> {
    conn.execute(
        "INSERT INTO user(ts_login, name, created_at) VALUES('local', 'local', unixepoch())
         ON CONFLICT(ts_login) DO NOTHING",
        [],
    )?;
    Ok(conn.query_row("SELECT id FROM user WHERE ts_login='local'", [], |r| r.get(0))?)
}

/// Lowercase, collapse whitespace, strip punctuation that fragments artists in
/// real-world tags ("Aphex Twin ", "Aphex  Twin", "aphex twin").
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.extend(ch.to_lowercase());
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_collapses_noise() {
        assert_eq!(norm("  Aphex   Twin "), "aphex twin");
        assert_eq!(norm("APHEX TWIN"), norm("aphex twin"));
        assert_eq!(norm(""), "");
    }

    #[test]
    fn schema_applies_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::new(dir.path().join("t.db"));
        db.init().unwrap();
        db.init().unwrap();
        let conn = db.open().unwrap();
        let user = default_user(&conn).unwrap();
        assert!(user > 0);
        assert_eq!(rev(&conn).unwrap(), 0);
    }
}
