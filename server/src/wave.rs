use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

use crate::features::{self, DIM};

/// One suggestion with the sentence that explains it. The reason is not
/// decoration: it is the score that won, written out, so the listener can tell
/// a recommendation that understands them from one that guesses.
#[derive(Debug, Clone, Serialize)]
pub struct WaveItem {
    pub track_id: i64,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Close to the track the wave was started from.
    Seed,
    /// Close to everything the listener rewards.
    Taste,
    /// Never played, but adjacent to the taste.
    Novel,
    /// Same artist as something played a lot.
    Artist,
    /// Played a lot and not recently.
    Return,
}

impl Why {
    fn code(self) -> &'static str {
        match self {
            Why::Seed => "seed",
            Why::Taste => "taste",
            Why::Novel => "novel",
            Why::Artist => "artist",
            Why::Return => "return",
        }
    }
}

pub struct Request<'a> {
    pub user: i64,
    /// Start the wave from this track instead of from the taste alone.
    pub seed: Option<i64>,
    /// Tracks to leave out — the ones already queued or just played.
    pub exclude: &'a [i64],
    pub limit: usize,
    /// 0 leans entirely on what is known, 1 leans entirely on the unknown.
    pub novelty: f64,
    /// Title of the seed track, for the sentence. The id alone cannot be
    /// turned into readable Russian.
    pub seed_title: Option<&'a str>,
}

struct Stats {
    plays: HashMap<i64, i64>,
    last_play: HashMap<i64, i64>,
    skips: HashMap<i64, i64>,
    likes: HashSet<i64>,
}

fn load_stats(conn: &Connection, user: i64) -> Result<Stats> {
    let mut plays = HashMap::new();
    let mut last_play = HashMap::new();
    let mut skips = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT track_id, event, ts FROM play_event WHERE user_id=?1
         ORDER BY ts DESC LIMIT 4000",
    )?;
    let rows = stmt.query_map([user], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
    })?;
    for row in rows.flatten() {
        let (track, event, ts) = row;
        match event {
            0 => {
                *plays.entry(track).or_insert(0) += 1;
                last_play.entry(track).or_insert(ts);
            }
            2 => {
                *skips.entry(track).or_insert(0) += 1;
            }
            4 => {
                *plays.entry(track).or_insert(0) += 1;
            }
            _ => {}
        }
    }
    let mut stmt = conn.prepare("SELECT track_id FROM track_like WHERE user_id=?1")?;
    let likes: HashSet<i64> = stmt
        .query_map([user], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(Stats {
        plays,
        last_play,
        skips,
        likes,
    })
}

