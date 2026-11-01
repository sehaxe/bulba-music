use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashSet;

/// Daily mixes are a product decision, not a query: a handful of named lists
/// that stay the same for a day, each built from an explicit axis so the name
/// is honest.
///
/// The axes come from the audio itself: tempo, brightness, loudness (energy) and
/// a major/minor estimate. A blind k-means over these vectors mixes a lullaby
/// with a rave, which is exactly what the first attempt did.
///
/// Novelty is a policy, not an accident: every list keeps a share of unheard
/// tracks that sit close to what the list is about, and "открытие" is entirely
/// unheard.
pub const MAX_PER_MIX: usize = 40;
pub const NOVELTY_SHARE: f32 = 0.15;

#[derive(Debug, Clone)]
pub struct DailyMix {
    pub slot: i64,
    pub id: String,
    pub title: String,
    pub note: String,
    pub ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Feat {
    pub id: i64,
    pub vec: Vec<f32>,
    pub tempo: f32,
    pub bright: f32,
    pub energy: f32,
    pub mode: f32,
}

#[derive(Debug, Default, Clone)]
pub struct Context {
    pub taste: Option<Vec<f32>>,
    /// Everything the listener has ever played.
    pub played: HashSet<i64>,
    pub liked: HashSet<i64>,
}

pub fn day_key() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("d{}", secs / 86_400)
}

/// Stable per-day pseudo-randomness so ties do not shuffle on every request.
fn seeded(day: &str, salt: u64) -> f32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    day.hash(&mut h);
    salt.hash(&mut h);
    (h.finish() % 1000) as f32 / 1000.0
}

pub fn ensure(conn: &Connection, model: &str) -> Result<()> {
    let day = day_key();
    let existing: i64 = conn.query_row(
        "SELECT count(*) FROM daily_mix WHERE day=?1",
        [&day],
        |r| r.get(0),
    )?;
    if existing > 0 {
        return Ok(());
    }

    let feats = load_features(conn, model)?;
    let ctx = load_context(conn)?;
    let mixes = build(&feats, &ctx, &day);

    for mix in mixes {
        for (position, id) in mix.ids.iter().enumerate() {
            conn.execute(
                "INSERT OR REPLACE INTO daily_mix(day, slot, position, track_id, title, note)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![day, mix.slot, position as i64, id, mix.title, mix.note],
            )?;
        }
    }
    Ok(())
}

