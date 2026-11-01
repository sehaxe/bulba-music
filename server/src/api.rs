use crate::db::{default_user, Db};
use crate::range::parse_range;
use crate::scan;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppState {
    pub db: Db,
    pub library: PathBuf,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/scan", post(run_scan))
        .route("/api/tracks", get(list_tracks))
        .route("/api/tracks/{id}", get(get_track))
        .route("/api/tracks/{id}/stream", get(stream))
        .route("/api/search", get(search))
        .route("/api/manifest", get(manifest))
        .route("/api/events", post(record_events))
        .with_state(state)
}

#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl ApiError {
    fn not_found(what: &str) -> Self {
        Self(StatusCode::NOT_FOUND, format!("{what} not found"))
    }
    fn forbidden(what: &str) -> Self {
        Self(StatusCode::FORBIDDEN, what.to_string())
    }
    fn internal(e: impl std::fmt::Display) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(e)
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        Self::internal(e)
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::internal(e)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(e)
    }
}

#[derive(Debug, Serialize)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: Option<i64>,
    pub disc: Option<i64>,
    pub track_no: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bits: Option<i64>,
    pub channels: Option<i64>,
    pub bytes: Option<i64>,
    pub master_count: i64,
    pub rev: i64,
}

const TRACK_COLUMNS: &str = "id, title, artist, album, year, disc, track_no, duration_ms,
    codec, bitrate, sample_rate, bits, channels, bytes, master_count, rev";

fn map_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        album: r.get(3)?,
        year: r.get(4)?,
        disc: r.get(5)?,
        track_no: r.get(6)?,
        duration_ms: r.get(7)?,
        codec: r.get(8)?,
        bitrate: r.get(9)?,
        sample_rate: r.get(10)?,
        bits: r.get(11)?,
        channels: r.get(12)?,
        bytes: r.get(13)?,
        master_count: r.get(14)?,
        rev: r.get(15)?,
    })
}

async fn health(State(st): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let stats = tokio::task::spawn_blocking(move || -> anyhow::Result<(i64, i64, i64, i64)> {
        let conn = db.open()?;
        let tracks: i64 = conn.query_row("SELECT count(*) FROM track", [], |r| r.get(0))?;
        let artists: i64 = conn.query_row("SELECT count(*) FROM artist", [], |r| r.get(0))?;
        let albums: i64 = conn.query_row("SELECT count(*) FROM album", [], |r| r.get(0))?;
        Ok((tracks, artists, albums, crate::db::rev(&conn)?))
    })
    .await??;
    Ok(Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "tracks": stats.0,
        "artists": stats.1,
        "albums": stats.2,
        "rev": stats.3,
        "library": st.library,
    })))
}

async fn run_scan(State(st): State<AppState>) -> Result<Json<scan::ScanReport>, ApiError> {
    let db = st.db.clone();
    let library = st.library.clone();
    let report = tokio::task::spawn_blocking(move || scan::scan(&db, &library)).await??;
    Ok(Json(report))
}

#[derive(Debug, Deserialize)]
struct Page {
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_tracks(
    State(st): State<AppState>,
    Query(page): Query<Page>,
) -> Result<Json<Vec<Track>>, ApiError> {
    let limit = page.limit.unwrap_or(200).clamp(1, 2000);
    let offset = page.offset.unwrap_or(0).max(0);
    let rows = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Track>> {
        let conn = st.db.open()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM v_track
             ORDER BY artist, album, disc, track_no, title
             LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map([limit, offset], map_track)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(rows))
}

async fn get_track(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Track>, ApiError> {
    let track = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<Track>> {
        let conn = st.db.open()?;
        let mut stmt = conn.prepare(&format!("SELECT {TRACK_COLUMNS} FROM v_track WHERE id=?1"))?;
        Ok(stmt.query_row([id], map_track).ok())
    })
    .await??;
    track.map(Json).ok_or_else(|| ApiError::not_found("track"))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<i64>,
}

async fn search(
    State(st): State<AppState>,
    Query(sq): Query<SearchQuery>,
) -> Result<Json<Vec<Track>>, ApiError> {
    // Wrap each term in quotes so user input cannot inject FTS operators.
    let expression: String = sq
        .q
        .split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");
    if expression.is_empty() {
        return Ok(Json(vec![]));
    }
    let limit = sq.limit.unwrap_or(50).clamp(1, 500);
    let rows = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Track>> {
        let conn = st.db.open()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM v_track v
             JOIN track_fts f ON f.rowid = v.id
             WHERE track_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
            TRACK_COLUMNS
                .split(',')
                .map(|c| format!("v.{}", c.trim()))
                .collect::<Vec<_>>()
                .join(", ")
        ))?;
        let rows = stmt.query_map([&expression, &limit.to_string()], map_track)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct SinceQuery {
    since: Option<i64>,
}

#[derive(Debug, Serialize)]
struct Manifest {
    rev: i64,
    tracks: Vec<Track>,
    removed: Vec<i64>,
}

async fn manifest(
    State(st): State<AppState>,
    Query(q): Query<SinceQuery>,
) -> Result<Json<Manifest>, ApiError> {
    let since = q.since.unwrap_or(0).max(0);
    let manifest = tokio::task::spawn_blocking(move || -> anyhow::Result<Manifest> {
        let conn = st.db.open()?;
        let rev = crate::db::rev(&conn)?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM v_track WHERE rev > ?1 ORDER BY rev"
        ))?;
        let tracks: Vec<Track> = stmt
            .query_map([since], map_track)?
            .filter_map(|r| r.ok())
            .collect();
        let mut stmt = conn.prepare("SELECT track_id FROM tombstones WHERE rev > ?1 ORDER BY rev")?;
        let removed: Vec<i64> = stmt
            .query_map([since], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(Manifest { rev, tracks, removed })
    })
    .await??;
    Ok(Json(manifest))
}

