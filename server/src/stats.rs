use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

/// Listening statistics. The point is not decoration: the same numbers feed the
/// recommender, because "what gets replayed" is the strongest taste signal a
/// single listener ever produces.
///
/// Counting rules, stated so the numbers can be trusted:
///   plays      — one row per start (event 0)
///   completions— one row per track finished (event 3)
///   repeats    — one row per deliberate replay of the same track (event 4)
///   hours      — completed plays count in full, started-but-unfinished plays
///                count for the fraction of the track actually reached
#[derive(Debug, Serialize)]
pub struct Stats {
    pub period: String,
    pub label: String,
    pub totals: Totals,
    pub top_tracks: Vec<TopTrack>,
    pub top_artists: Vec<TopArtist>,
    pub top_albums: Vec<TopAlbum>,
    pub months: Vec<MonthBucket>,
    pub days_played: i64,
    pub longest_streak: i64,
    pub first_play: Option<i64>,
    pub repeat_track: Option<TopTrack>,
}

#[derive(Debug, Serialize, Default)]
pub struct Totals {
    pub plays: i64,
    pub completions: i64,
    pub repeats: i64,
    pub skips: i64,
    /// Share of starts that ended in a skip. The one number that says whether the
    /// recommender is getting better.
    pub skip_rate: f64,
    /// How far into a track a skip happens, 0..1. Low means "wrong track",
    /// high means "wrong moment".
    pub skip_depth: f64,
    pub hours: f64,
    pub tracks: i64,
    pub artists: i64,
    pub likes: i64,
}

#[derive(Debug, Serialize)]
pub struct TopTrack {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album_id: i64,
    pub has_cover: i64,
    pub plays: i64,
    pub repeats: i64,
    pub completions: i64,
    pub hours: f64,
}

#[derive(Debug, Serialize)]
pub struct TopArtist {
    pub name: String,
    pub plays: i64,
    pub tracks: i64,
}

#[derive(Debug, Serialize)]
pub struct TopAlbum {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub has_cover: i64,
    pub plays: i64,
}

#[derive(Debug, Serialize)]
pub struct MonthBucket {
    pub month: i64,
    pub plays: i64,
    pub hours: f64,
}

/// `(from_ts, to_ts)` for a period, plus a human label. Months are local time;
/// a listening year is a human year, not a UTC one.
pub fn window(period: &str, year: Option<i64>, month: Option<i64>) -> (i64, i64, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Civil-from-days arithmetic without a date crate.
    let (y, m) = civil_from_days(now / 86_400);
    match period {
        "all" => (0, i64::MAX / 2, "всё время".into()),
        "month" => {
            let (yy, mm) = match (year, month) {
                (Some(y), Some(m)) => (y, m),
                _ => (y, m),
            };
            let start = days_from_civil(yy, mm, 1) * 86_400;
            let end = days_from_civil(if mm == 12 { yy + 1 } else { yy }, if mm == 12 { 1 } else { mm + 1 }, 1) * 86_400;
            (start, end, format!("{}-{:02}", yy, mm))
        }
        _ => {
            let yy = year.unwrap_or(y);
            let start = days_from_civil(yy, 1, 1) * 86_400;
            let end = days_from_civil(yy + 1, 1, 1) * 86_400;
            (start, end, format!("{yy}"))
        }
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m)
}

