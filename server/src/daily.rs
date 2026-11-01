use anyhow::Result;
use rusqlite::Connection;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Daily mixes are a product decision, not a query: four numbered playlists
/// that stay the same for a day, each with its own character. They are built by
/// clustering the library in embedding space and naming each cluster by its
/// tempo and brightness.
pub const SLOTS: usize = 4;

#[derive(Debug, Clone)]
pub struct DailyMix {
    pub slot: i64,
    pub title: String,
    pub note: String,
    pub ids: Vec<i64>,
}

/// One track: id, embedding, tempo in BPM, spectral brightness.
type Item = (i64, Vec<f32>, f32, f32);

pub fn day_key() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("d{}", secs / 86_400)
}

fn seeded(n: u64) -> u64 {
    let mut h = DefaultHasher::new();
    n.hash(&mut h);
    h.finish()
}

/// Ensures today's mixes exist. Cheap to call on every request: it is a single
/// count until the day rolls over.
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

    let vectors: Vec<Item> = {
        let mut stmt = conn.prepare(
            "SELECT e.track_id, e.vec, coalesce(t.tempo_bpm, 0), coalesce(t.brightness, 0)
             FROM embed e JOIN track t ON t.id = e.track_id
             WHERE e.model = ?1",
        )?;
        stmt.query_map([model], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                crate::features::from_blob(&r.get::<_, Vec<u8>>(1)?),
                r.get::<_, f64>(2)? as f32,
                r.get::<_, f64>(3)? as f32,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect()
    };

    let mixes = build(&vectors, &day);
    for mix in mixes {
        for (position, id) in mix.ids.iter().enumerate() {
            conn.execute(
                "INSERT OR REPLACE INTO daily_mix(day, slot, position, track_id, title, note)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    day,
                    mix.slot,
                    position as i64,
                    id,
                    mix.title,
                    mix.note
                ],
            )?;
        }
    }
    Ok(())
}

pub fn load(conn: &Connection) -> Result<Vec<DailyMix>> {
    let day = day_key();
    let mut stmt = conn.prepare(
        "SELECT slot, title, note, track_id FROM daily_mix
         WHERE day=?1 ORDER BY slot, position",
    )?;
    let rows: Vec<(i64, String, String, i64)> = stmt
        .query_map([&day], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let mut out: Vec<DailyMix> = Vec::new();
    for (slot, title, note, id) in rows {
        match out.last_mut() {
            Some(m) if m.slot == slot => m.ids.push(id),
            _ => out.push(DailyMix {
                slot,
                title,
                note,
                ids: vec![id],
            }),
        }
    }
    Ok(out)
}

fn build(vectors: &[Item], day: &str) -> Vec<DailyMix> {
    if vectors.is_empty() {
        return vec![];
    }
    if vectors.len() < 6 {
        let ids = vectors.iter().map(|(id, _, _, _)| *id).collect();
        return vec![DailyMix {
            slot: 1,
            title: "подборка 1".into(),
            note: "вся библиотека".into(),
            ids,
        }];
    }

    let k = SLOTS.min(vectors.len() / 3).max(2);
    let assignments = kmeans(vectors, k, seeded(day.len() as u64 ^ 0x5EED));

    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); k];
    for (i, cluster) in assignments.iter().enumerate() {
        groups[*cluster].push(i);
    }
    groups.retain(|g| !g.is_empty());
    groups.sort_by_key(|g| std::cmp::Reverse(g.len()));

    let mut mixes = Vec::new();
    for (slot, group) in groups.iter().enumerate() {
        let mut items: Vec<&Item> = group.iter().map(|i| &vectors[*i]).collect();
        // An energy ramp inside each mix: slow and dark first.
        items.sort_by(|a, b| {
            a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
        });
        let ids = items.iter().map(|(id, _, _, _)| *id).collect();
        let (tempo, bright) = character(&items);
        mixes.push(DailyMix {
            slot: (slot + 1) as i64,
            title: format!("подборка {}", slot + 1),
            note: format!("{tempo} · {bright} · {} треков", items.len()),
            ids,
        });
    }
    mixes
}

