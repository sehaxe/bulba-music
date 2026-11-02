//! What the listener likes, without collapsing it into one average.
//!
//! One centroid is the oldest mistake in a recommender: play a lullaby and a
//! rave, average them, and the result recommends neither. Taste here is a handful
//! of prototypes (k-means over the tracks the listener rewards), plus what they
//! played in the last ten tracks, plus what they play at this hour. Each of those
//! answers a different question, and the caller weights them.
//!
//! Exploration is a bandit with one arm: "how much unheard music should I offer
//! right now". Its reward is whether the unheard tracks that were offered got
//! finished or skipped, measured in the same time-of-day bucket.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::Connection;

/// Events, as the client writes them.
pub const PLAY_START: i64 = 0;
pub const SKIP: i64 = 2;
pub const PLAY_END: i64 = 4;
/// Explicit "not this": stronger than a skip, because it is a decision.
pub const REJECT: i64 = 5;

pub struct Taste {
    /// Best-match prototypes. Empty for a listener who has played nothing.
    pub prototypes: Vec<Vec<f32>>,
    /// The last few tracks, as one decaying vector.
    pub session: Option<Vec<f32>>,
    /// What this hour of the day means to them.
    pub context: Option<Vec<f32>>,
    /// 0..1, how much unheard material to offer right now.
    pub novelty: f64,
    /// Tracks the listener said "not this" to. Never offered again for now.
    pub rejected: HashSet<i64>,
}

impl Taste {
    /// How much a track looks like this taste. Zero when nothing is known.
    pub fn affinity(&self, vec: &[f32], seed: Option<&[f32]>, novelty: f64) -> (f64, Term) {
        let mut acc = 0.0f64;
        let mut weight = 0.0f64;
        let mut best = (-1.0f64, Term::Taste);
        let mut consider = |sim: f64, w: f64, term: Term| {
            acc += sim * w;
            weight += w;
            if sim > best.0 {
                best = (sim, term);
            }
        };
        if let Some(s) = seed {
            consider(cos(s, vec), 0.45, Term::Seed);
        }
        if let Some(p) = self.best_prototype(vec) {
            consider(p, 0.30, Term::Taste);
        }
        if let Some(c) = &self.context {
            consider(cos(c, vec), 0.15, Term::Context);
        }
        if let Some(s) = &self.session {
            consider(cos(s, vec), 0.10, Term::Session);
        }
        let _ = novelty;
        if weight <= 0.0 {
            return (0.0, Term::Taste);
        }
        (acc / weight, best.1)
    }

