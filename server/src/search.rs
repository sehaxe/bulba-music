use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

/// Search is deliberately not FTS-only: people type fragments, misspellings and
/// "artist album" in either order, and expect the album to come back too. A
/// ranked substring scan over a personal library stays under a few milliseconds
/// at 50k tracks and behaves the way a search box is expected to.
///
/// Placeholder order follows the SQL text: the ranking CASE is in the SELECT
/// list, so its values bind before the WHERE clause values.
#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub tracks: Vec<TrackHit>,
    pub albums: Vec<AlbumHit>,
    pub artists: Vec<ArtistHit>,
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct TrackHit {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: i64,
    pub has_cover: i64,
    pub year: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bits: Option<i64>,
    pub sample_rate: Option<i64>,
    pub score: i64,
}

#[derive(Debug, Serialize)]
pub struct AlbumHit {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub has_cover: i64,
    pub tracks: i64,
}

#[derive(Debug, Serialize)]
pub struct ArtistHit {
    pub id: i64,
    pub name: String,
    pub albums: i64,
    pub tracks: i64,
}

/// Lowercased tokens, deduplicated, in the order typed.
pub fn tokens(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in query.split_whitespace() {
        let t = raw
            .trim_matches(|c: char| c.is_ascii_punctuation() && c != '\'')
            .to_lowercase();
        if t.is_empty() || out.contains(&t) {
            continue;
        }
        out.push(t);
    }
    out.truncate(6);
    out
}

