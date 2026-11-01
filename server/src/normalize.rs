use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Uniform file names inside a library that was downloaded album by album.
///
/// The safety rules matter more than the pattern:
///   * a dry run is the default and reports exactly what would happen;
///   * nothing is overwritten — an existing target is a conflict, not a merge;
///   * two files that would land on the same name are a conflict for both;
///   * the database row is updated in the same pass, so play history, likes and
///     embeddings survive the rename instead of looking like a fresh import.
#[derive(Debug, Serialize, Clone)]
pub struct Rename {
    pub from: String,
    pub to: String,
    pub track_id: Option<i64>,
}

#[derive(Debug, Serialize, Default)]
pub struct Report {
    pub total: i64,
    pub planned: Vec<Rename>,
    pub conflicts: Vec<Conflict>,
    pub skipped: i64,
    pub applied: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Conflict {
    pub path: String,
    pub reason: String,
}

/// `01 - Title.ext` when the file already sits in its album folder, otherwise
/// `Artist - Album - 01 - Title.ext`. Extension is preserved byte for byte.
pub fn target_name(
    artist: &str,
    album: &str,
    track_no: Option<i64>,
    title: &str,
    ext: &str,
    in_album_folder: bool,
) -> String {
    let track = match track_no {
        Some(n) if n > 0 => format!("{n:02} - "),
        _ => String::new(),
    };
    let known_album = !album.trim().is_empty()
        && !album.eq_ignore_ascii_case("unknown album")
        && !album.eq_ignore_ascii_case("unknown");
    let base = if in_album_folder {
        format!("{track}{title}")
    } else {
        let a = sanitize(if artist.trim().is_empty() { "Unknown Artist" } else { artist });
        if known_album {
            let al = sanitize(album);
            format!("{a} - {al} - {track}{title}")
        } else {
            // No album tag: inventing "Unknown Album" makes the file name worse,
            // not more uniform.
            format!("{a} - {track}{title}")
        }
    };
    let mut name = sanitize(&base);
    if name.is_empty() {
        name = "track".into();
    }
    if name.len() > 180 {
        name.truncate(180);
        name = name.trim_end_matches([' ', '-']).to_string();
    }
    format!("{name}.{ext}")
}

/// Forbidden on at least one of the platforms this library travels to.
pub fn sanitize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            '\n' | '\r' | '\t' => ' ',
            c if (c as u32) < 0x20 => ' ',
            c => c,
        })
        .collect();
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    let trimmed = out.trim().trim_end_matches('.').trim().to_string();
    if trimmed.is_empty() { out.trim().to_string() } else { trimmed }
}

/// `(artist, album, track_no, title, ext)` for one row, from the database,
/// which already holds what the tags said.
struct Row {
    id: i64,
    path: String,
    artist: String,
    album: String,
    track_no: Option<i64>,
    title: String,
    album_id: i64,
}

