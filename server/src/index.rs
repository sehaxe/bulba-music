use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

/// The identity pages. An album is more than a folder and an artist is more
/// than a tag group: both need covers, counts, their own likes and — for the
/// artist — what the listener actually plays most.
#[derive(Debug, Serialize)]
pub struct ArtistSummary {
    pub id: i64,
    pub name: String,
    pub albums: i64,
    pub tracks: i64,
    pub plays: i64,
    pub cover_album: Option<i64>,
    pub has_cover: i64,
    pub liked: i64,
}

#[derive(Debug, Serialize)]
pub struct ArtistPage {
    pub artist: ArtistSummary,
    pub albums: Vec<AlbumSummary>,
    pub top_tracks: Vec<TopTrack>,
    pub first_year: Option<i64>,
    pub last_year: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AlbumSummary {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub artist_id: i64,
    pub year: Option<i64>,
    pub tracks: i64,
    pub plays: i64,
    pub has_cover: i64,
    pub liked: i64,
    pub disc_total: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AlbumPage {
    pub album: AlbumSummary,
    pub tracks: Vec<AlbumTrack>,
    pub total_ms: i64,
    pub plays: i64,
}

#[derive(Debug, Serialize)]
pub struct AlbumTrack {
    pub id: i64,
    pub title: String,
    pub disc: Option<i64>,
    pub track_no: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bits: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bitrate: Option<i64>,
    pub plays: i64,
    pub liked: i64,
}

#[derive(Debug, Serialize)]
pub struct TopTrack {
    pub id: i64,
    pub title: String,
    pub album_id: i64,
    pub album: String,
    pub has_cover: i64,
    pub plays: i64,
    pub duration_ms: Option<i64>,
}

const ARTIST_SQL: &str = "
    SELECT ar.id, ar.name,
           (SELECT count(*) FROM album a WHERE a.artist_id = ar.id),
           (SELECT count(*) FROM track t JOIN album a ON a.id = t.album_id
             WHERE a.artist_id = ar.id),
           coalesce((SELECT count(*) FROM play_event pe
                     JOIN track t2 ON t2.id = pe.track_id
                     JOIN album a2 ON a2.id = t2.album_id
                     WHERE a2.artist_id = ar.id AND pe.event=0), 0),
           (SELECT a3.id FROM album a3 WHERE a3.artist_id = ar.id
             AND a3.cover_path IS NOT NULL ORDER BY a3.year LIMIT 1),
           exists(SELECT 1 FROM artist_like l WHERE l.artist_id = ar.id)
    FROM artist ar
";

fn artist_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ArtistSummary> {
    Ok(ArtistSummary {
        id: r.get(0)?,
        name: r.get(1)?,
        albums: r.get(2)?,
        tracks: r.get(3)?,
        plays: r.get(4)?,
        cover_album: r.get(5)?,
        has_cover: if r.get::<_, Option<i64>>(5)?.is_some() { 1 } else { 0 },
        liked: r.get(6)?,
    })
}

pub fn artists(conn: &Connection, limit: i64) -> Result<Vec<ArtistSummary>> {
    let mut stmt = conn.prepare(&format!("{ARTIST_SQL} ORDER BY ar.name COLLATE NOCASE LIMIT ?1"))?;
    let rows = stmt.query_map([limit], artist_from_row)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn artist(conn: &Connection, id: i64) -> Result<Option<ArtistPage>> {
    let artist = conn
        .query_row(&format!("{ARTIST_SQL} WHERE ar.id = ?1"), [id], artist_from_row)
        .ok();
    let Some(artist) = artist else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT a.id, a.title, ar.name, a.artist_id, a.year,
                (SELECT count(*) FROM track t WHERE t.album_id = a.id),
                coalesce((SELECT count(*) FROM play_event pe
                          JOIN track t ON t.id = pe.track_id
                          WHERE t.album_id = a.id AND pe.event=0), 0),
                CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END,
                exists(SELECT 1 FROM album_like l WHERE l.album_id = a.id),
                a.disc_total
         FROM album a JOIN artist ar ON ar.id = a.artist_id
         WHERE a.artist_id = ?1
         ORDER BY coalesce(a.year, 9999), a.title",
    )?;
    let albums: Vec<AlbumSummary> = stmt
        .query_map([id], |r| {
            Ok(AlbumSummary {
                id: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                artist_id: r.get(3)?,
                year: r.get(4)?,
                tracks: r.get(5)?,
                plays: r.get(6)?,
                has_cover: r.get(7)?,
                liked: r.get(8)?,
                disc_total: r.get(9)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT t.id, t.title, a.id, a.title,
                CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END,
                count(*),
                t.duration_ms
         FROM play_event pe
         JOIN track t ON t.id = pe.track_id
         JOIN album a ON a.id = t.album_id
         WHERE a.artist_id = ?1 AND pe.event=0
         GROUP BY t.id ORDER BY count(*) DESC, t.title LIMIT 10",
    )?;
    let top_tracks: Vec<TopTrack> = stmt
        .query_map([id], |r| {
            Ok(TopTrack {
                id: r.get(0)?,
                title: r.get(1)?,
                album_id: r.get(2)?,
                album: r.get(3)?,
                has_cover: r.get(4)?,
                plays: r.get(5)?,
                duration_ms: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Nobody has played anything yet on a fresh library; showing an empty
    // "most played" block would look broken, so the last three added tracks
    // stand in until real history exists.
    let top_tracks = if top_tracks.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT t.id, t.title, a.id, a.title,
                    CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END,
                    0, t.duration_ms
             FROM track t JOIN album a ON a.id = t.album_id
             WHERE a.artist_id = ?1
             ORDER BY t.created_at DESC, t.id DESC LIMIT 3",
        )?;
        let rows = stmt.query_map([id], |r| {
            Ok(TopTrack {
                id: r.get(0)?,
                title: r.get(1)?,
                album_id: r.get(2)?,
                album: r.get(3)?,
                has_cover: r.get(4)?,
                plays: r.get(5)?,
                duration_ms: r.get(6)?,
            })
        })?;
        rows.filter_map(|r| r.ok()).collect()
    } else {
        top_tracks
    };

    let (first_year, last_year): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT min(year), max(year) FROM album WHERE artist_id=?1 AND year IS NOT NULL",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((None, None));

    Ok(Some(ArtistPage {
        artist,
        albums,
        top_tracks,
        first_year,
        last_year,
    }))
}

pub fn albums(conn: &Connection, limit: i64) -> Result<Vec<AlbumSummary>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.title, ar.name, a.artist_id, a.year,
                (SELECT count(*) FROM track t WHERE t.album_id = a.id),
                coalesce((SELECT count(*) FROM play_event pe
                          JOIN track t ON t.id = pe.track_id
                          WHERE t.album_id = a.id AND pe.event=0), 0),
                CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END,
                exists(SELECT 1 FROM album_like l WHERE l.album_id = a.id),
                a.disc_total
         FROM album a JOIN artist ar ON ar.id = a.artist_id
         ORDER BY ar.name COLLATE NOCASE, coalesce(a.year, 9999), a.title
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(AlbumSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            artist: r.get(2)?,
            artist_id: r.get(3)?,
            year: r.get(4)?,
            tracks: r.get(5)?,
            plays: r.get(6)?,
            has_cover: r.get(7)?,
            liked: r.get(8)?,
            disc_total: r.get(9)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn album(conn: &Connection, id: i64) -> Result<Option<AlbumPage>> {
    let album = conn
        .query_row(
            "SELECT a.id, a.title, ar.name, a.artist_id, a.year,
                    (SELECT count(*) FROM track t WHERE t.album_id = a.id),
                    coalesce((SELECT count(*) FROM play_event pe
                              JOIN track t ON t.id = pe.track_id
                              WHERE t.album_id = a.id AND pe.event=0), 0),
                    CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END,
                    exists(SELECT 1 FROM album_like l WHERE l.album_id = a.id),
                    a.disc_total
             FROM album a JOIN artist ar ON ar.id = a.artist_id WHERE a.id = ?1",
            [id],
            |r| {
                Ok(AlbumSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    artist: r.get(2)?,
                    artist_id: r.get(3)?,
                    year: r.get(4)?,
                    tracks: r.get(5)?,
                    plays: r.get(6)?,
                    has_cover: r.get(7)?,
                    liked: r.get(8)?,
                    disc_total: r.get(9)?,
                })
            },
        )
        .ok();
    let Some(album) = album else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT t.id, t.title, t.disc, t.track_no, t.duration_ms,
                (SELECT m.codec FROM track_master tm JOIN master m ON m.id=tm.master_id
                  WHERE tm.track_id=t.id ORDER BY (m.codec='flac') DESC, m.bytes DESC LIMIT 1),
                (SELECT m.bits FROM track_master tm JOIN master m ON m.id=tm.master_id
                  WHERE tm.track_id=t.id ORDER BY (m.codec='flac') DESC, m.bytes DESC LIMIT 1),
                (SELECT m.sample_rate FROM track_master tm JOIN master m ON m.id=tm.master_id
                  WHERE tm.track_id=t.id ORDER BY (m.codec='flac') DESC, m.bytes DESC LIMIT 1),
                (SELECT m.bitrate FROM track_master tm JOIN master m ON m.id=tm.master_id
                  WHERE tm.track_id=t.id ORDER BY (m.codec='flac') DESC, m.bytes DESC LIMIT 1),
                coalesce((SELECT count(*) FROM play_event pe
                          WHERE pe.track_id=t.id AND pe.event=0), 0),
                exists(SELECT 1 FROM track_like l WHERE l.track_id=t.id)
         FROM track t WHERE t.album_id = ?1
         ORDER BY coalesce(t.disc,1), coalesce(t.track_no, 999), t.title",
    )?;
    let rows = stmt.query_map([id], |r| {
        Ok(AlbumTrack {
            id: r.get(0)?,
            title: r.get(1)?,
            disc: r.get(2)?,
            track_no: r.get(3)?,
            duration_ms: r.get(4)?,
            codec: r.get(5)?,
            bits: r.get(6)?,
            sample_rate: r.get(7)?,
            bitrate: r.get(8)?,
            plays: r.get(9)?,
            liked: r.get(10)?,
        })
    })?;
    let tracks: Vec<AlbumTrack> = rows.filter_map(|r| r.ok()).collect();
    let total_ms = tracks
        .iter()
        .map(|t| t.duration_ms.unwrap_or(0))
        .sum::<i64>();
    let plays = tracks.iter().map(|t| t.plays).sum::<i64>();

    Ok(Some(AlbumPage {
        album,
        tracks,
        total_ms,
        plays,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn fixture() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::new(dir.path().join("i.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        conn.execute("INSERT INTO artist(name,norm) VALUES('Tame Impala','tame impala')", [])
            .unwrap();
        let artist = conn.last_insert_rowid();
        // Deliberately inserted out of order: pages sort by year.
        for (title, year) in [("Currents", 2015), ("The Slow Rush", 2020)] {
            conn.execute(
                "INSERT INTO album(title,artist_id,year,norm) VALUES(?1,?2,?3,?4)",
                rusqlite::params![title, artist, year, format!("tame impala/{title}")],
            )
            .unwrap();
            let album = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO track(title,album_id,disc,track_no,duration_ms,rev,created_at)
                 VALUES(?1,?2,1,1,180000,1,0)",
                rusqlite::params![format!("{title} opener"), album],
            )
            .unwrap();
        }
        (dir, conn)
    }

    #[test]
    fn artists_are_listed_with_counts() {
        let (_d, conn) = fixture();
        let list = artists(&conn, 50).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].albums, 2);
        assert_eq!(list[0].tracks, 2);
    }

    #[test]
    fn artist_page_orders_albums_by_year() {
        let (_d, conn) = fixture();
        let id = artists(&conn, 1).unwrap()[0].id;
        let page = artist(&conn, id).unwrap().unwrap();
        assert_eq!(page.albums[0].title, "Currents");
        assert_eq!(page.albums[1].title, "The Slow Rush");
        assert_eq!(page.first_year, Some(2015));
        assert_eq!(page.last_year, Some(2020));
    }

    #[test]
    fn album_page_returns_tracks_in_order_with_totals() {
        let (_d, conn) = fixture();
        let albums_list = albums(&conn, 10).unwrap();
        let page = album(&conn, albums_list[0].id).unwrap().unwrap();
        assert_eq!(page.tracks.len(), 1);
        assert_eq!(page.total_ms, 180_000);
    }

    #[test]
    fn missing_ids_return_none_not_an_error() {
        let (_d, conn) = fixture();
        assert!(album(&conn, 999).unwrap().is_none());
        assert!(artist(&conn, 999).unwrap().is_none());
    }

    #[test]
    fn likes_are_reflected_in_pages() {
        let (_d, conn) = fixture();
        let user = crate::db::default_user(&conn).unwrap();
        let id = artists(&conn, 1).unwrap()[0].id;
        conn.execute(
            "INSERT INTO artist_like(user_id,artist_id,ts) VALUES(?1,?2,0)",
            rusqlite::params![user, id],
        )
        .unwrap();
        let page = artist(&conn, id).unwrap().unwrap();
        assert_eq!(page.artist.liked, 1);
    }
}
