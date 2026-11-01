use crate::db::{norm, Db};
use anyhow::Result;
use lofty::prelude::*;
use rusqlite::{params, Transaction};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

pub const AUDIO_EXT: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "aiff", "aif", "ape", "wv", "alac",
];

/// Formats that are lossless and therefore normalised to FLAC on import.
pub const LOSSLESS_EXT: &[&str] = &["flac", "wav", "aiff", "aif", "ape", "wv", "alac"];

/// Values that appear in the wild as an "artist" but carry no artist.
const JUNK_ARTISTS: &[&str] = &[
    "music engine", "lavf", "lavc", "unknown", "unknown artist", "various", "various artists",
];

#[derive(Debug, Default, Serialize)]
pub struct ScanReport {
    pub scanned: usize,
    pub masters_added: usize,
    pub masters_updated: usize,
    pub masters_removed: usize,
    pub tracks_created: usize,
    pub unchanged: usize,
    pub errors: Vec<String>,
    pub rev: i64,
}

#[derive(Debug, Default)]
struct Props {
    title: String,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_no: Option<i64>,
    disc_no: Option<i64>,
    year: Option<i64>,
    duration_ms: Option<i64>,
    codec: Option<String>,
    bitrate: Option<i64>,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
    channels: Option<i64>,
    cover: Option<(Vec<u8>, String)>,
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_junk(s: &str) -> bool {
    let n = norm(s);
    n.is_empty() || JUNK_ARTISTS.iter().any(|j| n.starts_with(j)) || n.len() > 300
}

fn first_string(tag: &lofty::tag::Tag, keys: &[ItemKey]) -> Option<String> {
    keys.iter()
        .find_map(|k| tag.get_string(*k))
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
}

fn read_props(path: &Path) -> Result<Props> {
    let tagged = lofty::read_from_path(path)?;
    let p = tagged.properties();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let mut out = Props {
        duration_ms: Some(p.duration().as_millis() as i64),
        bitrate: p.audio_bitrate().map(|b| b as i64),
        sample_rate: p.sample_rate().map(|s| s as i64),
        bit_depth: p.bit_depth().map(|b| b as i64),
        channels: p.channels().map(|c| c as i64),
        codec: Some(ext(path)),
        ..Default::default()
    };

    if let Some(t) = tag {
        out.title = first_string(t, &[ItemKey::TrackTitle]).unwrap_or_default();
        out.artist = first_string(t, &[ItemKey::TrackArtist]);
        out.album_artist = first_string(t, &[ItemKey::AlbumArtist]);
        out.album = first_string(t, &[ItemKey::AlbumTitle]);
        out.track_no = t.track().map(|v| v as i64);
        out.disc_no = t.disk().map(|v| v as i64);
        out.year = first_string(t, &[ItemKey::Year, ItemKey::RecordingDate])
            .and_then(|v| v.get(0..4).map(str::to_string))
            .and_then(|v| v.parse::<i64>().ok());

        // Real-world tags lie: ARTIST says "MUSIC Engine", COMPOSER has the artist.
        if out.artist.as_deref().map(is_junk).unwrap_or(true) {
            out.artist = first_string(t, &[ItemKey::Composer]).filter(|s| !is_junk(s));
        }
    }

    if let Some(pic) = tag.and_then(|t| t.pictures().first()) {
        let mime = pic
            .mime_type()
            .map(|m| m.to_string())
            .unwrap_or_else(|| "image/jpeg".to_string());
        out.cover = Some((pic.data().to_vec(), mime));
    }

    if out.title.trim().is_empty() {
        out.title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
    }

    // "Artist - Title.flac" when the file carries no useful artist.
    if out.artist.is_none() {
        out.artist = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.split_once(" - ").map(|(a, _)| a.trim().to_string()))
            .filter(|a| !is_junk(a));
    }

    Ok(out)
}

fn stat(path: &Path) -> Result<(i64, i64)> {
    let m = std::fs::metadata(path)?;
    let mtime = m
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok((m.len() as i64, mtime))
}

fn artist_id(tx: &Transaction, name: &str) -> Result<i64> {
    let name = name.trim();
    let n = norm(name);
    if let Ok(id) = tx.query_row("SELECT id FROM artist WHERE norm=?1", [&n], |r| r.get(0)) {
        return Ok(id);
    }
    tx.execute("INSERT INTO artist(name, norm) VALUES(?1, ?2)", params![name, n])?;
    Ok(tx.last_insert_rowid())
}