#[derive(Debug, Deserialize)]
struct NewEvent {
    track_id: i64,
    session_id: i64,
    event: i64,
    position_ms: i64,
    position_ratio: f64,
    source: i64,
    prev_track_id: Option<i64>,
    device: i64,
    offline: bool,
    quality: i64,
    client_id: Option<String>,
    ts: Option<i64>,
}

async fn record_events(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(events): Json<Vec<NewEvent>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // The allowlist check runs before anything is written, so a stranger cannot
    // use this endpoint to probe which track ids exist.
    let login = headers
        .get("tailscale-user-login")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let db = st.db.clone();
    let user = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<i64>> {
        let conn = db.open()?;
        Ok(match &login {
            None => Some(default_user(&conn)?),
            Some(l) => conn
                .query_row("SELECT id FROM user WHERE ts_login=?1", [l], |r| r.get(0))
                .ok(),
        })
    })
    .await??;
    let Some(user) = user else {
        return Err(ApiError::forbidden("user not in allowlist"));
    };

    if events.is_empty() {
        return Ok(Json(json!({ "accepted": 0 })));
    }
    let accepted = tokio::task::spawn_blocking(move || -> anyhow::Result<usize> {
        let mut conn = st.db.open()?;
        let tx = conn.transaction()?;
        for e in &events {
            tx.execute(
                "INSERT INTO play_event
                  (ts, user_id, track_id, session_id, event, position_ms, position_ratio,
                   source, prev_track_id, device, offline, quality, client_id)
                 VALUES (coalesce(?1, unixepoch()), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    e.ts,
                    user,
                    e.track_id,
                    e.session_id,
                    e.event,
                    e.position_ms,
                    e.position_ratio,
                    e.source,
                    e.prev_track_id,
                    e.device,
                    e.offline as i64,
                    e.quality,
                    e.client_id,
                ],
            )?;
        }
        tx.commit()?;
        Ok(events.len())
    })
    .await??;
    Ok(Json(json!({ "accepted": accepted })))
}

fn mime_for(codec: &str) -> &'static str {
    match codec {
        "flac" => "audio/flac",
        "mp3" => "audio/mpeg",
        "m4a" | "alac" => "audio/mp4",
        "aac" => "audio/aac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        "aiff" | "aif" => "audio/aiff",
        _ => "application/octet-stream",
    }
}

async fn stream(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let found = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
        let conn = st.db.open()?;
        Ok(conn
            .query_row(
                "SELECT m.path, m.codec
                 FROM track_master tm JOIN master m ON m.id = tm.master_id
                 WHERE tm.track_id = ?1
                 ORDER BY (m.codec = 'flac') DESC, m.bytes DESC
                 LIMIT 1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .ok())
    })
    .await??;
    let (path, codec) = found.ok_or_else(|| ApiError::not_found("track"))?;

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| ApiError::not_found("file"))?;
    let len = file.metadata().await.map_err(|_| ApiError::not_found("file"))?.len();

    let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let range = range_header.and_then(|v| parse_range(v, len));

    // A syntactically valid single range that cannot be satisfied must answer 416,
    // not fall back to a full 200 (RFC 9110 §14.2).
    if range.is_none()
        && range_header.is_some_and(|v| v.starts_with("bytes=") && !v.contains(','))
    {
        return Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{len}"))
            .body(axum::body::Body::empty())
            .map_err(ApiError::internal);
    }

    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, mime_for(&codec))
        .header(header::ACCEPT_RANGES, "bytes");

    let body = match range {
        Some(r) => {
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            let mut f = file;
            f.seek(std::io::SeekFrom::Start(r.start)).await?;
            builder = builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", r.start, r.end, len),
                )
                .header(header::CONTENT_LENGTH, r.len());
            axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(f.take(r.len())))
        }
        None => {
            builder = builder.header(header::CONTENT_LENGTH, len);
            axum::body::Body::from_stream(tokio_util::io::ReaderStream::new(file))
        }
    };

    let mut resp = builder.body(body).map_err(ApiError::internal)?;
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=3600"),
    );
    Ok(resp)
}
