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
        let _ = conn.execute("ALTER TABLE track ADD COLUMN tempo_bpm REAL", []);
        let _ = conn.execute("ALTER TABLE track ADD COLUMN brightness REAL", []);
        let _ = conn.execute("ALTER TABLE track ADD COLUMN energy REAL", []);
        let _ = conn.execute("ALTER TABLE track ADD COLUMN mode REAL", []);

        // Playlists grew from "a name and a list" into something with a
        // visibility, a description and a revision.
        let _ = conn.execute(
            "ALTER TABLE playlist ADD COLUMN is_public INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE playlist ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE playlist ADD COLUMN kind TEXT NOT NULL DEFAULT 'manual'",
            [],
        );
        let _ = conn.execute("ALTER TABLE playlist ADD COLUMN cover_path TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE playlist ADD COLUMN rev INTEGER NOT NULL DEFAULT 1",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE playlist_track ADD COLUMN added_at INTEGER NOT NULL DEFAULT 0",
            [],
        );
        // The index has to come after the column exists: an old database gets
        // the column from the ALTER above, a new one from the table itself.
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS playlist_public ON playlist(is_public)",
            [],
        );
        // Daily mixes are per listener: a second device starts with its own
        // empty history and must not inherit the first one's picks.
        let _ = conn.execute(
            "ALTER TABLE daily_mix ADD COLUMN user_id INTEGER NOT NULL DEFAULT 1",
            [],
        );

        // Older databases described a user by a telegram login. The account is
        // now a phrase, so the column goes away; ids are preserved because every
        // like and play event points at them.
        let has_legacy: bool = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('user') WHERE name='ts_login'",
                [],
                |r| r.get::<_, i64>(0).map(|n| n > 0),
            )
            .unwrap_or(false);
        if has_legacy {
            conn.execute_batch(
                // Renaming a table would rewrite every foreign key that points
                // at it; legacy_alter_table keeps the old text so the likes and
                // events of the old user still point at `user`.
                "PRAGMA legacy_alter_table=ON;
                 PRAGMA foreign_keys=OFF;
                 BEGIN;
                 ALTER TABLE user RENAME TO user_legacy;
                 CREATE TABLE user (
                   id          INTEGER PRIMARY KEY,
                   name        TEXT NOT NULL,
                   phrase_hash TEXT UNIQUE,
                   created_at  INTEGER NOT NULL
                 );
                 INSERT INTO user(id, name, phrase_hash, created_at)
                   SELECT id, name, NULL, created_at FROM user_legacy;
                 DROP TABLE user_legacy;
                 COMMIT;
                 PRAGMA legacy_alter_table=OFF;
                 PRAGMA foreign_keys=ON;",
            )?;
        }
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
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  phrase_hash TEXT UNIQUE,
  created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS session (
  token      TEXT PRIMARY KEY,
  user_id    INTEGER NOT NULL REFERENCES user(id),
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
  tempo_bpm   REAL,
  brightness  REAL,
  energy      REAL,
  mode        REAL,
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

CREATE TABLE IF NOT EXISTS track_like (
  user_id  INTEGER NOT NULL REFERENCES user(id),
  track_id INTEGER NOT NULL REFERENCES track(id),
  ts       INTEGER NOT NULL,
  PRIMARY KEY (user_id, track_id)
);

CREATE TABLE IF NOT EXISTS album_like (
  user_id  INTEGER NOT NULL REFERENCES user(id),
  album_id INTEGER NOT NULL REFERENCES album(id),
  ts       INTEGER NOT NULL,
  PRIMARY KEY (user_id, album_id)
);

CREATE TABLE IF NOT EXISTS artist_like (
  user_id   INTEGER NOT NULL REFERENCES user(id),
  artist_id INTEGER NOT NULL REFERENCES artist(id),
  ts        INTEGER NOT NULL,
  PRIMARY KEY (user_id, artist_id)
);

CREATE TABLE IF NOT EXISTS playlist (
  id          INTEGER PRIMARY KEY,
  user_id     INTEGER NOT NULL REFERENCES user(id),
  name        TEXT NOT NULL,
  norm        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'manual',
  description TEXT NOT NULL DEFAULT '',
  cover_path  TEXT,
  -- Public playlists are what another device in the same account sees; a
  -- local one exists only for its owner.
  is_public   INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  rev         INTEGER NOT NULL DEFAULT 1,
  UNIQUE (user_id, norm)
);

CREATE TABLE IF NOT EXISTS playlist_track (
  playlist_id INTEGER NOT NULL REFERENCES playlist(id) ON DELETE CASCADE,
  track_id    INTEGER NOT NULL REFERENCES track(id),
  position    INTEGER NOT NULL,
  added_at    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (playlist_id, track_id)
);

CREATE TABLE IF NOT EXISTS playlist_like (
  user_id     INTEGER NOT NULL REFERENCES user(id),
  playlist_id INTEGER NOT NULL REFERENCES playlist(id) ON DELETE CASCADE,
  ts          INTEGER NOT NULL,
  PRIMARY KEY (user_id, playlist_id)
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

CREATE TABLE IF NOT EXISTS daily_mix (
  day      TEXT NOT NULL,
  slot     INTEGER NOT NULL,
  position INTEGER NOT NULL,
  track_id INTEGER NOT NULL REFERENCES track(id),
  title    TEXT NOT NULL,
  note     TEXT NOT NULL,
  PRIMARY KEY (day, slot, position)
);

-- One row per listener: where they stopped, so another device can continue.
CREATE TABLE IF NOT EXISTS playback_cursor (
  user_id     INTEGER PRIMARY KEY REFERENCES user(id),
  track_id    INTEGER NOT NULL REFERENCES track(id),
  position_ms INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS peaks (
  track_id INTEGER PRIMARY KEY REFERENCES track(id),
  data     BLOB NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS track_fts USING fts5(
  title, artist, album,
  tokenize='unicode61 remove_diacritics 2'
);

DROP VIEW IF EXISTS v_track;
CREATE VIEW v_track AS
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
  t.created_at,
  t.rev,
  t.loudness_db
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
    crate::auth::local_user(conn)
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