fn album_id(
    tx: &Transaction,
    title: &str,
    artist_id: i64,
    artist_norm: &str,
    year: Option<i64>,
) -> Result<i64> {
    let n = format!("{artist_norm}/{}", norm(title));
    if let Ok(id) = tx.query_row("SELECT id FROM album WHERE norm=?1", [&n], |r| r.get(0)) {
        if year.is_some() {
            tx.execute("UPDATE album SET year=coalesce(year, ?1) WHERE id=?2", params![year, id])?;
        }
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO album(title, artist_id, year, norm) VALUES(?1, ?2, ?3, ?4)",
        params![title.trim(), artist_id, year, n],
    )?;
    Ok(tx.last_insert_rowid())
}

fn upsert_track(
    tx: &Transaction,
    album: i64,
    props: &Props,
) -> Result<(i64, bool)> {
    let existing: Option<i64> = match props.track_no {
        Some(no) => tx
            .query_row(
                "SELECT id FROM track
                 WHERE album_id=?1 AND coalesce(disc,1)=coalesce(?2,1) AND track_no=?3",
                params![album, props.disc_no, no],
                |r| r.get(0),
            )
            .ok(),
        None => tx
            .query_row(
                "SELECT id FROM track WHERE album_id=?1 AND lower(title)=lower(?2)",
                params![album, props.title.trim()],
                |r| r.get(0),
            )
            .ok(),
    };

    match existing {
        Some(id) => {
            tx.execute(
                "UPDATE track SET title=?1, duration_ms=coalesce(duration_ms, ?2), rev=0
                 WHERE id=?3",
                params![props.title.trim(), props.duration_ms, id],
            )?;
            Ok((id, false))
        }
        None => {
            tx.execute(
                "INSERT INTO track(title, album_id, disc, track_no, duration_ms, rev, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5, 0, unixepoch())",
                params![
                    props.title.trim(),
                    album,
                    props.disc_no,
                    props.track_no,
                    props.duration_ms
                ],
            )?;
            Ok((tx.last_insert_rowid(), true))
        }
    }
}

fn reindex_fts(tx: &Transaction, track_id: i64) -> Result<()> {
    tx.execute("DELETE FROM track_fts WHERE rowid=?1", [track_id])?;
    tx.execute(
        "INSERT INTO track_fts(rowid, title, artist, album)
         SELECT t.id, t.title, ar.name, al.title
         FROM track t JOIN album al ON al.id=t.album_id JOIN artist ar ON ar.id=al.artist_id
         WHERE t.id=?1",
        [track_id],
    )?;
    Ok(())
}