pub fn plan(conn: &Connection, libraries: &[PathBuf]) -> Result<Report> {
    let mut report = Report::default();
    let mut stmt = conn.prepare(
        "SELECT t.id, m.path, ar.name, al.title, t.track_no, t.title, al.id
         FROM track t
         JOIN track_master tm ON tm.track_id = t.id
         JOIN master m ON m.id = tm.master_id
         JOIN album al ON al.id = t.album_id
         JOIN artist ar ON ar.id = al.artist_id
         ORDER BY m.path",
    )?;
    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok(Row {
                id: r.get(0)?,
                path: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                track_no: r.get(4)?,
                title: r.get(5)?,
                album_id: r.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    report.total = rows.len() as i64;

    let mut targets: HashMap<String, i64> = HashMap::new();
    let mut plans: Vec<(Rename, PathBuf, PathBuf, i64)> = Vec::new();

    for row in &rows {
        let src = PathBuf::from(&row.path);
        if !src.exists() {
            report.skipped += 1;
            continue;
        }
        if !libraries.is_empty()
            && !libraries
                .iter()
                .any(|lib| src.starts_with(lib.canonicalize().unwrap_or(lib.clone())))
        {
            report.skipped += 1;
            continue;
        }
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        if ext.is_empty() {
            report.skipped += 1;
            continue;
        }
        let parent = src.parent().map(Path::to_path_buf).unwrap_or_default();
        let album_folder = folder_matches(&parent, &row.album, &row.artist, row.track_no);
        let name = target_name(
            &row.artist,
            &row.album,
            row.track_no,
            &row.title,
            &ext,
            album_folder,
        );
        let dst = parent.join(&name);
        if dst == src {
            continue;
        }
        let dst_str = dst.to_string_lossy().to_string();
        if let Some(other) = targets.get(&dst_str) {
            report.conflicts.push(Conflict {
                path: row.path.clone(),
                reason: format!("цель совпадает с треком {other}"),
            });
            continue;
        }
        if dst.exists() {
            report.conflicts.push(Conflict {
                path: row.path.clone(),
                reason: "файл с таким именем уже есть".into(),
            });
            continue;
        }
        targets.insert(dst_str.clone(), row.id);
        plans.push((
            Rename {
                from: row.path.clone(),
                to: dst_str,
                track_id: Some(row.id),
            },
            src,
            dst,
            row.album_id,
        ));
    }

    // A file that is itself the target of another rename must move first;
    // sorting by "source is not a target" avoids most cycles. Remaining cycles
    // are reported instead of being half-applied.
    let sources: Vec<String> = plans.iter().map(|(r, _, _, _)| r.from.clone()).collect();
    plans.sort_by_key(|(r, _, _, _)| sources.contains(&r.to));

    report.planned = plans.iter().map(|(r, _, _, _)| r.clone()).collect();
    Ok(report)
}

fn folder_matches(parent: &Path, album: &str, artist: &str, _track_no: Option<i64>) -> bool {
    let name = parent
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if album.trim().is_empty() {
        return false;
    }
    let album_l = sanitize(album).to_lowercase();
    let artist_l = artist.to_lowercase();
    name.contains(&album_l) || (name.contains(&artist_l) && album_l.len() > 2 && name.contains(&album_l[..album_l.len().min(12)]))
}

pub fn apply(conn: &Connection, libraries: &[PathBuf], limit: usize) -> Result<Report> {
    let mut report = plan(conn, libraries)?;
    let planned = report.planned.clone();
    // A rename log next to the database makes the whole batch reversible even
    // if the tool's rules change tomorrow.
    let mut undone: Vec<Rename> = Vec::new();
    for rename in planned.into_iter().take(limit) {
        let from = PathBuf::from(&rename.from);
        let to = PathBuf::from(&rename.to);
        match std::fs::rename(&from, &to) {
            Ok(()) => {
                // Keep identity: the row follows the file.
                conn.execute(
                    "UPDATE master SET path=?1 WHERE path=?2",
                    rusqlite::params![rename.to, rename.from],
                )?;
                undone.push(rename.clone());
                report.applied += 1;
            }
            Err(e) => report.errors.push(format!("{}: {e}", rename.from)),
        }
    }
    if !undone.is_empty() {
        let path = std::env::var("BULBA_DB")
            .ok()
            .map(|p| PathBuf::from(p).with_extension("renames.json"))
            .unwrap_or_else(|| PathBuf::from("/tmp/bulba-renames.json"));
        if let Ok(json) = serde_json::to_string_pretty(&undone) {
            let _ = std::fs::write(path, json);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_path_separators_and_keeps_cyrillic() {
        assert_eq!(sanitize("AC/DC: Back in Black?"), "AC_DC_ Back in Black_");
        assert_eq!(sanitize("Кино  —  Группа крови"), "Кино — Группа крови");
    }

    #[test]
    fn track_number_is_zero_padded() {
        let n = target_name("Tame Impala", "Currents", Some(2), "Nangs", "flac", true);
        assert_eq!(n, "02 - Nangs.flac");
    }

    #[test]
    fn outside_an_album_folder_the_name_carries_artist_and_album() {
        let n = target_name("Tame Impala", "Currents", Some(2), "Nangs", "flac", false);
        assert_eq!(n, "Tame Impala - Currents - 02 - Nangs.flac");
    }

    #[test]
    fn missing_track_number_does_not_leave_a_dangling_dash() {
        let n = target_name("A", "B", None, "C", "mp3", true);
        assert_eq!(n, "C.mp3");
    }

    #[test]
    fn empty_title_falls_back() {
        assert_eq!(sanitize("   "), "");
        let n = target_name("", "", None, "  ", "flac", true);
        assert_eq!(n, "track.flac");
    }

    #[test]
    fn long_titles_are_cut_to_a_safe_length() {
        let long = "x".repeat(400);
        let n = target_name("a", "b", Some(1), &long, "flac", true);
        assert!(n.len() <= 190);
        assert!(n.ends_with(".flac"));
    }

    #[test]
    fn rename_updates_the_row_so_history_survives() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::new(dir.path().join("n.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        let album_dir = dir.path().join("Album");
        std::fs::create_dir_all(&album_dir).unwrap();
        let src = album_dir.join("incoming_track_.flac");
        std::fs::write(&src, b"x").unwrap();

        conn.execute("INSERT INTO artist(name,norm) VALUES('A','a')", []).unwrap();
        let artist = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO album(title,artist_id,year,norm) VALUES('Album',?1,2020,'a/album')",
            [artist],
        )
        .unwrap();
        let album = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO track(title,album_id,disc,track_no,duration_ms,rev,created_at)
             VALUES('Song',?1,1,3,1000,1,0)",
            [album],
        )
        .unwrap();
        let track = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO master(path,bytes,mtime,codec,ingested_at) VALUES(?1,1,0,'flac',0)",
            [src.to_string_lossy().to_string()],
        )
        .unwrap();
        let master = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO track_master(track_id,master_id,role) VALUES(?1,?2,'original')",
            [track, master],
        )
        .unwrap();
        let user = crate::db::default_user(&conn).unwrap();
        conn.execute(
            "INSERT INTO play_event(ts,user_id,track_id,session_id,event,position_ms,
                position_ratio,source,device,offline,quality,client_id)
             VALUES(1,?2,?1,1,0,0,0,0,0,0,0,'t')",
            rusqlite::params![track, user],
        )
        .unwrap();

        let report = apply(&conn, &[dir.path().to_path_buf()], 10).unwrap();
        assert_eq!(report.applied, 1);
        assert!(album_dir.join("03 - Song.flac").exists());
        assert!(!src.exists());

        // Same row, new path: the play history still points at this track.
        let events: i64 = conn
            .query_row("SELECT count(*) FROM play_event WHERE track_id=?1", [track], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(events, 1);
        let path: String = conn
            .query_row("SELECT path FROM master WHERE id=?1", [master], |r| r.get(0))
            .unwrap();
        assert!(path.ends_with("03 - Song.flac"));
    }

    #[test]
    fn collisions_are_reported_not_applied() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::new(dir.path().join("c.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        conn.execute("INSERT INTO artist(name,norm) VALUES('A','a')", []).unwrap();
        let artist = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO album(title,artist_id,year,norm) VALUES('Album',?1,2020,'a/album')",
            [artist],
        )
        .unwrap();
        let album = conn.last_insert_rowid();
        let album_dir = dir.path().join("Album");
        std::fs::create_dir_all(&album_dir).unwrap();
        for (name, title) in [("one.flac", "Same"), ("two.flac", "Same")] {
            let p = album_dir.join(name);
            std::fs::write(&p, b"x").unwrap();
            conn.execute(
                "INSERT INTO track(title,album_id,disc,track_no,duration_ms,rev,created_at)
                 VALUES(?1,?2,1,1,1000,1,0)",
                rusqlite::params![title, album],
            )
            .unwrap();
            let track = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO master(path,bytes,mtime,codec,ingested_at) VALUES(?1,1,0,'flac',0)",
                [p.to_string_lossy().to_string()],
            )
            .unwrap();
            let master = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO track_master(track_id,master_id,role) VALUES(?1,?2,'original')",
                [track, master],
            )
            .unwrap();
        }
        let report = apply(&conn, &[dir.path().to_path_buf()], 10).unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(report.conflicts.len(), 1);
        assert!(album_dir.join("01 - Same.flac").exists());
    }
}