    fn best_prototype(&self, vec: &[f32]) -> Option<f64> {
        self.prototypes
            .iter()
            .map(|p| cos(p, vec))
            .fold(None, |acc: Option<f64>, v| {
                Some(match acc {
                    Some(a) if a > v => a,
                    _ => v,
                })
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Term {
    Seed,
    Taste,
    Context,
    Session,
    Novel,
    Return,
}

/// The hour bucket a listener is in. Four is enough to separate morning from
/// night without inventing a profile per hour that nothing would fill.
pub fn bucket(hour: i64) -> i64 {
    match hour {
        6..=11 => 0,
        12..=17 => 1,
        18..=23 => 2,
        _ => 3,
    }
}

pub fn load(conn: &Connection, user: i64, vectors: &HashMap<i64, Vec<f32>>) -> Result<Taste> {
    if vectors.is_empty() {
        return Ok(Taste {
            prototypes: Vec::new(),
            session: None,
            context: None,
            novelty: 0.5,
            rejected: HashSet::new(),
        });
    }
    let now = now();
    let hour = (now % 86_400) / 3_600;
    let me = bucket(hour);

    // Every event, newest first, is what both the skip depth and the
    // "was this the first play" question need.
    let mut events: Vec<(i64, i64, f64, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT track_id, event, position_ratio, ts FROM play_event
             WHERE user_id=?1 ORDER BY ts DESC LIMIT 4000",
        )?;
        let rows = stmt.query_map([user], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        for row in rows.flatten() {
            events.push(row);
        }
    }

    let mut played: HashSet<i64> = HashSet::new();
    let mut rejections: HashSet<i64> = HashSet::new();
    // Skip cost by depth: bailing after five seconds is a verdict, bailing near
    // the end is a shrug. Both are negative, the first one much more.
    let mut skip_cost: HashMap<i64, f64> = HashMap::new();
    let mut first_outcome: HashMap<i64, (bool, bool)> = HashMap::new(); // finished, skipped
    for (track, event, ratio, _ts) in &events {
        match *event {
            PLAY_START | PLAY_END => {
                played.insert(*track);
            }
            SKIP => {
                played.insert(*track);
                let depth = 1.0 - ratio.clamp(0.0, 1.0);
                *skip_cost.entry(*track).or_insert(0.0) += 0.2 + depth * 0.8;
                first_outcome.entry(*track).or_insert((false, true)).1 = true;
            }
            REJECT => {
                played.insert(*track);
                rejections.insert(*track);
            }
            _ => {}
        }
    }
    // A rejected track is also a negative, not merely absent.
    for id in &rejections {
        *skip_cost.entry(*id).or_insert(0.0) += 3.0;
    }

    let mut likes: HashSet<i64> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT track_id FROM track_like WHERE user_id=?1")?;
        let rows = stmt.query_map([user], |r| r.get::<_, i64>(0))?;
        for r in rows.flatten() {
            likes.insert(r);
        }
    }
    // "Finished" for the bandit: the track was played to the end at least once.
    for (track, event, ratio, _ts) in &events {
        if *event == PLAY_END || *event == PLAY_START && *ratio > 0.8 {
            first_outcome.entry(*track).or_insert((true, false)).0 = true;
        }
    }

    let positives: Vec<(i64, f64)> = vectors
        .keys()
        .filter_map(|id| {
            if rejections.contains(id) {
                return None;
            }
            let plays = events.iter().filter(|e| e.0 == *id && (e.1 == PLAY_START || e.1 == PLAY_END)).count() as f64;
            let like = if likes.contains(id) { 3.0 } else { 0.0 };
            let weight = (plays + like - skip_cost.get(id).copied().unwrap_or(0.0)).max(0.0);
            (weight > 0.0).then_some((*id, weight))
        })
        .collect();

    let prototypes = if positives.len() >= 2 {
        let k = (positives.len() / 12).clamp(1, 8).max(1);
        kmeans(&positives, vectors, k)
    } else {
        Vec::new()
    };

    // Session: the last ten tracks with a 0.85 decay, because what you put on
    // five minutes ago says more about the next track than what you liked in March.
    let session = {
        let mut acc: Vec<f64> = Vec::new();
        let mut total = 0.0f64;
        for (i, (track, event, _, _)) in events.iter().take(10).enumerate() {
            if *event == REJECT || *event == SKIP {
                continue;
            }
            let Some(v) = vectors.get(track) else { continue };
            if acc.is_empty() {
                acc = vec![0.0; v.len()];
            }
            let w = 0.85f64.powi(i as i32);
            for (j, x) in v.iter().enumerate().take(acc.len()) {
                acc[j] += *x as f64 * w;
            }
            total += w;
        }
        if total > 0.0 {
            Some(acc.iter().map(|v| (v / total) as f32).collect())
        } else {
            None
        }
    };

    // Context: this bucket of the day, from the last 30 days only, so a habit
    // that stopped last year is not still deciding tonight's queue.
    let context = {
        let cutoff = now - 30 * 86_400;
        let mut acc: Vec<f64> = Vec::new();
        let mut total = 0.0f64;
        for (track, event, _, ts) in &events {
            if *ts < cutoff || *event == REJECT || *event == SKIP {
                continue;
            }
            if bucket((ts % 86_400) / 3_600) != me {
                continue;
            }
            let Some(v) = vectors.get(track) else { continue };
            if acc.is_empty() {
                acc = vec![0.0; v.len()];
            }
            for (j, x) in v.iter().enumerate().take(acc.len()) {
                acc[j] += *x as f64;
            }
            total += 1.0;
        }
        if total >= 3.0 {
            Some(acc.iter().map(|v| (v / total) as f32).collect())
        } else {
            None
        }
    };

    // The bandit: among tracks that were new to this listener, how many were
    // finished versus abandoned? A bucket that keeps rejecting the unfamiliar
    // gets less of it.
    let novelty = {
        let mut finished = 0.0;
        let mut abandoned = 0.0;
        for (id, (done, skipped)) in &first_outcome {
            if !vectors.contains_key(id) {
                continue;
            }
            if *done {
                finished += 1.0;
            }
            if *skipped {
                abandoned += 1.0;
            }
        }
        // Prior of 0.5 keeps a listener with ten events from swinging to either
        // extreme on their first three skips.
        let prior_finished = 3.0f64;
        let prior_abandoned = 3.0f64;
        let p = (finished + prior_finished)
            / (finished + abandoned + prior_finished + prior_abandoned);
        (0.15f64 + 0.85 * p).clamp(0.15, 1.0)
    };

    let _ = played;
    Ok(Taste {
        prototypes,
        session,
        context,
        novelty,
        rejected: rejections,
    })
}

/// Farthest-point seeding, then Lloyd. Deterministic on purpose: the same
/// library must produce the same taste model on every restart, or an A/B against
/// "yesterday" measures noise.
fn kmeans(positives: &[(i64, f64)], vectors: &HashMap<i64, Vec<f32>>, k: usize) -> Vec<Vec<f32>> {
    let mut pts: Vec<(Vec<f32>, f64)> = positives
        .iter()
        .filter_map(|(id, w)| vectors.get(id).map(|v| (v.clone(), *w)))
        .collect();
    if pts.is_empty() {
        return Vec::new();
    }
    pts.sort_by_key(|(v, _)| {
        // Stable ordering without hashing: sum of coordinates.
        (v.iter().sum::<f32>() * 1e6) as i64
    });

    let mut centers: Vec<Vec<f32>> = vec![pts[0].0.clone()];
    while centers.len() < k && centers.len() < pts.len() {
        // Farthest from everything chosen so far, weighted by how much the
        // listener likes it: a prototype nobody cares about is not a prototype.
        let (idx, _) = pts
            .iter()
            .enumerate()
            .map(|(i, (v, w))| {
                let nearest = centers.iter().map(|c| cos(c, v) as f64).fold(-1.0, f64::max);
                (i, (1.0 - nearest) * w)
            })
            .fold((0, f64::MIN), |a, b| if b.1 > a.1 { b } else { a });
        centers.push(pts[idx].0.clone());
    }

    for _ in 0..8 {
        let mut sums: Vec<Vec<f64>> = vec![vec![0.0; centers[0].len()]; centers.len()];
        let mut masses = vec![0.0f64; centers.len()];
        for (v, w) in &pts {
            let best = centers
                .iter()
                .enumerate()
                .map(|(i, c)| (i, cos(c, v) as f64))
                .fold((0, f64::MIN), |a, b| if b.1 > a.1 { b } else { a })
                .0;
            for (j, x) in v.iter().enumerate() {
                sums[best][j] += *x as f64 * w;
            }
            masses[best] += w;
        }
        for i in 0..centers.len() {
            if masses[i] > 0.0 {
                centers[i] = sums[i].iter().map(|v| (v / masses[i]) as f32).collect();
            }
        }
    }
    centers
        .into_iter()
        .filter(|c| c.iter().any(|x| x.abs() > 1e-6))
        .collect()
}

pub fn cos(a: &[f32], b: &[f32]) -> f64 {
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += *x as f64 * *y as f64;
        na += *x as f64 * *x as f64;
        nb += *y as f64 * *y as f64;
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

pub fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Vec<f32> {
        vec![x, y]
    }

    #[test]
    fn two_kinds_of_music_do_not_average_into_one() {
        // Four tracks: two quiet, two loud. A centroid would sit at (0,1) and be
        // equally close to both families; prototypes keep them apart.
        let mut vectors = HashMap::new();
        vectors.insert(1, v(1.0, 0.0));
        vectors.insert(2, v(0.9, 0.1));
        vectors.insert(3, v(0.0, 1.0));
        vectors.insert(4, v(0.1, 0.9));
        let positives = vec![(1, 1.0), (2, 1.0), (3, 1.0), (4, 1.0)];
        let centers = kmeans(&positives, &vectors, 2);
        assert_eq!(centers.len(), 2);
        let a = cos(&centers[0], &v(1.0, 0.0)) > cos(&centers[0], &v(0.0, 1.0));
        let b = cos(&centers[1], &v(0.0, 1.0)) > cos(&centers[1], &v(1.0, 0.0));
        assert!(a && b, "each prototype must sit on its own side: {centers:?}");
    }

    #[test]
    fn the_best_prototype_wins_not_the_average() {
        let taste = Taste {
            prototypes: vec![v(1.0, 0.0), v(0.0, 1.0)],
            session: None,
            context: None,
            novelty: 0.5,
            rejected: HashSet::new(),
        };
        let (sim, term) = taste.affinity(&v(0.95, 0.05), None, 0.5);
        assert!(sim > 0.9, "a track matching one prototype should score high, got {sim}");
        assert_eq!(term, Term::Taste);
    }

    #[test]
    fn the_hour_buckets_cover_the_day() {
        assert_eq!(bucket(7), 0);
        assert_eq!(bucket(13), 1);
        assert_eq!(bucket(21), 2);
        assert_eq!(bucket(2), 3);
        let mut seen = HashSet::new();
        for h in 0..24 {
            seen.insert(bucket(h));
        }
        assert_eq!(seen.len(), 4);
    }
}
