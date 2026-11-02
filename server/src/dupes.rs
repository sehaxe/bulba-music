//! Finding and merging the same artist or album that arrived twice.
//!
//! A library assembled from downloads always has "Nirvana", "Nirvana (US)" and
//! "NIRVANA" as three artists, and the same record as "Currents" and "Currents
//! (Deluxe)". The recommendation engine cannot see that they are one thing, so
//! the radio plays Nirvana twice in a row and the artist page shows a third of
//! the catalogue.
//!
//! Matching is deliberately conservative: names are folded to lowercase ASCII
//! letters and digits, and a handful of release-noise suffixes are dropped. Two
//! different spellings that fold to the same key are the same artist; anything
//! else needs a human, and the merge endpoint takes explicit ids.

use anyhow::Result;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, serde::Serialize)]
pub struct DupGroup {
    pub kind: String,
    /// The survivor: the member with the most tracks, so merging never shrinks
    /// what the listener sees by accident.
    pub keep: i64,
    pub members: Vec<DupMember>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DupMember {
    pub id: i64,
    pub name: String,
    pub tracks: i64,
}

/// Words and decorations that say nothing about *which* artist it is.
const NOISE: &[&str] = &[
    "the",
    "us",
    "uk",
    "usa",
    "official",
    "band",
    "deluxe",
    "remaster",
    "remastered",
    "expanded",
    "edition",
    "bonus",
    "ep",
    "lp",
];

/// Lowercase ASCII letters and digits only, minus release noise.
pub fn fold(name: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let mut flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            if !NOISE.contains(&word.as_str()) {
                out.push_str(word);
            }
            word.clear();
        }
    };
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            word.push(ch.to_ascii_lowercase());
        } else if ch.is_alphanumeric() {
            // Keep non-ASCII letters (Кино, 木村) rather than dropping them.
            for c in ch.to_lowercase() {
                word.push(c);
            }
        } else {
            flush(&mut word, &mut out);
        }
    }
    flush(&mut word, &mut out);
    out
}

pub fn find(conn: &Connection) -> Result<Vec<DupGroup>> {
    let mut groups = Vec::new();

    // Artists: same folded name, different rows.
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, (SELECT count(*) FROM track t
             JOIN album al ON al.id = t.album_id
             WHERE al.artist_id = a.id) AS n
         FROM artist a",
    )?;
    let rows: Vec<(i64, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    let mut by_key: std::collections::BTreeMap<String, Vec<DupMember>> =
        std::collections::BTreeMap::new();
    for (id, name, tracks) in rows {
        by_key.entry(fold(&name)).or_default().push(DupMember {
            id,
            name,
            tracks,
        });
    }
    for (_, mut members) in by_key {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|m| -m.tracks);
        groups.push(DupGroup {
            kind: "artist".into(),
            keep: members[0].id,
            members,
        });
    }

    // Albums: same folded title *and* same artist row, so two different artists
    // with a record called "Live" are not offered to the listener.
    let mut stmt = conn.prepare(
        "SELECT al.id, al.title, al.artist_id, (SELECT count(*) FROM track t
             WHERE t.album_id = al.id)
         FROM album al",
    )?;
    let rows: Vec<(i64, String, i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();
    let mut by_key: std::collections::BTreeMap<String, Vec<DupMember>> =
        std::collections::BTreeMap::new();
    for (id, title, artist, tracks) in rows {
        by_key
            .entry(format!("{artist}|{}", fold(&title)))
            .or_default()
            .push(DupMember {
                id,
                name: title,
                tracks,
            });
    }
    for (_, mut members) in by_key {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|m| -m.tracks);
        groups.push(DupGroup {
            kind: "album".into(),
            keep: members[0].id,
            members,
        });
    }

    Ok(groups)
}

