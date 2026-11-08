//! Talking to the bulba server. One blocking client, one base URL: the desktop
//! app is a viewer of the same library the phone sees, so there is no second
//! store of truth to keep in sync.

use anyhow::{Context as _, Result, anyhow};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const DEFAULT_SERVER: &str = "http://127.0.0.1:8422";

#[derive(Clone)]
pub struct Api {
    pub base: String,
    pub token: Option<String>,
    agent: ureq::Agent,
}

fn flag(v: &serde_json::Value) -> bool {
    v.as_bool().unwrap_or_else(|| v.as_i64().unwrap_or(0) != 0)
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq)]
pub struct Track {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub album_id: i64,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub bitrate: Option<i64>,
    #[serde(default)]
    pub sample_rate: Option<i64>,
    #[serde(default)]
    pub bits: Option<i64>,
    #[serde(default)]
    pub track_no: Option<i64>,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub rev: i64,
    #[serde(default)]
    pub loudness: Option<f64>,
    #[serde(default)]
    pub camelot: Option<String>,
    #[serde(default)]
    pub likes: Option<i64>,
}

impl Track {
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.duration_ms.max(0) as u64)
    }

    /// `flac 16/44`, `mp3 320` — exactly what the file is.
    pub fn format(&self) -> String {
        match self.codec.as_deref() {
            Some("flac") => format!(
                "flac {}/{}",
                self.bits.unwrap_or(0),
                self.sample_rate.unwrap_or(0) / 1000
            ),
            Some(c) if c == "mp3" => format!("mp3 {}", self.bitrate.unwrap_or(0)),
            Some(c) => c.to_string(),
            None => "---".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AlbumSummary {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub artist_id: i64,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    pub tracks: i64,
    #[serde(default)]
    pub plays: i64,
    #[serde(default)]
    pub has_cover: serde_json::Value,
}

impl AlbumSummary {
    pub fn cover(&self) -> bool {
        flag(&self.has_cover)
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AlbumPage {
    pub album: AlbumSummary,
    #[serde(default)]
    pub tracks: Vec<AlbumTrack>,
    #[serde(default)]
    pub total_ms: i64,
    #[serde(default)]
    pub plays: i64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct AlbumTrack {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub track_no: Option<i64>,
    #[serde(default)]
    pub disc: Option<i64>,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub bits: Option<i64>,
    #[serde(default)]
    pub sample_rate: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ArtistSummary {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub albums: i64,
    #[serde(default)]
    pub tracks: i64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct WaveTrack {
    #[serde(flatten)]
    pub track: Track,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub score: f64,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct WaveReply {
    #[serde(default)]
    pub tracks: Vec<WaveTrack>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub taste_ready: bool,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SearchResults {
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub albums: Vec<AlbumSummary>,
    #[serde(default)]
    pub artists: Vec<ArtistSummary>,
}

/// Where this listener stopped on any device. The server keeps one row per user,
/// so the phone and the desktop see the same point.
#[derive(Clone, Debug, Deserialize, Default)]
pub struct Cursor {
    #[serde(default)]
    pub track_id: i64,
    #[serde(default)]
    pub position_ms: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub track: Option<Track>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct Info {
    #[serde(default)]
    pub stats: serde_json::Value,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct PlayEvent<'a> {
    pub track_id: i64,
    pub kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<i64>,
}

impl Api {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            token: None,
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(std::time::Duration::from_secs(60)))
                .build()
                .new_agent(),
        }
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let body = self.get_text(path)?;
        serde_json::from_str(&body).with_context(|| format!("{path}: bad json"))
    }

    fn get_text(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.base, path);
        let mut req = self.agent.get(&url);
        if let Some(token) = &self.token {
            req = req.header("authorization", &format!("Bearer {token}"));
        }
        let mut res = req
            .call()
            .with_context(|| format!("GET {url}"))?;
        Ok(res.body_mut().read_to_string()?)
    }

    pub fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let url = format!("{}{}", self.base, path);
        let mut req = self.agent.get(&url);
        if let Some(token) = &self.token {
            req = req.header("authorization", &format!("Bearer {token}"));
        }
        let mut res = req.call().with_context(|| format!("GET {url}"))?;
        Ok(res.body_mut().read_to_vec()?)
    }

    pub fn post<T: DeserializeOwned>(&self, path: &str, body: &impl Serialize) -> Result<T> {
        let url = format!("{}{}", self.base, path);
        let payload = serde_json::to_string(body)?;
        let mut req = self.agent.post(&url).header("content-type", "application/json");
        if let Some(token) = &self.token {
            req = req.header("authorization", &format!("Bearer {token}"));
        }
        let mut res = req.send(payload.as_str()).with_context(|| format!("POST {url}"))?;
        let text = res.body_mut().read_to_string()?;
        serde_json::from_str(&text).map_err(|e| anyhow!("{path}: {e}"))
    }

    pub fn tracks(&self) -> Result<Vec<Track>> {
        self.get("/api/tracks?limit=20000")
    }

    pub fn albums(&self) -> Result<Vec<AlbumSummary>> {
        self.get("/api/albums?limit=20000")
    }

    pub fn artists(&self) -> Result<Vec<ArtistSummary>> {
        self.get("/api/artists?limit=20000")
    }

    pub fn album(&self, id: i64) -> Result<AlbumPage> {
        self.get(&format!("/api/albums/{id}"))
    }

    pub fn search(&self, q: &str) -> Result<SearchResults> {
        let q = urlencode(q);
        self.get(&format!("/api/search?q={q}&limit=80"))
    }

    pub fn wave(&self, seed: Option<i64>, exclude: &[i64], limit: usize) -> Result<WaveReply> {
        let mut url = format!("/api/recs/wave?novelty=0.35&limit={limit}");
        if let Some(seed) = seed {
            url.push_str(&format!("&seed={seed}"));
        }
        if !exclude.is_empty() {
            let list = exclude
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            url.push_str(&format!("&exclude={list}"));
        }
        self.get(&url)
    }

    pub fn home(&self) -> Result<serde_json::Value> {
        self.get("/api/recs/home")
    }

    pub fn like_ids(&self) -> Result<Vec<i64>> {
        self.get("/api/likes/ids")
    }

    pub fn set_like(&self, id: i64, liked: bool) -> Result<()> {
        if liked {
            let _: serde_json::Value = self.post(&format!("/api/likes/{id}"), &serde_json::json!({}))?;
        } else {
            let url = format!("{}/api/likes/{id}", self.base);
            let mut req = self.agent.delete(&url);
            if let Some(token) = &self.token {
                req = req.header("authorization", &format!("Bearer {token}"));
            }
            req.call().with_context(|| format!("DELETE {url}"))?;
        }
        Ok(())
    }

    pub fn events(&self, events: &[PlayEvent<'_>]) -> Result<()> {
        let _: serde_json::Value = self.post("/api/events", &events)?;
        Ok(())
    }

    pub fn info(&self) -> Result<Info> {
        self.get("/api/info")
    }

    pub fn cursor(&self) -> Result<Option<Cursor>> {
        let text = self.get_text("/api/playback")?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if value.is_null() || value.get("track_id").is_none() {
            return Ok(None);
        }
        Ok(serde_json::from_value(value).ok())
    }

    pub fn put_cursor(&self, track_id: i64, position_ms: i64) -> Result<()> {
        let _: serde_json::Value = self.post(
            "/api/playback",
            &serde_json::json!({ "track_id": track_id, "position_ms": position_ms }),
        )?;
        Ok(())
    }

    pub fn stream_url(&self, id: i64) -> String {
        format!("{}/api/tracks/{id}/stream", self.base)
    }

    pub fn cover_url(&self, album_id: i64, width: u32) -> String {
        format!("{}/api/albums/{album_id}/cover?w={width}", self.base)
    }
}

pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