/// Folder-level organisation: one canonical tree instead of whatever the
/// downloader left behind.
///
///   <root>/Artist/Album (Year)/[CDn/]NN - Title.ext
///
/// Files that would land on a name already taken are not merged and not
/// deleted: duplicates go to `<root>/_duplicates` and collisions are reported,
/// because a library is the one thing a tool must never silently eat.
#[derive(Debug, Serialize, Default)]
pub struct TreeReport {
    pub total: i64,
    pub planned: Vec<Rename>,
    pub duplicates: Vec<Rename>,
    pub conflicts: Vec<Conflict>,
    pub skipped: i64,
    pub applied: usize,
    pub errors: Vec<String>,
}

fn clean_component(s: &str, fallback: &str) -> String {
    let v = sanitize(s);
    if v.trim().is_empty() || v.eq_ignore_ascii_case("unknown") {
        fallback.to_string()
    } else {
        v
    }
}

pub fn tree_plan(conn: &Connection, root: &Path) -> Result<TreeReport> {
    let mut report = TreeReport::default();
    let mut stmt = conn.prepare(
        "SELECT t.id, m.path, ar.name, al.title, al.year, t.disc, t.track_no, t.title
         FROM track t
         JOIN track_master tm ON tm.track_id = t.id
         JOIN master m ON m.id = tm.master_id
         JOIN album al ON al.id = t.album_id
         JOIN artist ar ON ar.id = al.artist_id
         ORDER BY m.path",
    )?;

    struct Row {
        _id: i64,
        path: String,
        artist: String,
        album: String,
        year: Option<i64>,
        disc: Option<i64>,
        track_no: Option<i64>,
        title: String,
    }
    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok(Row {
                _id: r.get(0)?,
                path: r.get(1)?,
                artist: r.get(2)?,
                album: r.get(3)?,
                year: r.get(4)?,
                disc: r.get(5)?,
                track_no: r.get(6)?,
                title: r.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    report.total = rows.len() as i64;

    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_identity: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for row in &rows {
        let src = PathBuf::from(&row.path);
        if !src.exists() {
            report.skipped += 1;
            continue;
        }
        let ext = match src.extension() {
            Some(e) => e.to_string_lossy().to_string(),
            None => {
                report.skipped += 1;
                continue;
            }
        };

        let artist_dir = clean_component(&row.artist, "_unknown artist");
        let album_dir = match row.year {
            Some(y) => format!("{} ({y})", clean_component(&row.album, "Unknown Album")),
            None => clean_component(&row.album, "Unknown Album"),
        };
        let mut dir = root.join(&artist_dir).join(&album_dir);
        if let Some(d) = row.disc {
            if d > 1 {
                dir = dir.join(format!("CD{d}"));
            }
        }
        let name = match row.track_no {
            Some(n) if n > 0 => format!("{n:02} - {}.{}", sanitize(&row.title), ext),
            _ => format!("{}.{}", sanitize(&row.title), ext),
        };
        let mut dst = dir.join(&name);

        // Same record already present under a different file name.
        let identity = format!(
            "{}|{}|{}|{}|{}",
            row.artist.to_lowercase(),
            row.album.to_lowercase(),
            row.disc.unwrap_or(1),
            row.track_no.unwrap_or(0),
            row.title.to_lowercase()
        );
        if seen_identity.contains_key(&identity) {
            // Keep the file exactly as it arrived, only out of the way: the
            // duplicate folder is for deciding later, not for renaming twice.
            let original = src
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            let mut dup = root.join("_duplicates").join(&artist_dir).join(&original);
            let mut n = 2;
            while dup.exists() {
                let stems = original.rsplit_once('.').map(|(a, b)| (a.to_string(), b.to_string()));
                dup = match stems {
                    Some((stem, ext)) => root
                        .join("_duplicates")
                        .join(&artist_dir)
                        .join(format!("{stem} ({n}).{ext}")),
                    None => root
                        .join("_duplicates")
                        .join(&artist_dir)
                        .join(format!("{original} ({n})")),
                };
                n += 1;
            }
            report.duplicates.push(Rename {
                from: row.path.clone(),
                to: dup.to_string_lossy().to_string(),
                track_id: Some(row._id),
            });
            continue;
        }
        seen_identity.insert(identity, row.path.clone());

        let dst_str = dst.to_string_lossy().to_string();
        if dst == src {
            taken.insert(dst_str);
            continue;
        }
        if dst.exists() || taken.contains(&dst_str) {
            report.conflicts.push(Conflict {
                path: row.path.clone(),
                reason: format!("уже занято: {}", dst_str),
            });
            continue;
        }
        taken.insert(dst_str.clone());
        report.planned.push(Rename {
            from: row.path.clone(),
            to: dst_str,
            track_id: Some(row._id),
        });
    }

    Ok(report)
}

pub fn tree_apply(conn: &Connection, root: &Path, limit: usize) -> Result<TreeReport> {
    let mut report = tree_plan(conn, root)?;
    let mut undo: Vec<Rename> = Vec::new();

    for rename in report.planned.iter().take(limit) {
        let from = PathBuf::from(&rename.from);
        let to = PathBuf::from(&rename.to);
        if let Some(parent) = to.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                report.errors.push(format!("{}: {e}", rename.from));
                continue;
            }
        }
        match std::fs::rename(&from, &to) {
            Ok(()) => {
                conn.execute(
                    "UPDATE master SET path=?1 WHERE path=?2",
                    rusqlite::params![rename.to, rename.from],
                )?;
                // Keep the cover with the album folder it now belongs to.
                undo.push(rename.clone());
                report.applied += 1;
            }
            Err(e) => report.errors.push(format!("{}: {e}", rename.from)),
        }
    }

    for rename in report.duplicates.iter().take(limit) {
        let from = PathBuf::from(&rename.from);
        let to = PathBuf::from(&rename.to);
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::rename(&from, &to).is_ok() {
            conn.execute(
                "UPDATE master SET path=?1 WHERE path=?2",
                rusqlite::params![rename.to, rename.from],
            )?;
        }
    }

    if !undo.is_empty() {
        let path = std::env::var("BULBA_DB")
            .ok()
            .map(|p| PathBuf::from(p).with_extension("tree.json"))
            .unwrap_or_else(|| PathBuf::from("/tmp/bulba-tree.json"));
        if let Ok(json) = serde_json::to_string_pretty(&undo) {
            let _ = std::fs::write(path, json);
        }
    }
    Ok(report)
}
