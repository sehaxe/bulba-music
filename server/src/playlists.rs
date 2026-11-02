use crate::api::TRACK_COLUMNS;
use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::api::Track;

/// A playlist the user made. `is_public` is what another device in the same
/// account can see: local ones exist only for their owner, public ones are the
/// ones worth sharing over a link.
#[derive(Debug, Serialize)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_public: bool,
    pub kind: String,
    pub tracks: i64,
    pub duration_ms: i64,
    pub created_at: i64,
    pub rev: i64,
    pub owner: String,
    pub mine: bool,
    pub liked: bool,
    pub cover_album: Option<i64>,
    pub has_cover: bool,
}

#[derive(Debug, Serialize)]
pub struct PlaylistPage {
    pub playlist: Playlist,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Deserialize)]
pub struct NewPlaylist {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_public: bool,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_public: Option<bool>,
}

fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

fn row_to_playlist(r: &rusqlite::Row<'_>, me: i64) -> rusqlite::Result<Playlist> {
    let owner_id: i64 = r.get("user_id")?;
    Ok(Playlist {
        id: r.get("id")?,
        name: r.get("name")?,
        description: r.get("description")?,
        is_public: r.get::<_, i64>("is_public")? == 1,
        kind: r.get("kind")?,
        tracks: r.get("tracks")?,
        duration_ms: r.get::<_, Option<i64>>("duration_ms")?.unwrap_or(0),
        created_at: r.get("created_at")?,
        rev: r.get("rev")?,
        owner: r.get("owner")?,
        mine: owner_id == me,
        liked: r.get::<_, i64>("liked")? == 1,
        cover_album: r.get("cover_album")?,
        has_cover: r.get::<_, i64>("has_cover")? == 1,
    })
}

const SELECT: &str = "
    SELECT p.id, p.user_id, p.name, p.description, p.is_public, p.kind,
           p.created_at, p.rev,
           u.name AS owner,
           (SELECT count(*) FROM playlist_track pt
              JOIN track t ON t.id = pt.track_id WHERE pt.playlist_id = p.id) AS tracks,
           (SELECT sum(t.duration_ms) FROM playlist_track pt
              JOIN track t ON t.id = pt.track_id WHERE pt.playlist_id = p.id) AS duration_ms,
           exists(SELECT 1 FROM playlist_like l WHERE l.playlist_id = p.id AND l.user_id = ?1) AS liked,
           (SELECT t.album_id FROM playlist_track pt JOIN track t ON t.id = pt.track_id
              WHERE pt.playlist_id = p.id ORDER BY pt.position LIMIT 1) AS cover_album,
           coalesce((SELECT CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END
              FROM playlist_track pt JOIN track t ON t.id = pt.track_id
              JOIN album a ON a.id = t.album_id
              WHERE pt.playlist_id = p.id ORDER BY pt.position LIMIT 1), 0) AS has_cover
    FROM playlist p JOIN user u ON u.id = p.user_id
";

/// Own playlists plus every public playlist anybody made — a household server
/// has one listener, but the rule is the same with many.
pub fn list(conn: &Connection, me: i64) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} WHERE p.user_id = ?1 OR p.is_public = 1
         ORDER BY p.is_public DESC, p.created_at DESC"
    ))?;
    let rows = stmt.query_map([me], |r| row_to_playlist(r, me))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Playlists this user liked, for the liked screen.
pub fn liked(conn: &Connection, me: i64) -> Result<Vec<Playlist>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT} JOIN playlist_like pl ON pl.playlist_id = p.id AND pl.user_id = ?1
         ORDER BY pl.ts DESC"
    ))?;
    let rows = stmt.query_map([me], |r| row_to_playlist(r, me))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, me: i64, id: i64) -> Result<Option<PlaylistPage>> {
    let playlist = conn
        .query_row(&format!("{SELECT} WHERE p.id = ?2"), params![me, id], |r| {
            row_to_playlist(r, me)
        })
        .ok();
    let Some(playlist) = playlist else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {TRACK_COLUMNS} FROM v_track t
         JOIN playlist_track pt ON pt.track_id = t.id
         WHERE pt.playlist_id = ?1 ORDER BY pt.position"
    ))?;
    let tracks: Vec<Track> = stmt
        .query_map([id], crate::api::map_track)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(Some(PlaylistPage { playlist, tracks }))
}

