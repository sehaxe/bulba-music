//! What the server is willing to hand out.
//!
//! The client can already ask for a tile: `?quality=opus128` renders on demand
//! and caches. That is a *request*. This is the *policy*: one machine might be
//! the only copy of a 300 GB library and be fine sending FLAC over the tailnet,
//! while a phone on a metered link should not be able to pull 28 MB a track even
//! if it asks. The policy caps the request; it never upgrades it.

use anyhow::Result;
use rusqlite::Connection;

use crate::derive::Quality;

pub const KEY: &str = "serve_quality";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serve {
    Flac,
    Opus160,
    Opus128,
    Opus96,
}

impl Serve {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "flac" | "master" => Some(Self::Flac),
            "opus160" => Some(Self::Opus160),
            "opus128" | "opus" => Some(Self::Opus128),
            "opus96" => Some(Self::Opus96),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Flac => "flac",
            Self::Opus160 => "opus160",
            Self::Opus128 => "opus128",
            Self::Opus96 => "opus96",
        }
    }

    /// Higher means more bytes. The cap is a minimum of the two.
    fn rank(self) -> u8 {
        match self {
            Self::Flac => 3,
            Self::Opus160 => 2,
            Self::Opus128 => 1,
            Self::Opus96 => 0,
        }
    }

    pub fn opus(self) -> Option<Quality> {
        match self {
            Self::Flac => None,
            Self::Opus160 => Some(Quality::Opus160),
            Self::Opus128 => Some(Quality::Opus128),
            Self::Opus96 => Some(Quality::Opus96),
        }
    }

    /// The tier actually served for a request. `None` from the client means
    /// "give me the master"; the policy still applies.
    pub fn cap(self, requested: Option<Quality>) -> Option<Quality> {
        match (self.opus(), requested) {
            // The server refuses to send the master at all.
            (Some(policy), _) => {
                let asked = requested.unwrap_or(policy);
                let rank = |q: Quality| match q {
                    Quality::Opus160 => 2,
                    Quality::Opus128 => 1,
                    Quality::Opus96 => 0,
                };
                Some(if rank(asked) <= rank(policy) { asked } else { policy })
            }
            // Master allowed: whatever was asked, including nothing.
            (None, asked) => asked,
        }
    }
}

pub fn get(conn: &Connection) -> Serve {
    conn.query_row("SELECT value FROM meta WHERE key=?1", [KEY], |r| {
        r.get::<_, String>(0)
    })
    .ok()
    .and_then(|v| Serve::parse(&v))
    .unwrap_or(Serve::Flac)
}

pub fn set(conn: &Connection, serve: Serve) -> Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![KEY, serve.tag()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_of_flac_lets_the_client_choose() {
        assert_eq!(Serve::Flac.cap(None), None);
        assert_eq!(Serve::Flac.cap(Some(Quality::Opus96)), Some(Quality::Opus96));
        assert_eq!(Serve::Flac.cap(Some(Quality::Opus160)), Some(Quality::Opus160));
    }

    #[test]
    fn a_policy_of_opus128_never_hands_out_the_master() {
        assert_eq!(Serve::Opus128.cap(None), Some(Quality::Opus128));
        assert_eq!(Serve::Opus128.cap(Some(Quality::Opus96)), Some(Quality::Opus96));
    }

    #[test]
    fn a_client_cannot_ask_above_the_policy() {
        // The phone asks for 160 while the box only agrees to 128.
        assert_eq!(Serve::Opus128.cap(Some(Quality::Opus160)), Some(Quality::Opus128));
        assert_eq!(Serve::Opus96.cap(Some(Quality::Opus96)), Some(Quality::Opus96));
    }

    #[test]
    fn the_default_is_the_master() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        assert_eq!(get(&conn), Serve::Flac);
        set(&conn, Serve::Opus128).unwrap();
        assert_eq!(get(&conn), Serve::Opus128);
        set(&conn, Serve::Opus96).unwrap();
        assert_eq!(get(&conn), Serve::Opus96);
    }
}
