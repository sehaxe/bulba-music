use crate::db::Db;
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
        .with_state(state)
}

#[derive(Debug)]
pub struct ApiError(StatusCode, String);

impl ApiError {
    fn not_found(what: &str) -> Self {
        Self(StatusCode::NOT_FOUND, format!("{what} not found"))
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

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(e)
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::internal(e)
    }
}

#[derive(Debug, Serialize)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub year: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub size: i64,
    pub rev: i64,
}

const TRACK_COLUMNS: &str = "id, title, artist, album, album_artist, track_no, disc_no, year,
    duration_ms, codec, bitrate, sample_rate, bit_depth, channels, size, rev";

fn map_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        album: r.get(3)?,
        album_artist: r.get(4)?,
        track_no: r.get(5)?,
        disc_no: r.get(6)?,
        year: r.get(7)?,
        duration_ms: r.get(8)?,
        codec: r.get(9)?,
        bitrate: r.get(10)?,
        sample_rate: r.get(11)?,
        bit_depth: r.get(12)?,
        channels: r.get(13)?,
        size: r.get(14)?,
        rev: r.get(15)?,
    })
}

async fn health(State(st): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let (tracks, rev) = tokio::task::spawn_blocking(move || -> anyhow::Result<(i64, i64)> {
        let conn = db.open()?;
        let tracks: i64 = conn.query_row("SELECT count(*) FROM tracks", [], |r| r.get(0))?;
        Ok((tracks, crate::db::rev(&conn)?))
    })
    .await??;
    Ok(Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "tracks": tracks,
        "rev": rev,
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
            "SELECT {TRACK_COLUMNS} FROM tracks
             ORDER BY coalesce(album_artist, artist), album, disc_no, track_no, title
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
        let mut stmt = conn.prepare(&format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id=?1"))?;
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
    // Wrap each term in quotes so user input can't inject FTS operators.
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
            "SELECT {TRACK_COLUMNS_SELECT} FROM tracks t
             JOIN tracks_fts f ON f.rowid = t.id
             WHERE tracks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        ))?;
        let rows = stmt.query_map([&expression, &limit.to_string()], map_track)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(rows))
}

const TRACK_COLUMNS_SELECT: &str =
    "t.id, t.title, t.artist, t.album, t.album_artist, t.track_no, t.disc_no, t.year,
     t.duration_ms, t.codec, t.bitrate, t.sample_rate, t.bit_depth, t.channels, t.size, t.rev";

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
            "SELECT {TRACK_COLUMNS} FROM tracks WHERE rev > ?1 ORDER BY rev"
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
        let row = conn
            .query_row(
                "SELECT path, coalesce(codec,'') FROM tracks WHERE id=?1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .ok();
        Ok(row)
    })
    .await??;
    let (path, codec) = found.ok_or_else(|| ApiError::not_found("track"))?;

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| ApiError::not_found("file"))?;
    let len = file
        .metadata()
        .await
        .map_err(|_| ApiError::not_found("file"))?
        .len();

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
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=3600"));
    Ok(resp)
}