pub fn scan(db: &Db, root: &Path, ignores: &[String]) -> Result<ScanReport> {
    let mut conn = db.open()?;
    let covers_dir = db
        .path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("covers");
    std::fs::create_dir_all(&covers_dir)?;
    let tx = conn.transaction()?;
    let mut report = ScanReport::default();

    let existing: HashMap<String, (i64, i64, i64)> = {
        let mut stmt = tx.prepare("SELECT path, id, bytes, mtime FROM master")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?),
            ))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (path, v) = row?;
            map.insert(path, v);
        }
        map
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut touched_tracks: HashSet<i64> = HashSet::new();

    for entry in WalkDir::new(root).follow_links(true).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() || !is_audio(entry.path()) {
            continue;
        }
        let path_str_early = entry.path().to_string_lossy();
        if ignores.iter().any(|i| !i.is_empty() && path_str_early.contains(i.as_str())) {
            continue;
        }
        report.scanned += 1;
        let path_str = entry.path().to_string_lossy().to_string();
        seen.insert(path_str.clone());

        let (bytes, mtime) = match stat(entry.path()) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(format!("{path_str}: {e}"));
                continue;
            }
        };

        if let Some((_, old_bytes, old_mtime)) = existing.get(&path_str) {
            if *old_bytes == bytes && *old_mtime == mtime {
                report.unchanged += 1;
                continue;
            }
        }

        let props = match read_props(entry.path()) {
            Ok(p) => p,
            Err(e) => {
                report.errors.push(format!("{path_str}: {e}"));
                continue;
            }
        };

        let artist_name = props.artist.clone().unwrap_or_else(|| "Unknown Artist".into());
        let artist = artist_id(&tx, &artist_name)?;
        let album_title = props
            .album
            .clone()
            .filter(|a| !is_junk(a))
            .unwrap_or_else(|| "Unknown Album".into());
        let album = album_id(&tx, &album_title, artist, &norm(&artist_name), props.year)?;

        // The first embedded picture wins; an album never loses its art on rescan.
        if let Some((bytes, mime)) = &props.cover {
            let existing_cover: Option<String> = tx
                .query_row("SELECT cover_path FROM album WHERE id=?1", [album], |r| r.get(0))
                .ok()
                .flatten();
            if existing_cover.is_none() {
                let ext = if mime.contains("png") { "png" } else { "jpg" };
                let path = covers_dir.join(format!("{album}.{ext}"));
                if std::fs::write(&path, bytes).is_ok() {
                    tx.execute(
                        "UPDATE album SET cover_path=?1 WHERE id=?2",
                        params![path.to_string_lossy().to_string(), album],
                    )?;
                }
            }
        }
        let (track, created) = upsert_track(&tx, album, &props)?;
        touched_tracks.insert(track);
        if created {
            report.tracks_created += 1;
        }

        tx.execute(
            "INSERT INTO master(path, bytes, mtime, codec, duration_ms, sample_rate, channels,
                                bits, bitrate, ingested_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9, unixepoch())
             ON CONFLICT(path) DO UPDATE SET
               bytes=excluded.bytes, mtime=excluded.mtime, codec=excluded.codec,
               duration_ms=excluded.duration_ms, sample_rate=excluded.sample_rate,
               channels=excluded.channels, bits=excluded.bits, bitrate=excluded.bitrate",
            params![
                path_str,
                bytes,
                mtime,
                props.codec,
                props.duration_ms,
                props.sample_rate,
                props.channels,
                props.bit_depth,
                props.bitrate
            ],
        )?;
        let master_id = tx.query_row("SELECT id FROM master WHERE path=?1", [&path_str], |r| r.get::<_, i64>(0))?;
        tx.execute(
            "INSERT OR IGNORE INTO track_master(track_id, master_id, role) VALUES(?1, ?2, 'primary')",
            params![track, master_id],
        )?;

        if existing.contains_key(&path_str) {
            report.masters_updated += 1;
        } else {
            report.masters_added += 1;
        }
    }

    let rev = crate::db::rev(&tx)? + 1;

    // Masters whose file vanished, and the tracks they leave orphaned.
    let gone_masters: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id, path FROM master")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.filter_map(|r| r.ok())
            .filter(|(_, p)| !seen.contains(p))
            .map(|(id, _)| id)
            .collect()
    };
    for id in &gone_masters {
        tx.execute("DELETE FROM track_master WHERE master_id=?1", [id])?;
        tx.execute("DELETE FROM master WHERE id=?1", [id])?;
    }
    report.masters_removed = gone_masters.len();

    let orphaned: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT t.id FROM track t
             WHERE NOT EXISTS (SELECT 1 FROM track_master tm WHERE tm.track_id = t.id)",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    for id in &orphaned {
        tx.execute("DELETE FROM track WHERE id=?1", [id])?;
        tx.execute("DELETE FROM track_fts WHERE rowid=?1", [id])?;
        tx.execute("INSERT INTO tombstones(track_id, rev) VALUES(?1, ?2)", params![id, rev])?;
    }

    for id in &touched_tracks {
        tx.execute("UPDATE track SET rev=?1 WHERE id=?2", params![rev, id])?;
        reindex_fts(&tx, *id)?;
    }

    tx.execute(
        "INSERT INTO meta(key, value) VALUES('rev', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [rev.to_string()],
    )?;
    crate::db::default_user(&tx)?;
    tx.commit()?;

    report.rev = rev;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_extensions_are_recognised() {
        assert!(is_audio(Path::new("/x/a.FLAC")));
        assert!(is_audio(Path::new("/x/a.opus")));
        assert!(!is_audio(Path::new("/x/cover.jpg")));
        assert!(!is_audio(Path::new("/x/noext")));
    }

    #[test]
    fn lossless_list_is_a_subset_of_audio() {
        for e in LOSSLESS_EXT {
            assert!(AUDIO_EXT.contains(e), "{e} must be importable");
        }
    }

    #[test]
    fn junk_artists_are_rejected() {
        assert!(is_junk("MUSIC Engine"));
        assert!(is_junk("Lavf58.29.100"));
        assert!(is_junk("  "));
        assert!(!is_junk("Toby Fox"));
    }
}
