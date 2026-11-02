//! What key a track is in, and therefore which track may follow it.
//!
//! The chroma vector was already computed for every track (`dsp-v1`, twelve bins
//! from C). A key is what makes a crossfade sound intentional instead of
//! accidental: two tracks a fifth apart blend, two a semitone apart fight.
//!
//! Detection is Krumhansl's profile correlation — the oldest working method, and
//! at twelve numbers the only one that fits. It is not a substitute for a trained
//! key detector; it is right often enough to choose between "long blend" and
//! "short blend", which is the only decision being made here.

use anyhow::Result;
use rusqlite::Connection;

/// Krumhansl–Kessler probe-tone profiles, major and minor.
const MAJOR: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// Camelot codes, indexed by key (0 = C … 11 = B).
const MAJOR_CAMELOT: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
const MINOR_CAMELOT: [u8; 12] = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];

pub struct Key {
    /// 0 = C … 11 = B.
    pub key: u8,
    /// 0 = major, 1 = minor.
    pub mode: u8,
    /// How much the winning profile beat the runner-up, 0..1.
    pub conf: f32,
}

impl Key {
    pub fn camelot(&self) -> String {
        let n = if self.mode == 0 {
            MAJOR_CAMELOT[self.key as usize]
        } else {
            MINOR_CAMELOT[self.key as usize]
        };
        format!("{}{}", n, if self.mode == 0 { 'B' } else { 'A' })
    }
}

/// Chroma is L2-normalised before correlation: a quiet track and a loud one have
/// the same key.
pub fn detect(chroma: &[f32; 12]) -> Key {
    let norm = chroma.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm <= 1e-9 {
        return Key {
            key: 0,
            mode: 0,
            conf: 0.0,
        };
    }
    let c: Vec<f32> = chroma.iter().map(|x| x / norm).collect();

    // Pearson, not a raw dot product. Without centring, the profile with the
    // biggest numbers wins regardless of shape — measured on a bare C major
    // triad, which raw correlation calls E minor.
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
    let cm = mean(&c);
    let centered: Vec<f32> = c.iter().map(|x| x - cm).collect();
    let cn = centered.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);

    let mut best = (f32::MIN, 0u8, 0u8);
    let mut second = f32::MIN;
    for mode in 0..2u8 {
        let base = if mode == 0 { MAJOR } else { MINOR };
        let bm = mean(&base);
        for rotation in 0..12usize {
            let mut dot = 0.0f32;
            let mut pn = 0.0f32;
            for i in 0..12 {
                let p = base[(i + 12 - rotation) % 12] - bm;
                dot += centered[i] * p;
                pn += p * p;
            }
            let r = dot / (cn * pn.sqrt().max(1e-9));
            if r > best.0 {
                second = best.0;
                best = (r, rotation as u8, mode);
            } else if r > second {
                second = r;
            }
        }
    }
    let spread = (best.0 - second).max(0.0);
    Key {
        key: best.1,
        mode: best.2,
        conf: (spread * 8.0).clamp(0.0, 1.0),
    }
}

/// 0 = same code, 1 = adjacent on the wheel or the relative key, 2 = anything
/// else. Adjacent means ±1 number on the same letter, which is a fifth away.
pub fn compatibility(a: &str, b: &str) -> u8 {
    if a == b {
        return 0;
    }
    let parse = |s: &str| -> Option<(i32, char)> {
        let mut n = String::new();
        let mut letter = ' ';
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                n.push(ch);
            } else {
                letter = ch;
            }
        }
        Some((n.parse().ok()?, letter))
    };
    let (Some((na, la)), Some((nb, lb))) = (parse(a), parse(b)) else {
        return 2;
    };
    if na == nb && la != lb {
        return 1; // relative major/minor: same code, different letter
    }
    let diff = (na - nb).abs();
    if la == lb && (diff == 1 || diff == 11) {
        return 1;
    }
    2
}

/// How long a blend between these keys should be, in seconds, given what the
/// listener asked for as the maximum.
pub fn blend_seconds(a: Option<&str>, b: Option<&str>, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    match (a, b) {
        (Some(a), Some(b)) => match compatibility(a, b) {
            0 => max,
            1 => (max * 2 / 3).max(4),
            _ => (max / 3).max(2),
        },
        // Unknown key is not a reason to give up the blend, only to be modest.
        _ => (max * 2 / 3).max(4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_c_major_triad_is_c_major() {
        let mut chroma = [0.05f32; 12];
        chroma[0] = 1.0; // C
        chroma[4] = 0.8; // E
        chroma[7] = 0.7; // G
        let k = detect(&chroma);
        assert_eq!(k.key, 0);
        assert_eq!(k.mode, 0);
        assert_eq!(k.camelot(), "8B");
    }

    #[test]
    fn an_a_minor_triad_is_a_minor() {
        let mut chroma = [0.05f32; 12];
        chroma[9] = 1.0; // A
        chroma[0] = 0.8; // C
        chroma[4] = 0.7; // E
        let k = detect(&chroma);
        assert_eq!(k.key, 9);
        assert_eq!(k.mode, 1);
        assert_eq!(k.camelot(), "8A");
    }

    #[test]
    fn the_wheel_is_walkable() {
        assert_eq!(compatibility("8A", "8A"), 0);
        assert_eq!(compatibility("8A", "8B"), 1); // relative major
        assert_eq!(compatibility("8A", "9A"), 1); // a fifth up
        assert_eq!(compatibility("12A", "1A"), 1); // wraps around
        assert_eq!(compatibility("8A", "3A"), 2);
    }

    #[test]
    fn a_clash_gets_a_shorter_blend() {
        assert_eq!(blend_seconds(Some("8A"), Some("8A"), 12), 12);
        assert_eq!(blend_seconds(Some("8A"), Some("9A"), 12), 8);
        assert_eq!(blend_seconds(Some("8A"), Some("3A"), 12), 4);
        assert_eq!(blend_seconds(None, Some("3A"), 12), 8);
    }
}

/// Every track's key, computed from the chroma already stored in `dsp-v1`.
/// Runs on startup for tracks that are missing one: 504 rows is a few
/// milliseconds, and it needs no file access.
pub fn ensure(conn: &Connection) -> Result<usize> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS track_key (
           track_id INTEGER PRIMARY KEY REFERENCES track(id),
           key      INTEGER NOT NULL,
           mode     INTEGER NOT NULL,
           conf     REAL NOT NULL,
           camelot  TEXT NOT NULL
         );",
    )?;
    let mut stmt = conn.prepare(
        "SELECT e.track_id, e.vec FROM embed e
         WHERE e.model = 'dsp-v1' AND e.track_id NOT IN (SELECT track_id FROM track_key)",
    )?;
    let rows: Vec<(i64, Vec<u8>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get::<_, Vec<u8>>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    let mut n = 0;
    for (id, blob) in rows {
        let vec = crate::features::from_blob(&blob);
        if vec.len() < 52 {
            continue;
        }
        let mut chroma = [0.0f32; 12];
        chroma.copy_from_slice(&vec[40..52]);
        let k = detect(&chroma);
        conn.execute(
            "INSERT OR REPLACE INTO track_key(track_id,key,mode,conf,camelot)
             VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params![id, k.key, k.mode, k.conf, k.camelot()],
        )?;
        n += 1;
    }
    Ok(n)
}