/// Moves everything from `drop` onto `keep` and deletes the empty row.
/// Returns false when the ids do not exist or are the same.
pub fn merge(conn: &Connection, kind: &str, keep: i64, loser: i64) -> Result<bool> {
    if keep == loser {
        return Ok(false);
    }
    let exists = |table: &str, id: i64| -> Result<bool> {
        let n: i64 = conn.query_row(
            &format!("SELECT count(*) FROM {table} WHERE id=?1"),
            [id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    };
    if !exists("artist", keep)? && kind == "artist" {
        return Ok(false);
    }
    if !exists("album", keep)? && kind == "album" {
        return Ok(false);
    }

    let tx = conn.unchecked_transaction()?;
    match kind {
        "artist" => {
            let keep_name: String =
                tx.query_row("SELECT name FROM artist WHERE id=?1", [keep], |r| {
                    r.get(0)
                })?;
            // Album identities belong to the artist: moving the albums is enough,
            // tracks follow their album.
            tx.execute(
                "UPDATE album SET artist_id=?1 WHERE artist_id=?2",
                params![keep, loser],
            )?;
            // The album `norm` is unique per row, and two albums of the same
            // title can now sit under one artist. Re-key the duplicates by
            // including the album id, which keeps them distinct without
            // inventing a different title.
            let mut stmt = tx.prepare(
                "SELECT id FROM album WHERE artist_id=?1",
            )?;
            let ids: Vec<i64> = stmt
                .query_map([keep], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);
            let mut seen = std::collections::HashSet::new();
            for id in ids {
                let title: String =
                    tx.query_row("SELECT title FROM album WHERE id=?1", [id], |r| r.get(0))?;
                let key = format!("{}#{}", fold(&title), id);
                if !seen.insert(format!("{}", fold(&title))) {
                    tx.execute("UPDATE album SET norm=?1 WHERE id=?2", params![key, id])?;
                }
            }
            // Keep the nicest spelling of the name.
            let drop_name: String =
                tx.query_row("SELECT name FROM artist WHERE id=?1", [loser], |r| {
                    r.get(0)
                })?;
            if drop_name.chars().count() > keep_name.chars().count()
                && drop_name.to_lowercase() == keep_name.to_lowercase()
            {
                tx.execute(
                    "UPDATE artist SET name=?1 WHERE id=?2",
                    params![drop_name, keep],
                )?;
            }
            tx.execute("DELETE FROM artist WHERE id=?1", [loser])?;
            tx.execute("DELETE FROM artist_like WHERE artist_id=?1", [loser])?;
        }
        "album" => {
            let keep_cover: Option<String> =
                tx.query_row("SELECT cover_path FROM album WHERE id=?1", [keep], |r| {
                    r.get(0)
                })?;
            if keep_cover.is_none() {
                let cover: Option<String> =
                    tx.query_row("SELECT cover_path FROM album WHERE id=?1", [loser], |r| {
                        r.get(0)
                    })?;
                if cover.is_some() {
                    tx.execute(
                        "UPDATE album SET cover_path=?1 WHERE id=?2",
                        params![cover, keep],
                    )?;
                }
            }
            // A track that exists on both records is the same recording; the
            // unique index is (album, disc, track_no), so re-inserting without
            // care would collide. Keep the surviving copy and loser the other.
            let mut stmt = tx.prepare(
                "SELECT t.id, t.title, t.disc, t.track_no FROM track t
                 WHERE t.album_id=?1",
            )?;
            let moving: Vec<(i64, String, Option<i64>, Option<i64>)> = stmt
                .query_map([loser], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);
            for (id, title, disc, track_no) in moving {
                let twin: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM track WHERE album_id=?1
                         AND coalesce(disc,-1)=coalesce(?2,-1)
                         AND coalesce(track_no,-1)=coalesce(?3,-1)
                         AND lower(title)=lower(?4)",
                        params![keep, disc, track_no, title],
                        |r| r.get(0),
                    )
                    .ok();
                match twin {
                    Some(_) => {
                        // Same recording twice: the safer move is to leave the
                        // duplicate on the album being dropped and delete it with
                        // the album, not to guess which copy is better.
                    }
                    None => {
                        tx.execute(
                            "UPDATE track SET album_id=?1 WHERE id=?2",
                            params![keep, id],
                        )?;
                    }
                }
            }
            // Anything still pointing at the dropped album is a duplicate that
            // was deliberately left behind.
            let leftovers: i64 = tx.query_row(
                "SELECT count(*) FROM track WHERE album_id=?1",
                [loser],
                |r| r.get(0),
            )?;
            if leftovers > 0 {
                tx.execute(
                    "DELETE FROM track_master WHERE track_id IN
                       (SELECT id FROM track WHERE album_id=?1)",
                    [loser],
                )?;
                tx.execute("DELETE FROM embed WHERE track_id IN
                       (SELECT id FROM track WHERE album_id=?1)", [loser])?;
                tx.execute("DELETE FROM track WHERE album_id=?1", [loser])?;
            }
            tx.execute("DELETE FROM album_like WHERE album_id=?1", [loser])?;
            tx.execute("DELETE FROM album WHERE id=?1", [loser])?;
        }
        _ => return Ok(false),
    }
    tx.commit()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::new(dir.path().join("d.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        (dir, conn)
    }

    #[test]
    fn folding_hides_case_and_release_noise() {
        assert_eq!(fold("Nirvana (US)"), fold("NIRVANA"));
        assert_eq!(fold("The Beatles"), fold("beatles"));
        assert_eq!(fold("Currents (Deluxe Edition)"), fold("currents"));
        assert_ne!(fold("Nirvana"), fold("Nirvana UK Tribute"));
    }

    #[test]
    fn two_spellings_of_one_artist_are_offered_once() {
        let (_dir, conn) = db();
        conn.execute_batch(
            "INSERT INTO artist(id,name,norm) VALUES(1,'Nirvana','a1'),(2,'Nirvana (US)','a2');
             INSERT INTO album(id,title,artist_id,norm) VALUES(1,'Nevermind',1,'b1'),
                                                            (2,'In Utero',2,'b2');
             INSERT INTO track(id,title,album_id,rev,created_at) VALUES(1,'Breed',1,1,0),
                                                                     (2,'Heart-Shaped Box',2,1,0);",
        )
        .unwrap();
        let groups = find(&conn).unwrap();
        let artist = groups.iter().find(|g| g.kind == "artist").unwrap();
        assert_eq!(artist.members.len(), 2);
        assert_eq!(artist.keep, 1, "the survivor should own the most tracks");
    }

    #[test]
    fn merging_artists_moves_albums_and_keeps_tracks() {
        let (_dir, conn) = db();
        conn.execute_batch(
            "INSERT INTO artist(id,name,norm) VALUES(1,'Nirvana','a1'),(2,'Nirvana (US)','a2');
             INSERT INTO album(id,title,artist_id,norm) VALUES(1,'Nevermind',1,'b1'),
                                                            (2,'In Utero',2,'b2');
             INSERT INTO track(id,title,album_id,rev,created_at) VALUES(1,'Breed',1,1,0),
                                                                     (2,'Heart-Shaped Box',2,1,0);",
        )
        .unwrap();
        assert!(merge(&conn, "artist", 1, 2).unwrap());
        let left: i64 = conn
            .query_row("SELECT count(*) FROM artist", [], |r| r.get(0))
            .unwrap();
        let tracks: i64 = conn
            .query_row(
                "SELECT count(*) FROM track t JOIN album a ON a.id=t.album_id
                 WHERE a.artist_id=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(left, 1);
        assert_eq!(tracks, 2, "no track may be lost in the merge");
        assert!(find(&conn).unwrap().iter().all(|g| g.kind != "artist"));
    }

    #[test]
    fn merging_albums_keeps_the_cover_and_does_not_duplicate_a_track() {
        let (_dir, conn) = db();
        conn.execute_batch(
            "INSERT INTO artist(id,name,norm) VALUES(1,'Tame Impala','a1');
             INSERT INTO album(id,title,artist_id,norm,cover_path) VALUES(1,'Currents',1,'b1',NULL),
                                                                     (2,'Currents (Deluxe)',1,'b2','/c.jpg');
             INSERT INTO track(id,title,album_id,disc,track_no,rev,created_at)
               VALUES(1,'Let It Happen',1,1,1,1,0),
                     (2,'Let It Happen',2,1,1,1,0),
                     (3,'Nangs',2,1,2,1,0);",
        )
        .unwrap();
        assert!(merge(&conn, "album", 1, 2).unwrap());
        let tracks: i64 = conn
            .query_row("SELECT count(*) FROM track WHERE album_id=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        let cover: Option<String> = conn
            .query_row("SELECT cover_path FROM album WHERE id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cover.as_deref(), Some("/c.jpg"), "the cover must survive");
        assert_eq!(tracks, 2, "the duplicate recording stays out, Nangs moves");
    }
}