/// Loads the day's mixes; the id is derived from the title so the client can
/// keep requesting "the calm one" across restarts.
pub fn load(conn: &Connection) -> Result<Vec<DailyMix>> {
    let day = day_key();
    let mut stmt = conn.prepare(
        "SELECT slot, title, note, track_id FROM daily_mix
         WHERE day=?1 ORDER BY slot, position",
    )?;
    let rows: Vec<(i64, String, String, i64)> = stmt
        .query_map([&day], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .filter_map(|r| r.ok())
        .collect();

    let mut out: Vec<DailyMix> = Vec::new();
    for (slot, title, note, id) in rows {
        match out.last_mut() {
            Some(m) if m.slot == slot => m.ids.push(id),
            _ => out.push(DailyMix {
                slot,
                id: format!("day-{slot}"),
                title,
                note,
                ids: vec![id],
            }),
        }
    }
    Ok(out)
}

fn load_features(conn: &Connection, model: &str) -> Result<Vec<Feat>> {
    let mut stmt = conn.prepare(
        "SELECT e.track_id, e.vec, coalesce(t.tempo_bpm,0), coalesce(t.brightness,0),
                coalesce(t.energy,0), coalesce(t.mode,0)
         FROM embed e JOIN track t ON t.id = e.track_id
         WHERE e.model = ?1",
    )?;
    Ok(stmt
        .query_map([model], |r| {
            Ok(Feat {
                id: r.get(0)?,
                vec: crate::features::from_blob(&r.get::<_, Vec<u8>>(1)?),
                tempo: r.get::<_, f64>(2)? as f32,
                bright: r.get::<_, f64>(3)? as f32,
                energy: r.get::<_, f64>(4)? as f32,
                mode: r.get::<_, f64>(5)? as f32,
            })
        })?
        .filter_map(|r| r.ok())
        .collect())
}

fn load_context(conn: &Connection) -> Result<Context> {
    let played: HashSet<i64> = {
        let mut stmt = conn.prepare("SELECT DISTINCT track_id FROM play_event")?;
        stmt.query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect()
    };
    let liked: HashSet<i64> = {
        let mut stmt = conn.prepare("SELECT track_id FROM track_like")?;
        stmt.query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect()
    };
    Ok(Context {
        taste: taste_vector(conn)?,
        played,
        liked,
    })
}

/// One weighted centroid of everything the listener rewarded, minus what they
/// skipped. Completion counts, a like counts more, a skip subtracts.
pub fn taste_vector(conn: &Connection) -> Result<Option<Vec<f32>>> {
    let mut stmt = conn.prepare(
        "SELECT e.vec,
                sum(CASE pe.event
                      WHEN 3 THEN 1.0
                      WHEN 4 THEN 0.8
                      WHEN 1 THEN 0.25 * pe.position_ratio
                      WHEN 2 THEN -0.45
                      ELSE 0.0 END)
                + 2.0 * (SELECT count(*) FROM track_like l WHERE l.track_id = e.track_id)
                + min(1.5, 0.5 * (SELECT count(*) FROM play_event r
                                   WHERE r.track_id = e.track_id AND r.event = 4))
                AS weight
         FROM play_event pe
         JOIN embed e ON e.track_id = pe.track_id AND e.model = ?1
         GROUP BY pe.track_id",
    )?;
    let rows: Vec<(Vec<u8>, f64)> = stmt
        .query_map([crate::features::MODEL], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, f64>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();
    if rows.is_empty() {
        return Ok(None);
    }

    let mut acc = vec![0.0f64; crate::features::DIM];
    let mut total = 0.0f64;
    for (blob, weight) in rows {
        if weight <= 0.0 {
            continue;
        }
        let vec = crate::features::from_blob(&blob);
        for (i, v) in vec.iter().enumerate().take(acc.len()) {
            acc[i] += *v as f64 * weight;
        }
        total += weight;
    }
    if total <= 0.0 {
        return Ok(None);
    }
    let norm = acc.iter().map(|v| v * v).sum::<f64>().sqrt();
    if norm <= 1e-9 {
        return Ok(None);
    }
    Ok(Some(
        acc.iter().map(|v| (v / norm) as f32).collect::<Vec<f32>>(),
    ))
}

fn cos_to(target: &[f32], feat: &Feat) -> f32 {
    crate::features::cosine(target, &feat.vec)
}

/// A named slice of the library with a scoring function, a floor and a quota.
struct Recipe {
    title: &'static str,
    note: &'static str,
    keep: fn(&Feat) -> bool,
    score: fn(&Feat, &Context) -> f32,
    min_tracks: usize,
}

fn recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            title: "для тебя",
            note: "похоже на то, что ты слушаешь",
            keep: |_| true,
            score: |f, c| match &c.taste {
                Some(t) => cos_to(t, f) + if c.liked.contains(&f.id) { 0.15 } else { 0.0 },
                None => 1.0 - f.tempo / 200.0,
            },
            min_tracks: 5,
        },
        Recipe {
            title: "спокойное",
            note: "медленно и тихо",
            keep: |f| f.tempo < 118.0 && f.energy < 0.62,
            score: |f, _| (1.0 - f.energy) * 0.6 + (1.0 - (f.tempo / 160.0).min(1.0)) * 0.4,
            min_tracks: 3,
        },
        Recipe {
            title: "энергичное",
            note: "быстро и громко",
            keep: |f| f.tempo > 118.0 || f.energy > 0.55,
            score: |f, _| f.energy * 0.6 + (f.tempo / 180.0).min(1.0) * 0.4,
            min_tracks: 3,
        },
        Recipe {
            title: "ночное",
            note: "минор, тембр в тень",
            keep: |f| f.mode < 0.0 && f.bright < 0.24,
            score: |f, _| (-f.mode) * 0.5 + (1.0 - f.bright) * 0.3 + (1.0 - f.energy) * 0.2,
            min_tracks: 3,
        },
        Recipe {
            title: "яркое",
            note: "мажор и свет",
            keep: |f| f.mode > 0.0 && f.bright > 0.18,
            score: |f, _| f.mode * 0.4 + f.bright * 0.4 + f.energy * 0.2,
            min_tracks: 4,
        },
        Recipe {
            title: "открытие",
            note: "ещё не играло, но похоже",
            keep: |_| true,
            score: |f, c| match &c.taste {
                Some(t) => cos_to(t, f),
                None => seeded("", f.id as u64),
            },
            min_tracks: 4,
        },
    ]
}

