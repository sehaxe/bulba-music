//! "Put on something that sounds like this feeling."
//!
//! The text tower of CLAP lives in Python because the server must not link ONNX;
//! so `tools/embed_clap.py --phrases` precomputes a vector for every phrase in
//! `tools/phrases.txt`, and the query is matched to a phrase here.
//!
//! That is a real limitation and it is visible: a request that matches nothing
//! returns nothing, instead of pretending a sentence was turned into a vector. A
//! fixed vocabulary of fifty moods and genres covers the common asks; the honest
//! upgrade is a text encoder at request time, which needs ONNX in the server and
//! is not worth a second runtime for a personal library.

use anyhow::Result;
use rusqlite::Connection;

/// Four letters is enough to match "грустн" / "грустное" / "грустный" without a
/// stemmer, and short enough that "ночь"/"ночное" meet.
const STEM: usize = 4;

fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .map(|w| w.chars().take(STEM).collect::<String>())
        .collect()
}

/// The phrase whose words the query shares most. Ties go to the phrase that is
/// shorter in words — "рок" beats "хард-рок" for "рок", which is what a person
/// means when they say it.
pub fn match_phrase(conn: &Connection, query: &str) -> Result<Option<(String, Vec<f32>)>> {
    let want = tokens(query);
    if want.is_empty() {
        return Ok(None);
    }
    let mut stmt = conn.prepare("SELECT phrase, vec FROM phrase_vec")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
    let mut best: Option<(f64, usize, String, Vec<f32>)> = None;
    for (phrase, blob) in rows.flatten() {
        let have = tokens(&phrase);
        if have.is_empty() || blob.len() < 4 {
            continue;
        }
        let hits = have.iter().filter(|w| want.contains(w)).count() as f64;
        if hits == 0.0 {
            continue;
        }
        // Overlap relative to the phrase, so a long phrase does not win just by
        // containing more words.
        let score = hits / have.len() as f64;
        let better = match &best {
            None => true,
            Some((s, n, _, _)) => score > *s + 1e-9 || ((score - *s).abs() < 1e-9 && have.len() < *n),
        };
        if better {
            best = Some((
                score,
                have.len(),
                phrase.clone(),
                crate::features::from_blob(&blob),
            ));
        }
    }
    // A single weak hit ("this" in "this is dark") is not a mood.
    Ok(best.filter(|(score, _, _, _)| *score >= 0.5).map(|(_, _, p, v)| (p, v)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_meets_its_phrase_through_four_letters() {
        assert_eq!(tokens("грустный джаз на ночь"), vec!["грус", "джаз", "ночь"]);
    }

    #[test]
    fn a_short_phrase_wins_over_a_longer_one_that_contains_it() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE phrase_vec(phrase TEXT PRIMARY KEY, vec BLOB NOT NULL);",
        )
        .unwrap();
        let v = crate::features::to_blob(&[1.0f32, 0.0]);
        conn.execute("INSERT INTO phrase_vec VALUES('рок', ?1)", [v.clone()])
            .unwrap();
        conn.execute("INSERT INTO phrase_vec VALUES('хард-рок', ?1)", [v])
            .unwrap();
        let (phrase, _) = match_phrase(&conn, "рок").unwrap().unwrap();
        assert_eq!(phrase, "рок");
    }
}