/// Two axes humans actually hear: tempo and spectral brightness.
fn character(items: &[&Item]) -> (&'static str, &'static str) {
    let n = items.len().max(1) as f32;
    let tempo = items.iter().map(|(_, _, t, _)| *t).sum::<f32>() / n;
    let bright = items.iter().map(|(_, _, _, b)| *b).sum::<f32>() / n;
    let tempo_name = if tempo < 95.0 {
        "медленное"
    } else if tempo < 140.0 {
        "средний темп"
    } else {
        "быстрое"
    };
    let bright_name = if bright < 0.10 {
        "тёмное"
    } else if bright < 0.22 {
        "тёплое"
    } else {
        "яркое"
    };
    (tempo_name, bright_name)
}

fn kmeans(vectors: &[Item], k: usize, seed: u64) -> Vec<usize> {
    let dim = vectors[0].1.len();
    let mut centroids: Vec<Vec<f32>> = (0..k)
        .map(|i| {
            let idx = (seeded(seed ^ i as u64) as usize) % vectors.len();
            vectors[idx].1.clone()
        })
        .collect();

    let mut assignments = vec![0usize; vectors.len()];
    for _ in 0..16 {
        let mut changed = false;
        for (i, item) in vectors.iter().enumerate() {
            let mut best = 0;
            let mut best_score = f32::NEG_INFINITY;
            for (c, centroid) in centroids.iter().enumerate() {
                let score = crate::features::cosine(&item.1, centroid);
                if score > best_score {
                    best_score = score;
                    best = c;
                }
            }
            if assignments[i] != best {
                assignments[i] = best;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        let mut sums = vec![vec![0.0f32; dim]; k];
        let mut counts = vec![0usize; k];
        for (i, item) in vectors.iter().enumerate() {
            let c = assignments[i];
            counts[c] += 1;
            for d in 0..dim {
                sums[c][d] += item.1[d];
            }
        }
        for c in 0..k {
            if counts[c] > 0 {
                for d in 0..dim {
                    sums[c][d] /= counts[c] as f32;
                }
                centroids[c] = sums[c].clone();
            }
        }
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, tempo: f32, bright: f32) -> Item {
        item_band(id, tempo, bright, 0)
    }

    fn item_band(id: i64, tempo: f32, bright: f32, band: usize) -> Item {
        let mut v = vec![0.0f32; crate::features::DIM];
        v[band] = 1.0;
        v[55] = tempo / 200.0;
        v[52] = bright;
        (id, v, tempo, bright)
    }

    #[test]
    fn two_distinct_groups_are_separated() {
        let mut data: Vec<Item> = Vec::new();
        for i in 0..6 {
            data.push(item_band(i, 80.0, 0.05, 0));
        }
        for i in 6..12 {
            data.push(item_band(i, 160.0, 0.6, 20));
        }
        let mixes = build(&data, "d1");
        assert_eq!(mixes.len(), 2);
        let slow = mixes.iter().find(|m| m.note.contains("медленное")).unwrap();
        assert!(slow.ids.iter().all(|id| *id < 6));
        let fast = mixes.iter().find(|m| m.note.contains("быстрое")).unwrap();
        assert!(fast.ids.iter().all(|id| *id >= 6));
    }

    #[test]
    fn tiny_libraries_still_produce_one_mix() {
        let data: Vec<Item> = (0..3).map(|i| item(i, 100.0, 0.1)).collect();
        let mixes = build(&data, "d1");
        assert_eq!(mixes.len(), 1);
        assert_eq!(mixes[0].ids.len(), 3);
    }

    #[test]
    fn every_track_appears_exactly_once() {
        let data: Vec<Item> = (0..30)
            .map(|i| item(i, 60.0 + (i as f32 * 7.0), (i as f32 % 5.0) / 10.0))
            .collect();
        let mixes = build(&data, "d1");
        let mut seen = std::collections::HashSet::new();
        for m in &mixes {
            for id in &m.ids {
                assert!(seen.insert(*id), "track {id} appears twice");
            }
        }
        assert_eq!(seen.len(), 30);
    }

    #[test]
    fn mixes_are_ordered_by_tempo() {
        let data: Vec<Item> = (0..8).map(|i| item(i, 60.0 + i as f32 * 10.0, 0.1)).collect();
        let mixes = build(&data, "d1");
        for m in &mixes {
            let tempos: Vec<f32> = m
                .ids
                .iter()
                .map(|id| data.iter().find(|(t, _, _, _)| t == id).unwrap().2)
                .collect();
            assert!(tempos.windows(2).all(|w| w[0] <= w[1]), "{tempos:?}");
        }
    }
}