/// Embeddings of every track that has one, skipping the embedded model we do
/// not use any more.
fn load_vectors(conn: &Connection) -> Result<HashMap<i64, Vec<f32>>> {
    let mut stmt = conn.prepare("SELECT track_id, vec FROM embed WHERE model=?1")?;
    let rows = stmt.query_map([features::MODEL], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows.flatten() {
        out.insert(row.0, features::from_blob(&row.1));
    }
    Ok(out)
}

fn cos(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for i in 0..a.len().min(b.len()) {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn centroid(vectors: impl Iterator<Item = (Vec<f32>, f64)>) -> Option<Vec<f32>> {
    let mut acc = vec![0.0f64; DIM];
    let mut total = 0.0;
    let mut any = false;
    for (vec, weight) in vectors {
        if weight <= 0.0 {
            continue;
        }
        any = true;
        for (i, v) in vec.iter().enumerate().take(DIM) {
            acc[i] += *v as f64 * weight;
        }
        total += weight;
    }
    if !any || total <= 0.0 {
        return None;
    }
    Some(acc.iter().map(|v| (v / total) as f32).collect())
}

/// A taste built from what the listener rewards, minus what they skip. This is
/// the same shape as `daily::taste_vector` but works on the 64-dim DSP vector,
/// which is what a fresh library actually has.
fn taste_of(
    vectors: &HashMap<i64, Vec<f32>>,
    stats: &Stats,
    artist_of: &HashMap<i64, i64>,
) -> Option<(Vec<f32>, HashMap<i64, f64>)> {
    let mut artist_weight: HashMap<i64, f64> = HashMap::new();
    for (track, plays) in &stats.plays {
        if let Some(artist) = artist_of.get(track) {
            *artist_weight.entry(*artist).or_insert(0.0) += *plays as f64;
        }
    }
    let pairs = vectors.iter().filter_map(|(id, vec)| {
        let plays = *stats.plays.get(id).unwrap_or(&0) as f64;
        let skips = *stats.skips.get(id).unwrap_or(&0) as f64;
        let liked = if stats.likes.contains(id) { 3.0 } else { 0.0 };
        let weight = plays + liked - skips * 0.6;
        if weight > 0.0 {
            Some((vec.clone(), weight))
        } else {
            None
        }
    });
    centroid(pairs).map(|c| (c, artist_weight))
}

fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Rank the library for one listener.
///
/// The formula is deliberately small and explainable:
///
///   score = similarity            (to the seed, or to the taste centroid)
///         + novelty_bonus         (unheard tracks, scaled by `novelty`)
///         - familiarity_penalty   (over-played tracks, unless they are liked)
///         - skip_penalty          (skipped more than played)
///         - recency_penalty       (played in the last day)
///
/// and whichever term contributed most is what the reason says.
pub fn pick(
    conn: &Connection,
    req: &Request<'_>,
    artist_of: &HashMap<i64, i64>,
    name_of_artist: &HashMap<i64, String>,
    title_of: &HashMap<i64, String>,
) -> Result<Vec<WaveItem>> {
    let vectors = load_vectors(conn)?;
    if vectors.is_empty() {
        return Ok(Vec::new());
    }
    let stats = load_stats(conn, req.user)?;
    let excluded: HashSet<i64> = req.exclude.iter().copied().collect();

    let seed_vec = req
        .seed
        .and_then(|id| vectors.get(&id).cloned());
    let taste = taste_of(&vectors, &stats, artist_of).map(|(v, _)| v);

    let base = seed_vec.clone().or_else(|| taste.clone());
    let base = match base {
        Some(b) => b,
        None => {
            // Nothing played yet: any track is a fair start, and the wave will
            // learn from the first skip.
            vectors.values().next().cloned().unwrap_or_default()
        }
    };

    let ts = now();
    let mut scored: Vec<(i64, f64, Why)> = Vec::new();
    for (id, vec) in &vectors {
        if excluded.contains(id) {
            continue;
        }
        let sim = cos(&base, vec) as f64;
        let plays = *stats.plays.get(id).unwrap_or(&0) as f64;
        let skips = *stats.skips.get(id).unwrap_or(&0) as f64;
        let liked = stats.likes.contains(id);

        let heard = plays > 0.0;
        let novelty_bonus = if heard { 0.0 } else { 0.22 * req.novelty };
        let want_return = if heard && !liked {
            // Played a lot and not touched for a week: the wave is allowed to
            // bring it back, but never right after it was skipped.
            let last = *stats.last_play.get(id).unwrap_or(&0);
            let idle_days = (ts - last) as f64 / 86_400.0;
            if skips == 0.0 && idle_days > 7.0 && plays >= 2.0 {
                0.10
            } else {
                0.0
            }
        } else {
            0.0
        };
        let familiarity_penalty = if liked {
            0.0
        } else {
            (plays / 6.0).min(0.9) * (1.0 - req.novelty)
        };
        let skip_penalty = (skips * 0.25).min(1.2);
        let recency_penalty = {
            let last = *stats.last_play.get(id).unwrap_or(&0);
            if last > 0 && ts - last < 86_400 {
                0.35
            } else {
                0.0
            }
        };

        let score = sim + novelty_bonus + want_return
            - familiarity_penalty
            - skip_penalty
            - recency_penalty;

        // The winning term becomes the sentence.
        let why = if seed_vec.is_some() && sim >= 0.55 {
            Why::Seed
        } else if !heard && sim >= 0.35 {
            Why::Novel
        } else if want_return > 0.0 {
            Why::Return
        } else if liked {
            Why::Taste
        } else {
            let artist = artist_of.get(id).copied().unwrap_or(-1);
            let artist_heavy = artist >= 0
                && stats
                    .plays
                    .iter()
                    .filter(|(t, _)| artist_of.get(*t).copied() == Some(artist))
                    .map(|(_, n)| *n as f64)
                    .sum::<f64>()
                    >= 4.0;
            if artist_heavy {
                Why::Artist
            } else {
                Why::Taste
            }
        };
        scored.push((*id, score, why));
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // At most two tracks per artist in one batch: five songs by the same band
    // is a playlist, not a wave.
    // For every candidate, the artist of the most similar track the listener
    // actually rewards — that is what makes a reason specific instead of a
    // global "you like Tame Impala" repeated on every row.
    let mut anchors: Vec<(Vec<f32>, i64)> = Vec::new();
    for (id, vec) in &vectors {
        let plays = *stats.plays.get(id).unwrap_or(&0);
        let liked = stats.likes.contains(id);
        if plays > 0 || liked {
            anchors.push((vec.clone(), artist_of.get(id).copied().unwrap_or(-1)));
        }
    }

    // The artist the listener rewards most, used to make the sentence concrete.
    let mut top_artist: Option<String> = None;
    {
        let mut best = (0.0f64, -1i64);
        for (track, plays) in &stats.plays {
            if let Some(artist) = artist_of.get(track) {
                let total: f64 = stats
                    .plays
                    .iter()
                    .filter(|(t, _)| artist_of.get(*t) == Some(artist))
                    .map(|(_, n)| *n as f64)
                    .sum();
                if total > best.0 {
                    best = (total, *artist);
                }
            }
        }
        top_artist = name_of_artist.get(&best.1).cloned();
    }

    let mut per_artist: HashMap<i64, i32> = HashMap::new();
    // The same recording lives on a dozen compilations; two of them in one
    // batch reads as a bug even when the tags are right.
    let mut seen_song: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for (id, score, why) in scored {
        let nearest_artist = |candidate: &[f32]| -> Option<i64> {
            let mut best = (f32::MIN, -1i64);
            for (vec, artist) in &anchors {
                let sim = cos(candidate, vec);
                if sim > best.0 {
                    best = (sim, *artist);
                }
            }
            (best.1 >= 0).then_some(best.1)
        };
        if out.len() >= req.limit {
            break;
        }
        let artist = artist_of.get(&id).copied().unwrap_or(-1);
        let song = format!(
            "{}|{}",
            title_of
                .get(&id)
                .cloned()
                .unwrap_or_default()
                .to_lowercase(),
            name_of_artist.get(&artist).cloned().unwrap_or_default().to_lowercase()
        );
        if !seen_song.insert(song) {
            continue;
        }
        let count = per_artist.entry(artist).or_insert(0);
        if *count >= 2 {
            continue;
        }
        *count += 1;
        let reason = match why {
            Why::Seed => match req.seed_title {
                Some(title) => format!("рядом с «{title}» по звуку"),
                None => "рядом с этим треком по звуку".to_string(),
            },
            Why::Novel => {
                let near = vectors
                    .get(&id)
                    .and_then(|v| nearest_artist(v))
                    .and_then(|a| name_of_artist.get(&a).cloned())
                    .or_else(|| top_artist.clone());
                match near {
                    Some(name) => format!("новое, но рядом с {name}"),
                    None => "новое, но близко к твоему вкусу".to_string(),
                }
            }
            Why::Return => "ты давно не слышал это".to_string(),
            Why::Artist => {
                let name = name_of_artist
                    .get(&artist)
                    .cloned()
                    .unwrap_or_else(|| "этого артиста".into());
                format!("ты много слушаешь {name}")
            }
            Why::Taste => {
                let near = vectors
                    .get(&id)
                    .and_then(|v| nearest_artist(v))
                    .and_then(|a| name_of_artist.get(&a).cloned())
                    .or_else(|| top_artist.clone());
                match near {
                    Some(name) => format!("похоже на {name}"),
                    None => "похоже на то, что ты слушаешь".to_string(),
                }
            }
        };
        out.push(WaveItem {
            track_id: id,
            score,
            reason,
        });
        // keep the enum used, so the compiler notices if a case is dropped
        debug_assert!(!why.code().is_empty());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn setup() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::new(dir.path().join("w.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        conn.execute("INSERT INTO artist(name,norm) VALUES('A','a')", []).unwrap();
        let a = conn.last_insert_rowid();
        conn.execute("INSERT INTO artist(name,norm) VALUES('B','b')", []).unwrap();
        let b = conn.last_insert_rowid();
        for (i, artist) in [(1, a), (2, a), (3, a), (4, b), (5, b)] {
            conn.execute(
                "INSERT INTO album(title,artist_id,norm) VALUES(?1,?2,?3)",
                rusqlite::params![format!("al{i}"), artist, format!("al{i}")],
            )
            .unwrap();
            let album = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO track(title,album_id,duration_ms,rev,created_at)
                 VALUES(?1,?2,180000,1,0)",
                rusqlite::params![format!("t{i}"), album],
            )
            .unwrap();
        }
        // Two clusters: 1,2,3 point one way, 4,5 the other.
        for (id, payload) in [
            (1i64, 0.9f32),
            (2, 0.8),
            (3, 0.7),
            (4, -0.8),
            (5, -0.9),
        ] {
            let vec = vec![payload; DIM];
            conn.execute(
                "INSERT INTO embed(track_id, model, dim, vec) VALUES(?1,?2,?3,?4)",
                rusqlite::params![id, features::MODEL, DIM as i64, features::to_blob(&vec)],
            )
            .unwrap();
        }
        (dir, conn)
    }

    fn ids(conn: &Connection, seed: i64) -> Vec<i64> {
        let req = Request {
            user: crate::auth::local_user(conn).unwrap(),
            seed: Some(seed),
            exclude: &[],
            limit: 3,
            novelty: 0.3,
            seed_title: None,
        };
        let mut artists = HashMap::new();
        let mut names = HashMap::new();
        for (id, artist, name) in [
            (1i64, 1i64, "A"),
            (2, 1, "A"),
            (3, 1, "A"),
            (4, 2, "B"),
            (5, 2, "B"),
        ] {
            artists.insert(id, artist);
            names.insert(artist, name.to_string());
        }
        pick(conn, &req, &artists, &names, &HashMap::new())
            .unwrap()
            .into_iter()
            .map(|i| i.track_id)
            .collect()
    }

    #[test]
    fn a_seed_pulls_its_own_cluster_first() {
        let (_d, conn) = setup();
        let got = ids(&conn, 1);
        assert!(got.contains(&2) || got.contains(&3));
        assert!(!got.contains(&4), "the far cluster must not lead");
    }

    #[test]
    fn the_seed_itself_is_not_suggested_when_excluded() {
        let (_d, conn) = setup();
        let req = Request {
            user: crate::auth::local_user(&conn).unwrap(),
            seed: Some(1),
            exclude: &[1],
            limit: 5,
            novelty: 0.3,
            seed_title: None,
        };
        let mut artists = HashMap::new();
        for id in 1..=5 {
            artists.insert(id as i64, if id <= 3 { 1 } else { 2 });
        }
        let got: Vec<i64> = pick(&conn, &req, &artists, &HashMap::new(), &HashMap::new())
            .unwrap()
            .into_iter()
            .map(|i| i.track_id)
            .collect();
        assert!(!got.contains(&1));
    }

    #[test]
    fn every_suggestion_explains_itself() {
        let (_d, conn) = setup();
        let req = Request {
            user: crate::auth::local_user(&conn).unwrap(),
            seed: None,
            exclude: &[],
            limit: 4,
            novelty: 0.5,
            seed_title: None,
        };
        let mut artists = HashMap::new();
        for id in 1..=5 {
            artists.insert(id as i64, if id <= 3 { 1 } else { 2 });
        }
        let items = pick(&conn, &req, &artists, &HashMap::new(), &HashMap::new()).unwrap();
        assert!(!items.is_empty());
        for item in items {
            assert!(!item.reason.trim().is_empty(), "reason must not be empty");
        }
    }

    #[test]
    fn no_more_than_two_per_artist() {
        let (_d, conn) = setup();
        let req = Request {
            user: crate::auth::local_user(&conn).unwrap(),
            seed: Some(1),
            exclude: &[],
            limit: 5,
            novelty: 0.0,
            seed_title: None,
        };
        let mut artists = HashMap::new();
        for id in 1..=5 {
            artists.insert(id as i64, 1i64);
        }
        let items = pick(&conn, &req, &artists, &HashMap::new(), &HashMap::new()).unwrap();
        assert!(items.len() <= 2, "same artist repeated: {}", items.len());
    }

    #[test]
    fn an_empty_library_returns_nothing_instead_of_failing() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::new(dir.path().join("e.db"));
        db.init().unwrap();
        let conn = db.open().unwrap();
        let req = Request {
            user: crate::auth::local_user(&conn).unwrap(),
            seed: None,
            exclude: &[],
            limit: 5,
            novelty: 0.3,
            seed_title: None,
        };
        assert!(pick(&conn, &req, &HashMap::new(), &HashMap::new(), &HashMap::new())
            .unwrap()
            .is_empty());
    }
}