pub fn collect(
    conn: &Connection,
    user: i64,
    period: &str,
    year: Option<i64>,
    month: Option<i64>,
) -> Result<Stats> {
    let (from, to, label) = window(period, year, month);
    let bind = rusqlite::params![from, to];

    let count_of = |event: i64| -> Result<i64> {
        Ok(conn.query_row(
            "SELECT count(*) FROM play_event
             WHERE event=?1 AND ts>=?2 AND ts<?3 AND user_id=?4",
            rusqlite::params![event, from, to, user],
            |r| r.get(0),
        )?)
    };
    let plays = count_of(0)?;
    let completions = count_of(3)?;
    let repeats = count_of(4)?;
    let skips = count_of(2)?;
    // Depth is how far into a track people bail; the rate is skips over starts.
    // Dividing skips by skips (the obvious SQL) reports 100% forever.
    let skip_depth: f64 = conn.query_row(
        "SELECT coalesce(avg(position_ratio),0) FROM play_event
         WHERE user_id=?1 AND event=2",
        [user],
        |r| r.get(0),
    )?;
    let skip_rate = if plays > 0 {
        skips as f64 / plays as f64
    } else {
        0.0
    };

    // Hours are "how far into the track did this session get", not the sum of
    // progress ticks: a long track would otherwise be counted once per 15-second
    // heartbeat and report days of listening.
    let hours: f64 = conn.query_row(
        "SELECT coalesce(sum(
            CASE WHEN mx >= coalesce(duration_ms,0) THEN coalesce(duration_ms,0)
                 ELSE mx END),0)/3600000.0
         FROM (
           SELECT pe.session_id, pe.track_id,
                  max(coalesce(pe.position_ms,0)) AS mx,
                  (SELECT duration_ms FROM track WHERE id = pe.track_id) AS duration_ms
           FROM play_event pe
           WHERE pe.ts>=?1 AND pe.ts<?2 AND pe.user_id=?3
           GROUP BY pe.session_id, pe.track_id
         )",
        rusqlite::params![from, to, user],
        |r| r.get(0),
    )?;

    let like_count: i64 = conn.query_row(
        "SELECT count(*) FROM track_like WHERE user_id=?1",
        [user],
        |r| r.get(0),
    )?;

    // Distinct tracks and artists touched in the period.
    let (tracks, artists): (i64, i64) = conn.query_row(
        "SELECT count(DISTINCT pe.track_id),
                count(DISTINCT al.artist_id)
         FROM play_event pe
         JOIN track t ON t.id = pe.track_id
         JOIN album al ON al.id = t.album_id
         WHERE pe.ts>=?1 AND pe.ts<?2 AND pe.user_id=?3",
        rusqlite::params![from, to, user],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let top_tracks = tracks_query(
        conn,
        user,
        from,
        to,
        "ORDER BY plays DESC, hours DESC LIMIT 20",
    )?;
    let repeat_track = tracks_query(
        conn,
        user,
        from,
        to,
        "ORDER BY repeats DESC, plays DESC LIMIT 1",
    )?
        .into_iter()
        .next();

    let mut stmt = conn.prepare(
        "SELECT ar.name, count(*) AS plays,
                count(DISTINCT t.id) AS tracks
         FROM play_event pe
         JOIN track t ON t.id = pe.track_id
         JOIN album al ON al.id = t.album_id
         JOIN artist ar ON ar.id = al.artist_id
         WHERE pe.event=0 AND pe.ts>=?1 AND pe.ts<?2 AND pe.user_id=?3
         GROUP BY ar.id ORDER BY plays DESC LIMIT 12",
    )?;
    let top_artists: Vec<TopArtist> = stmt
        .query_map(rusqlite::params![from, to, user], |r| {
            Ok(TopArtist {
                name: r.get(0)?,
                plays: r.get(1)?,
                tracks: r.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn.prepare(
        "SELECT al.id, al.title, ar.name,
                CASE WHEN al.cover_path IS NULL THEN 0 ELSE 1 END,
                count(*) AS plays
         FROM play_event pe
         JOIN track t ON t.id = pe.track_id
         JOIN album al ON al.id = t.album_id
         JOIN artist ar ON ar.id = al.artist_id
         WHERE pe.event=0 AND pe.ts>=?1 AND pe.ts<?2 AND pe.user_id=?3
         GROUP BY al.id ORDER BY plays DESC LIMIT 12",
    )?;
    let top_albums: Vec<TopAlbum> = stmt
        .query_map(rusqlite::params![from, to, user], |r| {
            Ok(TopAlbum {
                id: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                has_cover: r.get(3)?,
                plays: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Twelve buckets, always. A year view with gaps should look like gaps.
    let mut months = Vec::with_capacity(12);
    {
        let mut stmt = conn.prepare(
            "SELECT cast(strftime('%m', ts, 'unixepoch', 'localtime') as integer) AS m,
                    count(*),
                    coalesce(sum(coalesce((SELECT duration_ms FROM track WHERE id=pe.track_id),0)),0)/3600000.0
             FROM play_event pe
             WHERE event IN (0,3) AND ts>=?1 AND ts<?2
             GROUP BY m",
        )?;
        let rows: Vec<(i64, i64, f64)> = stmt
            .query_map([from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        for m in 1..=12 {
            let found = rows.iter().find(|(mm, _, _)| *mm == m);
            months.push(MonthBucket {
                month: m,
                plays: found.map(|f| f.1).unwrap_or(0),
                hours: found.map(|f| f.2).unwrap_or(0.0),
            });
        }
    }

    let mut days: Vec<i64> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT cast(strftime('%s', date(ts,'unixepoch','localtime')) as integer)/86400
             FROM play_event WHERE ts>=?1 AND ts<?2 AND event=0 ORDER BY 1",
        )?;
        days = stmt
            .query_map([from, to], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
    }
    let mut longest = 0;
    let mut run = 0;
    let mut prev: Option<i64> = None;
    for d in &days {
        run = match prev {
            Some(p) if *d == p + 1 => run + 1,
            _ => 1,
        };
        longest = longest.max(run);
        prev = Some(*d);
    }

    let first_play: Option<i64> = conn
        .query_row("SELECT min(ts) FROM play_event WHERE event=0", [], |r| {
            r.get(0)
        })
        .ok()
        .flatten();

    Ok(Stats {
        period: period.to_string(),
        label,
        totals: Totals {
            plays,
            completions,
            repeats,
            skips,
            skip_rate,
            skip_depth,
            hours,
            tracks,
            artists,
            likes: like_count,
        },
        top_tracks,
        top_artists,
        top_albums,
        months,
        days_played: days.len() as i64,
        longest_streak: longest,
        first_play,
        repeat_track,
    })
}

fn tracks_query(
    conn: &Connection,
    user: i64,
    from: i64,
    to: i64,
    tail: &str,
) -> Result<Vec<TopTrack>> {
    let sql = format!(
        "SELECT t.id, t.title, t.artist, t.album_id, t.has_cover,
                sum(CASE WHEN pe.event=0 THEN 1 ELSE 0 END) AS plays,
                sum(CASE WHEN pe.event=4 THEN 1 ELSE 0 END) AS repeats,
                sum(CASE WHEN pe.event=3 THEN 1 ELSE 0 END) AS completions,
                coalesce(sum(CASE WHEN pe.event=3 THEN coalesce(t.duration_ms,0) ELSE 0 END),0)/3600000.0 AS hours
         FROM play_event pe JOIN v_track t ON t.id = pe.track_id
         WHERE pe.ts>=?1 AND pe.ts<?2 AND pe.user_id=?3
         GROUP BY t.id
         {tail}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![from, to, user], |r| {
        Ok(TopTrack {
            id: r.get(0)?,
            title: r.get(1)?,
            artist: r.get(2)?,
            album_id: r.get(3)?,
            has_cover: r.get(4)?,
            plays: r.get(5)?,
            repeats: r.get(6)?,
            completions: r.get(7)?,
            hours: r.get(8)?,
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        for (y, m, d) in [(2026, 11, 1), (2000, 2, 29), (1970, 1, 1), (1999, 12, 31)] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m), "date {y}-{m}-{d}");
        }
    }

    #[test]
    fn window_covers_a_year() {
        let (from, to, label) = window("year", Some(2026), None);
        assert_eq!(label, "2026");
        assert!(to - from == 365 * 86_400);
    }

    #[test]
    fn empty_db_reports_zeroes_not_errors() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::Db::new(dir.path().join("s.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        let user = crate::auth::local_user(&conn).unwrap();
        let s = collect(&conn, user, "all", None, None).unwrap();
        assert_eq!(s.totals.plays, 0);
        assert_eq!(s.months.len(), 12);
    }
}