pub fn create(conn: &Connection, me: i64, new: &NewPlaylist) -> Result<i64> {
    let name = new.name.trim();
    let name = if name.is_empty() { "новый плейлист" } else { name };
    conn.execute(
        "INSERT INTO playlist(user_id, name, norm, description, is_public, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, unixepoch())",
        params![me, name, normalize(name), new.description, new.is_public as i64],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, me: i64, id: i64, patch: &PlaylistPatch) -> Result<bool> {
    let owned: i64 = conn.query_row(
        "SELECT count(*) FROM playlist WHERE id=?1 AND user_id=?2",
        params![id, me],
        |r| r.get(0),
    )?;
    if owned == 0 {
        return Ok(false);
    }
    if let Some(name) = &patch.name {
        conn.execute(
            "UPDATE playlist SET name=?1, norm=?2, rev=rev+1 WHERE id=?3",
            params![name.trim(), normalize(name), id],
        )?;
    }
    if let Some(desc) = &patch.description {
        conn.execute(
            "UPDATE playlist SET description=?1, rev=rev+1 WHERE id=?2",
            params![desc, id],
        )?;
    }
    if let Some(public) = patch.is_public {
        conn.execute(
            "UPDATE playlist SET is_public=?1, rev=rev+1 WHERE id=?2",
            params![public as i64, id],
        )?;
    }
    Ok(true)
}

pub fn delete(conn: &Connection, me: i64, id: i64) -> Result<bool> {
    let n = conn.execute(
        "DELETE FROM playlist WHERE id=?1 AND user_id=?2",
        params![id, me],
    )?;
    Ok(n > 0)
}

/// Appends at the end, or at `position` when given. Re-adding a track moves it
/// instead of failing, which is what a person expects from a drag.
pub fn add_track(conn: &Connection, me: i64, id: i64, track_id: i64, position: Option<i64>) -> Result<bool> {
    let owns: i64 = conn.query_row(
        "SELECT count(*) FROM playlist WHERE id=?1 AND user_id=?2",
        params![id, me],
        |r| r.get(0),
    )?;
    if owns == 0 {
        return Ok(false);
    }
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM track WHERE id=?1",
        [track_id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(false);
    }
    let last: i64 = conn
        .query_row(
            "SELECT coalesce(max(position), -1) FROM playlist_track WHERE playlist_id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap_or(-1);
    let pos = position.unwrap_or(last + 1);
    conn.execute(
        "INSERT INTO playlist_track(playlist_id, track_id, position, added_at)
         VALUES(?1, ?2, ?3, unixepoch())
         ON CONFLICT(playlist_id, track_id) DO UPDATE SET position=excluded.position",
        params![id, track_id, pos],
    )?;
    conn.execute("UPDATE playlist SET rev=rev+1 WHERE id=?1", [id])?;
    Ok(true)
}

pub fn remove_track(conn: &Connection, me: i64, id: i64, track_id: i64) -> Result<bool> {
    let owns: i64 = conn.query_row(
        "SELECT count(*) FROM playlist WHERE id=?1 AND user_id=?2",
        params![id, me],
        |r| r.get(0),
    )?;
    if owns == 0 {
        return Ok(false);
    }
    conn.execute(
        "DELETE FROM playlist_track WHERE playlist_id=?1 AND track_id=?2",
        params![id, track_id],
    )?;
    conn.execute("UPDATE playlist SET rev=rev+1 WHERE id=?1", [id])?;
    Ok(true)
}

/// Rewrites the whole order. Drag and drop produces the full list anyway, and a
/// per-pair swap would need a second endpoint for every direction.
pub fn reorder(conn: &Connection, me: i64, id: i64, track_ids: &[i64]) -> Result<bool> {
    let owns: i64 = conn.query_row(
        "SELECT count(*) FROM playlist WHERE id=?1 AND user_id=?2",
        params![id, me],
        |r| r.get(0),
    )?;
    if owns == 0 {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction()?;
    let mut stmt = tx.prepare(
        "UPDATE playlist_track SET position=?1
         WHERE playlist_id=?2 AND track_id=?3",
    )?;
    for (i, track_id) in track_ids.iter().enumerate() {
        stmt.execute(params![i as i64, id, track_id])?;
    }
    drop(stmt);
    tx.execute("UPDATE playlist SET rev=rev+1 WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(true)
}

pub fn set_like(conn: &Connection, me: i64, id: i64, on: bool) -> Result<bool> {
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM playlist WHERE id=?1 AND (user_id=?2 OR is_public=1)",
        params![id, me],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Ok(false);
    }
    if on {
        conn.execute(
            "INSERT OR REPLACE INTO playlist_like(user_id, playlist_id, ts) VALUES(?1,?2,unixepoch())",
            params![me, id],
        )?;
    } else {
        conn.execute(
            "DELETE FROM playlist_like WHERE user_id=?1 AND playlist_id=?2",
            params![me, id],
        )?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn setup() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::new(dir.path().join("p.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        conn.execute("INSERT INTO artist(name,norm) VALUES('A','a')", []).unwrap();
        let artist = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO album(title,artist_id,norm) VALUES('Al',?1,'a/al')",
            [artist],
        )
        .unwrap();
        let album = conn.last_insert_rowid();
        for title in ["one", "two", "three"] {
            conn.execute(
                "INSERT INTO track(title,album_id,duration_ms,rev,created_at)
                 VALUES(?1,?2,180000,1,0)",
                params![title, album],
            )
            .unwrap();
        }
        (dir, conn)
    }

    #[test]
    fn create_then_add_and_read_back() {
        let (_d, conn) = setup();
        let me = crate::auth::local_user(&conn).unwrap();
        let id = create(
            &conn,
            me,
            &NewPlaylist { name: "night".into(), description: String::new(), is_public: false },
        )
        .unwrap();
        assert!(add_track(&conn, me, id, 1, None).unwrap());
        assert!(add_track(&conn, me, id, 2, None).unwrap());
        let page = get(&conn, me, id).unwrap().unwrap();
        assert_eq!(page.tracks.len(), 2);
        assert_eq!(page.tracks[0].title, "one");
        assert_eq!(page.playlist.tracks, 2);
        assert_eq!(page.playlist.duration_ms, 360_000);
    }

    #[test]
    fn re_adding_moves_instead_of_duplicating() {
        let (_d, conn) = setup();
        let me = crate::auth::local_user(&conn).unwrap();
        let id = create(&conn, me, &NewPlaylist { name: "x".into(), description: String::new(), is_public: false }).unwrap();
        add_track(&conn, me, id, 1, None).unwrap();
        add_track(&conn, me, id, 2, None).unwrap();
        add_track(&conn, me, id, 1, Some(5)).unwrap();
        let page = get(&conn, me, id).unwrap().unwrap();
        assert_eq!(page.tracks.len(), 2);
        assert_eq!(page.tracks.last().unwrap().title, "one");
    }

    #[test]
    fn local_playlists_are_invisible_to_other_users() {
        let (_d, conn) = setup();
        let me = crate::auth::local_user(&conn).unwrap();
        conn.execute(
            "INSERT INTO user(name, created_at) VALUES('other', 0)",
            [],
        )
        .unwrap();
        let other = conn.last_insert_rowid();
        let id = create(&conn, me, &NewPlaylist { name: "mine".into(), description: String::new(), is_public: false }).unwrap();
        assert!(list(&conn, other).unwrap().iter().all(|p| p.id != id));
        update(&conn, me, id, &PlaylistPatch { name: None, description: None, is_public: Some(true) }).unwrap();
        assert!(list(&conn, other).unwrap().iter().any(|p| p.id == id));
    }

    #[test]
    fn only_the_owner_can_edit() {
        let (_d, conn) = setup();
        let me = crate::auth::local_user(&conn).unwrap();
        conn.execute("INSERT INTO user(name, created_at) VALUES('other', 0)", []).unwrap();
        let other = conn.last_insert_rowid();
        let id = create(&conn, me, &NewPlaylist { name: "p".into(), description: String::new(), is_public: true }).unwrap();
        assert!(!add_track(&conn, other, id, 1, None).unwrap());
        assert!(!delete(&conn, other, id).unwrap());
        assert!(add_track(&conn, me, id, 1, None).unwrap());
    }

    #[test]
    fn likes_show_up_in_the_liked_list() {
        let (_d, conn) = setup();
        let me = crate::auth::local_user(&conn).unwrap();
        let id = create(&conn, me, &NewPlaylist { name: "p".into(), description: String::new(), is_public: true }).unwrap();
        assert!(set_like(&conn, me, id, true).unwrap());
        let liked_list = liked(&conn, me).unwrap();
        assert_eq!(liked_list.len(), 1);
        assert!(liked_list[0].liked);
        set_like(&conn, me, id, false).unwrap();
        assert!(liked(&conn, me).unwrap().is_empty());
    }
}
