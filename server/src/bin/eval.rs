//! Which descriptor actually knows that two tracks belong together?
//!
//! `dsp-v1` exists because a hand-built 56-number fingerprint was cheap, and the
//! CLAP model exists because a trained one should be better. "Better" has to be
//! a number, so this binary ranks the whole library for every track that was
//! played and asks how often a track by the same artist (or from the same album)
//! lands in the top K. Same-artist is the only ground truth a personal library
//! gives for free, and it is a hard one: it is exactly what the recommender must
//! get right before it can be trusted with an actual record.
//!
//! Run: `cargo run --release --bin eval -- --model dsp-v1`

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use rusqlite::Connection;

fn main() -> Result<()> {
    let mut model = String::from("dsp-v1");
    let mut db_arg: Option<String> = None;
    // The number that makes the rest readable: what a model that knows nothing
    // scores on this same task. With 18 artists a lucky guess is not rare.
    let mut control = false;
    let mut top: usize = 20;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--model" => model = args.next().unwrap_or(model),
            "--top" => top = args.next().and_then(|v| v.parse().ok()).unwrap_or(top),
            "--control" => control = true,
            "--db" => {
                db_arg = Some(args.next().unwrap_or_default());
            }
            _ => {}
        }
    }
    let db_path = db_arg
        .or_else(|| std::env::var("BULBA_DB").ok())
        .unwrap_or_else(|| format!("{}/.local/share/bulba-music/bulba.db", home()));
    let conn = Connection::open(&db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;

    let mut vectors: HashMap<i64, Vec<f32>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT track_id, vec FROM embed WHERE model=?1")?;
        let rows = stmt.query_map([&model], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        for row in rows.flatten() {
            vectors.insert(row.0, from_blob(&row.1));
        }
    }
    // Artist and album per track, and which tracks were ever played.
    let mut artist: HashMap<i64, i64> = HashMap::new();
    let mut album: HashMap<i64, i64> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT t.id, al.artist_id, al.id FROM track t JOIN album al ON al.id = t.album_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows.flatten() {
            artist.insert(row.0, row.1);
            album.insert(row.0, row.2);
        }
    }
    let mut played: HashSet<i64> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT DISTINCT track_id FROM play_event WHERE event IN (0,4)")?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        for r in rows.flatten() {
            played.insert(r);
        }
    }

    if vectors.len() < 10 {
        println!("model={model}: only {} vectors, nothing to evaluate", vectors.len());
        return Ok(());
    }

    let ids: Vec<i64> = vectors.keys().copied().collect();
    let dim = vectors.values().next().map(|v| v.len()).unwrap_or(0);
    let mut same_artist = 0usize;
    let mut same_album = 0usize;
    let mut queries = 0usize;
    let mut mrr_artist = 0.0f64;

    // Only tracks that were actually played are queries: an unheard track has no
    // listener history to defend, and scoring it would flatter the model.
    let queries_ids: Vec<i64> = ids
        .iter()
        .copied()
        .filter(|id| played.contains(id))
        .collect();
    let queries_ids = if queries_ids.is_empty() { ids.clone() } else { queries_ids };

    let mut rng: u64 = 0x9e3779b97f4a7c15;
    for &q in &queries_ids {
        let qv = &vectors[&q];
        let mut ranked: Vec<(i64, f32)> = ids
            .iter()
            .filter(|id| **id != q)
            .map(|id| {
                if control {
                    // xorshift: a deterministic permutation is enough to answer
                    // "what does chance look like here".
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    (*id, (rng >> 40) as f32)
                } else {
                    (*id, dot(qv, &vectors[id]))
                }
            })
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let want_artist = artist.get(&q).copied();
        let want_album = album.get(&q).copied();
        let mut hit_artist = false;
        let mut pos_artist: Option<usize> = None;
        for (i, (id, _)) in ranked.iter().take(top).enumerate() {
            if artist.get(id).copied() == want_artist {
                hit_artist = true;
                pos_artist.get_or_insert(i + 1);
            }
        }
        if let Some(p) = pos_artist {
            mrr_artist += 1.0 / p as f64;
        }
        if hit_artist {
            same_artist += 1;
        }
        if ranked
            .iter()
            .take(top)
            .any(|(id, _)| album.get(id).copied() == want_album)
        {
            same_album += 1;
        }
        queries += 1;
    }

    let n = queries as f64;
    println!(
        "model={model} dim={dim} queries={queries} recall@{}: artist {:.3} album {:.3} mrr_artist {:.3}",
        top,
        same_artist as f64 / n,
        same_album as f64 / n,
        mrr_artist / n
    );
    Ok(())
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn from_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/home/sehaxe".into())
}
