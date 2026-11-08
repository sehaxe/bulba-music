//! What the desktop remembers between runs: where the server is, how loud the
//! blend is, whether levels are matched, and which room the owner prefers.
//!
//! One small JSON file under `$XDG_CONFIG_HOME/bulba-music/desktop.json`. No
//! database, no schema version: an unreadable file is a fresh start, not an
//! error to show.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub server: String,
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default = "default_crossfade")]
    pub crossfade: f64,
    #[serde(default = "default_true")]
    pub replay_gain: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_quality() -> String {
    "flac".into()
}
fn default_crossfade() -> f64 {
    8.0
}
fn default_true() -> bool {
    true
}
fn default_theme() -> String {
    "auto".into()
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            server: String::new(),
            quality: default_quality(),
            crossfade: default_crossfade(),
            replay_gain: true,
            theme: default_theme(),
        }
    }
}

fn path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });
    base.join("bulba-music").join("desktop.json")
}

impl Prefs {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_file_is_defaults_not_a_crash() {
        assert_eq!(Prefs::default().crossfade, 8.0);
        let parsed: Prefs = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.quality, "flac");
        assert!(parsed.replay_gain);
        assert!(serde_json::from_str::<Prefs>("not json").is_err());
    }
}
