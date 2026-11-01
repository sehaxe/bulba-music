use crate::db::Db;
use anyhow::Result;
use lofty::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

pub const AUDIO_EXT: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "aiff", "aif", "ape", "wv", "alac",
];

/// Formats that are lossless and therefore normalised to FLAC on import.
pub const LOSSLESS_EXT: &[&str] = &["flac", "wav", "aiff", "aif", "ape", "wv", "alac"];

#[derive(Debug, Default, Serialize)]
pub struct ScanReport {
    pub scanned: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
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
        out.title = t.title().map(|v| v.to_string()).unwrap_or_default();
        out.artist = t.artist().map(|v| v.to_string()).filter(|s| !s.is_empty());
        out.album = t.album().map(|v| v.to_string()).filter(|s| !s.is_empty());
        out.album_artist = t
            .get_string(ItemKey::AlbumArtist)
            .map(|v| v.to_string())
            .filter(|s| !s.is_empty());
        out.track_no = t.track().map(|v| v as i64);
        out.disc_no = t.disk().map(|v| v as i64);
        out.year = [ItemKey::Year, ItemKey::RecordingDate, ItemKey::OriginalReleaseDate]
            .into_iter()
            .find_map(|k| t.get_string(k))
            .and_then(|v| v.get(0..4))
            .and_then(|v| v.parse::<i64>().ok());
    }

    if out.title.trim().is_empty() {
        out.title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
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

pub fn scan(db: &Db, root: &Path) -> Result<ScanReport> {
    let mut conn = db.open()?;
    let tx = conn.transaction()?;
    let mut report = ScanReport::default();

    let existing: HashMap<String, (i64, i64)> = {
        let mut stmt = tx.prepare("SELECT path, size, mtime FROM tracks")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut map = HashMap::new();
        for row in rows {
            let (path, size, mtime) = row?;
            map.insert(path, (size, mtime));
        }
        map
    };

    let mut seen: HashMap<String, bool> = HashMap::new();

    for entry in WalkDir::new(root).follow_links(true).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() || !is_audio(entry.path()) {
            continue;
        }
        report.scanned += 1;
        let path_str = entry.path().to_string_lossy().to_string();
        seen.insert(path_str.clone(), true);

        let (size, mtime) = match stat(entry.path()) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(format!("{path_str}: {e}"));
                continue;
            }
        };

        if let Some((old_size, old_mtime)) = existing.get(&path_str) {
            if *old_size == size && *old_mtime == mtime {
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

        let existed = existing.contains_key(&path_str);
        tx.execute(
            "INSERT INTO tracks
              (path, size, mtime, title, artist, album, album_artist, track_no, disc_no,
               year, duration_ms, codec, bitrate, sample_rate, bit_depth, channels, rev, added_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16, 0, unixepoch())
             ON CONFLICT(path) DO UPDATE SET
               size=excluded.size, mtime=excluded.mtime, title=excluded.title,
               artist=excluded.artist, album=excluded.album,
               album_artist=excluded.album_artist, track_no=excluded.track_no,
               disc_no=excluded.disc_no, year=excluded.year,
               duration_ms=excluded.duration_ms, codec=excluded.codec,
               bitrate=excluded.bitrate, sample_rate=excluded.sample_rate,
               bit_depth=excluded.bit_depth, channels=excluded.channels,
               rev=0",
            rusqlite::params![
                path_str,
                size,
                mtime,
                props.title,
                props.artist,
                props.album,
                props.album_artist,
                props.track_no,
                props.disc_no,
                props.year,
                props.duration_ms,
                props.codec,
                props.bitrate,
                props.sample_rate,
                props.bit_depth,
                props.channels,
            ],
        )?;

        if existed {
            report.updated += 1;
        } else {
            report.added += 1;
        }
    }

    // Anything previously imported but no longer on disk is deleted and tombstoned.
    let rev = crate::db::rev(&tx)? + 1;
    let gone: Vec<(i64, String)> = {
        let mut stmt = tx.prepare("SELECT id, path FROM tracks")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        rows.filter_map(|r| r.ok())
            .filter(|(_, p)| !seen.contains_key(p))
            .collect()
    };
    for (id, _) in &gone {
        tx.execute("DELETE FROM tracks WHERE id=?1", [id])?;
        tx.execute("INSERT INTO tombstones(track_id, rev) VALUES (?1, ?2)", [id, &rev])?;
    }
    report.removed = gone.len();

    // Rows touched in this scan were written with rev=0; stamp them with the scan revision.
    tx.execute("UPDATE tracks SET rev=?1 WHERE rev=0", [rev])?;
    tx.execute(
        "INSERT INTO meta(key, value) VALUES('rev', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [rev.to_string()],
    )?;
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
}
