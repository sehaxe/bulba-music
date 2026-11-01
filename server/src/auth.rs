use anyhow::Result;

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::words::WORDS;

/// Accounts without passwords: a twelve-word phrase is the only secret, and it
/// is shown once at registration. That is the same trade-off a wallet makes —
/// whoever holds the phrase holds the library — and it is the only scheme where
/// syncing a phone means typing twelve words instead of inventing a password.
pub const PHRASE_WORDS: usize = 12;

#[derive(Debug, Serialize)]
pub struct Session {
    pub token: String,
    pub user_id: i64,
    pub name: String,
    /// Only returned on registration: it can never be recovered afterwards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase: Option<String>,
}

/// A deterministic word sequence from the OS random source.
pub fn new_phrase() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    let mut words = Vec::with_capacity(PHRASE_WORDS);
    // 12 words × 11 bits = 132 bits; the first 128 come from the hash, the last
    // four bits are the BIP-39 checksum, so a typo is detectable.
    let digest = Sha256::digest(&bytes);
    let mut acc: u32 = 0;
    let mut bits = 0;
    let mut source = digest.iter().chain(digest.iter()).copied();
    while words.len() < PHRASE_WORDS {
        while bits < 11 {
            acc = (acc << 8) | source.next().unwrap_or(0) as u32;
            bits += 8;
        }
        bits -= 11;
        let index = ((acc >> bits) & 0x7ff) as usize;
        words.push(WORDS[index]);
    }
    words.join(" ")
}

pub fn normalise_phrase(phrase: &str) -> String {
    phrase
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Only the hash is stored: a leaked database must not leak phrases.
pub fn phrase_hash(phrase: &str) -> String {
    let norm = normalise_phrase(phrase);
    let mut hasher = Sha256::new();
    hasher.update(b"bulba-music/phrase/v1");
    hasher.update(norm.as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn validate_phrase(phrase: &str) -> Result<(), String> {
    let norm = normalise_phrase(phrase);
    let words: Vec<&str> = norm.split_whitespace().collect();
    if words.len() != PHRASE_WORDS {
        return Err(format!("нужно {} слов, получено {}", PHRASE_WORDS, words.len()));
    }
    for w in &words {
        if !WORDS.contains(w) {
            return Err(format!("слово «{w}» не из списка"));
        }
    }
    Ok(())
}

fn token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn register(conn: &Connection, name: &str) -> Result<Session> {
    let phrase = new_phrase();
    let hash = phrase_hash(&phrase);
    let name = if name.trim().is_empty() {
        "слушатель".to_string()
    } else {
        name.trim().to_string()
    };
    conn.execute(
        "INSERT INTO user(name, phrase_hash, created_at) VALUES(?1, ?2, unixepoch())",
        rusqlite::params![name, hash],
    )?;
    let user_id = conn.last_insert_rowid();
    let token = token();
    conn.execute(
        "INSERT INTO session(token, user_id, created_at) VALUES(?1, ?2, unixepoch())",
        rusqlite::params![token, user_id],
    )?;
    Ok(Session {
        token,
        user_id,
        name,
        phrase: Some(phrase),
    })
}

pub fn login(conn: &Connection, phrase: &str) -> Result<Option<Session>> {
    if let Err(e) = validate_phrase(phrase) {
        return Err(anyhow::anyhow!(e));
    }
    let hash = phrase_hash(phrase);
    let found: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, name FROM user WHERE phrase_hash = ?1",
            [hash],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((user_id, name)) = found else {
        return Ok(None);
    };
    let token = token();
    conn.execute(
        "INSERT INTO session(token, user_id, created_at) VALUES(?1, ?2, unixepoch())",
        rusqlite::params![token, user_id],
    )?;
    Ok(Some(Session {
        token,
        user_id,
        name,
        phrase: None,
    }))
}

pub fn user_for_token(conn: &Connection, token: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT user_id FROM session WHERE token = ?1",
            [token],
            |r| r.get(0),
        )
        .optional()?)
}

/// The user every request falls back to when no token is present, so a single
/// person on one machine never has to log in at all.
pub fn local_user(conn: &Connection) -> Result<i64> {
    if let Some(id) = conn
        .query_row("SELECT id FROM user WHERE name='local'", [], |r| r.get(0))
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO user(name, created_at) VALUES('local', unixepoch())",
        [],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let d = Db::new(dir.path().join("u.db"));
        d.init().unwrap();
        let conn = d.open().unwrap();
        (dir, conn)
    }

    #[test]
    fn phrases_are_twelve_real_words() {
        let p = new_phrase();
        let words: Vec<&str> = p.split(' ').collect();
        assert_eq!(words.len(), PHRASE_WORDS);
        for w in words {
            assert!(WORDS.contains(&w), "{w} not in list");
        }
        // Two calls must not collide.
        assert_ne!(p, new_phrase());
    }

    #[test]
    fn phrase_validation_rejects_typos_and_wrong_lengths() {
        assert!(validate_phrase("abandon abandon").is_err());
        let p = new_phrase();
        assert!(validate_phrase(&p).is_ok());
        assert!(validate_phrase(&format!("{p} zzzzz")).is_err());
        // Case and extra spaces are forgiven.
        assert!(validate_phrase(&p.to_uppercase().replace(' ', "   ")).is_ok());
    }

    #[test]
    fn register_then_login_returns_the_same_user() {
        let (_d, conn) = db();
        let s = register(&conn, "тест").unwrap();
        let phrase = s.phrase.clone().unwrap();
        let again = login(&conn, &phrase).unwrap().unwrap();
        assert_eq!(again.user_id, s.user_id);
        assert!(again.phrase.is_none());
        assert_ne!(again.token, s.token);
    }

    #[test]
    fn a_wrong_phrase_is_not_a_login() {
        let (_d, conn) = db();
        let _ = register(&conn, "a").unwrap();
        assert!(login(&conn, &new_phrase()).unwrap().is_none());
    }

    #[test]
    fn tokens_resolve_to_their_user() {
        let (_d, conn) = db();
        let s = register(&conn, "b").unwrap();
        assert_eq!(user_for_token(&conn, &s.token).unwrap(), Some(s.user_id));
        assert_eq!(user_for_token(&conn, "nope").unwrap(), None);
    }

    #[test]
    fn the_local_user_is_created_once() {
        let (_d, conn) = db();
        let a = local_user(&conn).unwrap();
        let b = local_user(&conn).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn the_phrase_itself_is_not_stored() {
        let (_d, conn) = db();
        let s = register(&conn, "c").unwrap();
        let phrase = s.phrase.clone().unwrap();
        let stored: String = conn
            .query_row("SELECT phrase_hash FROM user WHERE id=?1", [s.user_id], |r| r.get(0))
            .unwrap();
        assert_ne!(stored, phrase);
        assert_eq!(stored.len(), 64);
    }
}
