//! Smart lists: the questions a person asks about their own library, answered by
//! the database.
//!
//! Each preset is a named question with a fixed answer shape, not a query
//! language. That is deliberate: a rule builder is a lot of UI for a library with
//! one owner, and the questions people actually ask are few — "what have I not
//! touched in a month", "what is fast and lossless", "what is short".

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// Owned, liked or not, untouched for a month.
    DeepCuts,
    /// Danceable and lossless: the honest answer to "put on something that moves".
    FastLossless,
    /// Under two and a half minutes: the filler between two things.
    Short,
    /// Loud, fast and major: a morning that is not a slow one.
    Bright,
}

impl Preset {
    pub fn parse(s: &str) -> Option<Preset> {
        match s {
            "deep_cuts" => Some(Preset::DeepCuts),
            "fast_lossless" => Some(Preset::FastLossless),
            "short" => Some(Preset::Short),
            "bright" => Some(Preset::Bright),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Preset::DeepCuts => "deep_cuts",
            Preset::FastLossless => "fast_lossless",
            Preset::Short => "short",
            Preset::Bright => "bright",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Preset::DeepCuts => "не трогал месяц",
            Preset::FastLossless => "быстрое и лосслесс",
            Preset::Short => "короче двух минут",
            Preset::Bright => "яркое и бодрое",
        }
    }

    pub fn note(self) -> &'static str {
        match self {
            Preset::DeepCuts => "ни разу не играло или молчит 30 дней",
            Preset::FastLossless => "темп выше 125, мастер flac",
            Preset::Short => "треки до 2:30",
            Preset::Bright => "мажор, громко, быстро",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SmartList {
    pub id: String,
    pub title: String,
    pub note: String,
    pub count: i64,
}

/// The count is cheap to compute and lets the client grey out an empty list
/// instead of offering a button that plays nothing.
pub fn presets(conn: &Connection, user: i64) -> Result<Vec<SmartList>> {
    let mut out = Vec::new();
    for p in [
        Preset::DeepCuts,
        Preset::FastLossless,
        Preset::Short,
        Preset::Bright,
    ] {
        let q = sql(p);
        let bound = if q.contains("?1") {
            conn.query_row(&format!("SELECT count(*) FROM ({q})"), rusqlite::params![user], |r| r.get(0))?
        } else {
            conn.query_row(&format!("SELECT count(*) FROM ({q})"), [], |r| r.get(0))?
        };
        let n: i64 = bound;
        out.push(SmartList {
            id: p.id().to_string(),
            title: p.title().to_string(),
            note: p.note().to_string(),
            count: n,
        });
    }
    Ok(out)
}

/// Track ids, already ordered: deep cuts by how much they were once loved,
/// everything else by how well it fits the question.
pub fn ids(conn: &Connection, user: i64, p: Preset) -> Result<Vec<i64>> {
    let q = sql(p);
    let mut stmt = conn.prepare(&q)?;
    // Only the deep-cuts question mentions the listener; the rest are pure
    // library queries, and binding a parameter nobody asked for is an error.
    let rows = if q.contains("?1") {
        stmt.query_map(rusqlite::params![user], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect::<Vec<i64>>()
    } else {
        stmt.query_map([], |r| r.get::<_, i64>(0))?
            .filter_map(|r| r.ok())
            .collect::<Vec<i64>>()
    };
    Ok(rows)
}

fn sql(p: Preset) -> String {
    match p {
        Preset::DeepCuts => "SELECT t.id FROM v_track t
             WHERE NOT EXISTS (
               SELECT 1 FROM play_event e
               WHERE e.track_id = t.id AND e.user_id = ?1 AND e.event IN (0,3,4)
                 AND e.ts > strftime('%s','now') - 30*86400)
             ORDER BY (SELECT count(*) FROM play_event e2
                       WHERE e2.track_id = t.id AND e2.event IN (0,4)) DESC,
                      t.artist, t.album, t.track_no"
            .to_string(),
        Preset::FastLossless => "SELECT t.id FROM v_track t
             JOIN track tr ON tr.id = t.id
             WHERE coalesce(tr.tempo_bpm,0) > 125
               AND t.codec IN ('flac','alac','wav','aiff')
             ORDER BY tr.tempo_bpm DESC, t.artist"
            .to_string(),
        Preset::Short => "SELECT t.id FROM v_track t
             WHERE coalesce(t.duration_ms, 999999) <= 150000
             ORDER BY t.duration_ms ASC"
            .to_string(),
        // `energy` in this library is RMS in dB (negative), not 0..1: comparing
        // it against 0.5 matched nothing. Brightness is compared against the
        // library's own average, which is the honest reference.
        Preset::Bright => "SELECT t.id FROM v_track t
             JOIN track tr ON tr.id = t.id
             WHERE coalesce(tr.mode,0) > 0.05
               AND coalesce(tr.tempo_bpm,0) >= 110
               AND coalesce(tr.brightness,0) >= (SELECT avg(brightness) FROM track)
             ORDER BY tr.brightness DESC, tr.tempo_bpm DESC"
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_an_answer_for_its_name() {
        for p in [
            Preset::DeepCuts,
            Preset::FastLossless,
            Preset::Short,
            Preset::Bright,
        ] {
            assert!(!p.title().is_empty());
            assert_eq!(Preset::parse(p.id()), Some(p));
        }
        assert_eq!(Preset::parse("nonsense"), None);
    }

    #[test]
    fn the_short_list_only_contains_short_tracks() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIEW v_track AS
               SELECT id, CASE WHEN id=1 THEN 100000 ELSE 300000 END AS duration_ms,
                      'flac' AS codec, 'a' AS artist, 'x' AS album, 1 AS track_no, 2020 AS year
               FROM (SELECT 1 AS id UNION ALL SELECT 2);",
        )
        .unwrap();
        let ids = ids(&conn, 1, Preset::Short).unwrap();
        assert_eq!(ids, vec![1]);
    }
}