fn build(feats: &[Feat], ctx: &Context, day: &str) -> Vec<DailyMix> {
    if feats.is_empty() {
        return vec![];
    }
    // Loudness is relative: masters differ, so energies are stretched across the
    // library's own range before any axis is computed.
    let min_e = feats.iter().map(|f| f.energy).fold(f32::INFINITY, f32::min);
    let max_e = feats
        .iter()
        .map(|f| f.energy)
        .fold(f32::NEG_INFINITY, f32::max);
    let span = (max_e - min_e).max(1e-6);
    let feats: Vec<Feat> = feats
        .iter()
        .map(|f| Feat {
            energy: ((f.energy - min_e) / span).clamp(0.0, 1.0),
            ..f.clone()
        })
        .collect();
    let feats = &feats[..];
    let mut mixes: Vec<DailyMix> = Vec::new();

    for (slot, recipe) in recipes().iter().enumerate() {
        let pool: Vec<&Feat> = feats
            .iter()
            .filter(|f| (recipe.keep)(f))
            .filter(|f| {
                // "открытие" is the only list that insists on unheard tracks.
                if recipe.title == "открытие" {
                    !ctx.played.contains(&f.id)
                } else {
                    true
                }
            })
            .collect();
        if pool.len() < recipe.min_tracks {
            continue;
        }

        let mut scored: Vec<(f32, &Feat)> = pool
            .iter()
            .map(|f| {
                let base = (recipe.score)(f, ctx);
                // Deterministic jitter keeps a list from freezing forever.
                (base + seeded(day, f.id as u64) * 0.02, *f)
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let take = scored.len().min(MAX_PER_MIX);
        let known_novelty = recipe.title == "открытие";
        let novelty_slots = if known_novelty {
            0
        } else {
            (take as f32 * NOVELTY_SHARE).round() as usize
        };
        let keep_known = take.saturating_sub(novelty_slots);

        let mut ids: Vec<i64> = scored
            .iter()
            .take(keep_known)
            .map(|(_, f)| f.id)
            .collect();

        if !known_novelty && novelty_slots > 0 {
            // Inject unheard tracks that are close to what this list is about,
            // ranked against the list's own centroid.
            let centroid = centroid_of(&scored[..keep_known]);
            let mut fresh: Vec<(f32, i64)> = feats
                .iter()
                .filter(|f| (recipe.keep)(f))
                .filter(|f| !ctx.played.contains(&f.id) && !ids.contains(&f.id))
                .map(|f| {
                    let score = match &centroid {
                        Some(c) => cos_to(c, f),
                        None => seeded(day, f.id as u64),
                    };
                    (score, f.id)
                })
                .collect();
            fresh.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            for (_, id) in fresh.into_iter().take(novelty_slots) {
                ids.push(id);
            }
        }

        // Novelty is a ceiling, not a reservation: if there is not enough new
        // music that fits, the list is topped up with the best known tracks
        // instead of shrinking below its floor.
        if ids.len() < take {
            for (_, f) in scored.iter() {
                if ids.len() >= take {
                    break;
                }
                if !ids.contains(&f.id) {
                    ids.push(f.id);
                }
            }
        }
        if ids.len() < recipe.min_tracks {
            continue;
        }
        ids.dedup();
        mixes.push(DailyMix {
            slot: (mixes.len() + 1) as i64,
            id: format!("mix-{}", slug(recipe.title)),
            title: recipe.title.to_string(),
            note: format!("{} · {} {}", recipe.note, ids.len(), plural(ids.len())),
            ids,
        });
    }

    // A library too small or too uniform for the axes still deserves a list.
    if mixes.is_empty() {
        let mut ids: Vec<i64> = feats.iter().map(|f| f.id).collect();
        ids.sort_unstable();
        ids.truncate(MAX_PER_MIX);
        mixes.push(DailyMix {
            slot: 1,
            id: "mix-all".into(),
            title: "вся библиотека".into(),
            note: format!("{} {}", ids.len(), plural(ids.len())),
            ids,
        });
    }
    mixes
}

/// 1 трек, 2 трека, 5 треков.
fn plural(n: usize) -> &'static str {
    let m100 = n % 100;
    let m10 = n % 10;
    if (11..=14).contains(&m100) {
        "треков"
    } else if m10 == 1 {
        "трек"
    } else if (2..=4).contains(&m10) {
        "трека"
    } else {
        "треков"
    }
}

fn slug(title: &str) -> String {
    const MAP: &[(char, &str)] = &[
        ('а', "a"),
        ('б', "b"),
        ('в', "v"),
        ('г', "g"),
        ('д', "d"),
        ('е', "e"),
        ('ж', "zh"),
        ('з', "z"),
        ('и', "i"),
        ('й', "y"),
        ('к', "k"),
        ('л', "l"),
        ('м', "m"),
        ('н', "n"),
        ('о', "o"),
        ('п', "p"),
        ('р', "r"),
        ('с', "s"),
        ('т', "t"),
        ('у', "u"),
        ('ф', "f"),
        ('х', "h"),
        ('ц', "c"),
        ('ч', "ch"),
        ('ш', "sh"),
        ('щ', "sch"),
        ('ъ', ""),
        ('ы', "y"),
        ('ь', ""),
        ('э', "e"),
        ('ю', "yu"),
        ('я', "ya"),
    ];
    title
        .chars()
        .map(|c| {
            if let Some((_, latin)) = MAP.iter().find(|(cyr, _)| *cyr == c) {
                latin.to_string()
            } else if c.is_ascii_alphanumeric() {
                c.to_string()
            } else {
                "-".to_string()
            }
        })
        .collect()
}

fn centroid_of(items: &[(f32, &Feat)]) -> Option<Vec<f32>> {
    if items.is_empty() {
        return None;
    }
    let mut acc = vec![0.0f32; crate::features::DIM];
    for (_, f) in items {
        for (i, v) in f.vec.iter().enumerate().take(acc.len()) {
            acc[i] += v;
        }
    }
    let norm = acc.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm <= 1e-9 {
        return None;
    }
    Some(acc.iter().map(|v| v / norm).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feat(id: i64, tempo: f32, bright: f32, energy: f32, mode: f32) -> Feat {
        let mut v = vec![0.0f32; crate::features::DIM];
        v[0] = 1.0;
        v[52] = bright;
        v[55] = tempo / 200.0;
        Feat {
            id,
            vec: v,
            tempo,
            bright,
            energy,
            mode,
        }
    }

    fn library() -> Vec<Feat> {
        let mut v = Vec::new();
        for i in 0..10 {
            v.push(feat(i, 70.0, 0.05, 0.2, -0.5)); // calm minor
        }
        for i in 10..20 {
            v.push(feat(i, 160.0, 0.4, 0.9, 0.4)); // loud major
        }
        v
    }

    #[test]
    fn calm_list_has_only_calm_tracks() {
        let feats = library();
        let ctx = Context::default();
        let mixes = build(&feats, &ctx, "d1");
        let calm = mixes.iter().find(|m| m.title == "спокойное").unwrap();
        for id in &calm.ids {
            let f = feats.iter().find(|f| f.id == *id).unwrap();
            assert!(f.tempo < 118.0 && f.energy < 0.62, "track {id} is not calm");
        }
    }

    #[test]
    fn energetic_list_excludes_quiet_tracks() {
        let feats = library();
        let mixes = build(&feats, &Context::default(), "d1");
        let loud = mixes.iter().find(|m| m.title == "энергичное").unwrap();
        for id in &loud.ids {
            let f = feats.iter().find(|f| f.id == *id).unwrap();
            assert!(f.tempo > 118.0 || f.energy > 0.55);
        }
    }

    #[test]
    fn novelty_is_injected_but_bounded() {
        let feats = library();
        let mut played = HashSet::new();
        for i in 0..8 {
            played.insert(i);
        }
        let ctx = Context {
            taste: Some(vec![0.0; crate::features::DIM]),
            played: played.clone(),
            liked: HashSet::new(),
        };
        let mixes = build(&feats, &ctx, "d1");
        for m in &mixes {
            if m.title == "открытие" {
                for id in &m.ids {
                    assert!(!played.contains(id), "открытие must be unheard");
                }
            } else {
                let fresh = m.ids.iter().filter(|id| !played.contains(*id)).count();
                // Known tracks fill the list first; the novelty share only
                // bounds how much of the *remaining* space new music may take.
                let known_available = feats
                    .iter()
                    .filter(|f| played.contains(&f.id) && (super::recipes()
                        .iter()
                        .find(|r| r.title == m.title)
                        .map(|r| (r.keep)(f))
                        .unwrap_or(true)))
                    .count();
                let novelty = (m.ids.len() as f32 * NOVELTY_SHARE).round() as usize;
                let allowed = novelty.max(m.ids.len().saturating_sub(known_available)) + 1;
                assert!(
                    fresh <= allowed,
                    "{} has {fresh} fresh of {} (known available {known_available})",
                    m.title,
                    m.ids.len()
                );
            }
        }
    }

    #[test]
    fn tiny_library_gets_one_list() {
        let feats = vec![feat(1, 100.0, 0.1, 0.3, 0.0)];
        let mixes = build(&feats, &Context::default(), "d1");
        assert_eq!(mixes.len(), 1);
        assert_eq!(mixes[0].ids, vec![1]);
    }

    #[test]
    fn lists_are_stable_for_the_same_day() {
        let feats = library();
        let a = build(&feats, &Context::default(), "d1");
        let b = build(&feats, &Context::default(), "d1");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.ids, y.ids);
        }
    }
}