pub fn search(conn: &Connection, query: &str, limit: i64) -> Result<SearchResults> {
    let tokens = tokens(query);
    if tokens.is_empty() {
        return Ok(SearchResults {
            tracks: vec![],
            albums: vec![],
            artists: vec![],
            query: query.to_string(),
        });
    }
    let full = query.trim().to_lowercase();
    let lower: Vec<String> = tokens.clone();

    // ---- tracks ---------------------------------------------------------
    let where_clause = tokens
        .iter()
        .map(|_| "(lower(t.title) LIKE ? OR lower(t.artist) LIKE ? OR lower(t.album) LIKE ?)")
        .collect::<Vec<_>>()
        .join(" AND ");

    let sql = format!(
        "SELECT t.id, t.title, t.artist, t.album, t.album_id, t.has_cover, t.year,
                t.duration_ms, t.codec, t.bits, t.sample_rate,
                CASE
                  WHEN lower(t.title)  = ?                    THEN 0
                  WHEN lower(t.title)  LIKE ? || '%'          THEN 1
                  WHEN lower(t.title)  LIKE '%' || ? || '%'   THEN 2
                  WHEN lower(t.artist) = ?                    THEN 3
                  WHEN lower(t.artist) LIKE ? || '%'          THEN 4
                  WHEN lower(t.artist) LIKE '%' || ? || '%'   THEN 5
                  WHEN lower(t.album)  LIKE ? || '%'          THEN 6
                  ELSE 7
                END AS score
         FROM v_track t
         WHERE {where_clause}
         ORDER BY score, lower(t.title)
         LIMIT ?"
    );

    let mut params: Vec<String> = Vec::new();
    for _ in 0..7 {
        params.push(full.clone());
    }
    for t in &lower {
        let p = format!("%{t}%");
        params.push(p.clone());
        params.push(p.clone());
        params.push(p);
    }
    params.push(limit.to_string());

    let mut tracks = Vec::new();
    {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok(TrackHit {
                id: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                album_id: r.get(4)?,
                has_cover: r.get(5)?,
                year: r.get(6)?,
                duration_ms: r.get(7)?,
                codec: r.get(8)?,
                bits: r.get(9)?,
                sample_rate: r.get(10)?,
                score: r.get(11)?,
            })
        })?;
        for row in rows {
            tracks.push(row?);
        }
    }

    // ---- albums ---------------------------------------------------------
    let mut albums = Vec::new();
    {
        let where_clause = tokens
            .iter()
            .map(|_| "(lower(al.title) LIKE ? OR lower(ar.name) LIKE ?)")
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "SELECT al.id, al.title, ar.name, al.year,
                    CASE WHEN al.cover_path IS NULL THEN 0 ELSE 1 END,
                    (SELECT count(*) FROM track t WHERE t.album_id = al.id)
             FROM album al JOIN artist ar ON ar.id = al.artist_id
             WHERE {where_clause}
             ORDER BY (lower(ar.name) LIKE ?) DESC, lower(al.title)
             LIMIT ?"
        );
        let mut params: Vec<String> = Vec::new();
        for t in &lower {
            let p = format!("%{t}%");
            params.push(p.clone());
            params.push(p);
        }
        params.push(format!("%{}%", lower[0]));
        params.push(limit.to_string());

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok(AlbumHit {
                id: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                year: r.get(3)?,
                has_cover: r.get(4)?,
                tracks: r.get(5)?,
            })
        })?;
        for row in rows {
            albums.push(row?);
        }
    }

    // ---- artists --------------------------------------------------------
    let mut artists = Vec::new();
    {
        let where_clause = tokens
            .iter()
            .map(|_| "lower(ar.name) LIKE ?")
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = format!(
            "SELECT ar.id, ar.name,
                    (SELECT count(*) FROM album al WHERE al.artist_id = ar.id),
                    (SELECT count(*) FROM track t JOIN album al ON al.id = t.album_id
                      WHERE al.artist_id = ar.id)
             FROM artist ar
             WHERE {where_clause}
             ORDER BY lower(ar.name) LIMIT ?"
        );
        let mut params: Vec<String> = lower.iter().map(|t| format!("%{t}%")).collect();
        params.push(limit.to_string());

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok(ArtistHit {
                id: r.get(0)?,
                name: r.get(1)?,
                albums: r.get(2)?,
                tracks: r.get(3)?,
            })
        })?;
        for row in rows {
            artists.push(row?);
        }
    }

    Ok(SearchResults {
        tracks,
        albums,
        artists,
        query: query.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn fixture() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::new(dir.path().join("s.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        conn.execute("INSERT INTO artist(name,norm) VALUES('Tame Impala','tame impala')", [])
            .unwrap();
        let artist = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO album(title,artist_id,year,norm) VALUES('Currents',?1,2015,'tame impala/currents')",
            [artist],
        )
        .unwrap();
        let album = conn.last_insert_rowid();
        for (title, no) in [("Let It Happen", 1), ("The Less I Know The Better", 2)] {
            conn.execute(
                "INSERT INTO track(title,album_id,disc,track_no,duration_ms,rev,created_at)
                 VALUES(?1,?2,1,?3,200000,1,0)",
                rusqlite::params![title, album, no],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO track(title,album_id,disc,track_no,duration_ms,rev,created_at)
             VALUES('Xtal',?1,1,3,200000,1,0)",
            [album],
        )
        .unwrap();
        (dir, conn)
    }

    #[test]
    fn tokens_lowercase_and_dedupe() {
        assert_eq!(tokens("Tame  Impala tame"), vec!["tame", "impala"]);
        assert_eq!(tokens("   "), Vec::<String>::new());
        assert_eq!(tokens("Jane!"), vec!["jane"]);
    }

    #[test]
    fn tokens_are_capped() {
        assert_eq!(tokens("a b c d e f g h i j").len(), 6);
    }

    #[test]
    fn partial_word_finds_track() {
        let (_d, conn) = fixture();
        let r = search(&conn, "happ", 20).unwrap();
        assert_eq!(r.tracks.len(), 1);
        assert_eq!(r.tracks[0].title, "Let It Happen");
    }

    #[test]
    fn artist_query_returns_their_tracks_and_album() {
        let (_d, conn) = fixture();
        let r = search(&conn, "tame", 20).unwrap();
        assert_eq!(r.tracks.len(), 3);
        assert_eq!(r.albums.len(), 1);
        assert_eq!(r.artists.len(), 1);
        assert_eq!(r.artists[0].tracks, 3);
    }

    #[test]
    fn exact_title_ranks_first() {
        let (_d, conn) = fixture();
        let r = search(&conn, "xtal", 20).unwrap();
        assert_eq!(r.tracks[0].title, "Xtal");
        assert_eq!(r.tracks[0].score, 0);
    }

    #[test]
    fn multiword_matches_across_fields() {
        let (_d, conn) = fixture();
        let r = search(&conn, "tame currents", 20).unwrap();
        assert!(!r.tracks.is_empty());
    }

    #[test]
    fn nonsense_returns_nothing() {
        let (_d, conn) = fixture();
        let r = search(&conn, "zzzzz", 20).unwrap();
        assert!(r.tracks.is_empty() && r.albums.is_empty() && r.artists.is_empty());
    }
}
