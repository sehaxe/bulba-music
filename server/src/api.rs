use crate::db::{default_user, Db};
use crate::range::parse_range;
use crate::scan;
use std::collections::HashMap;
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
    pub cache: crate::derive::Cache,
    pub db: Db,
    pub libraries: Vec<PathBuf>,
    pub ignores: Vec<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/scan", post(run_scan))
        .route("/api/tracks", get(list_tracks))
        .route("/api/tracks/{id}", get(get_track))
        .route("/api/tracks/{id}/stream", get(stream))
        .route("/api/search", get(search_handler))
        .route("/api/albums/{id}/cover", get(cover))
        .route("/api/tracks/{id}/peaks", get(peaks))
        .route("/api/features/build", post(build_features))
        .route("/api/recs/similar/{id}", get(similar))
        .route("/api/recs/home", get(home_mixes))
        .route("/api/recs/daily", get(daily_mixes))
        .route("/api/recs/wave", get(wave))
        .route("/api/likes", get(list_likes))
        .route("/api/likes/ids", get(like_ids))
        .route("/api/likes/{id}", post(add_like).delete(remove_like))
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/{id}",
            get(get_playlist).patch(patch_playlist).delete(delete_playlist),
        )
        .route(
            "/api/playlists/{id}/tracks",
            post(add_playlist_track).put(reorder_playlist),
        )
        .route(
            "/api/playlists/{id}/tracks/{track_id}",
            axum::routing::delete(remove_playlist_track),
        )
        .route("/api/likes/playlists", get(list_playlist_likes))
        .route(
            "/api/likes/playlists/{id}",
            post(like_playlist).delete(unlike_playlist),
        )
        .route("/api/artists", get(list_artists))
        .route("/api/artists/{id}", get(get_artist))
        .route("/api/albums", get(list_albums))
        .route("/api/albums/{id}", get(get_album))
        .route("/api/likes/albums", get(list_album_likes))
        .route("/api/likes/albums/ids", get(album_like_ids))
        .route("/api/likes/albums/{id}", post(add_album_like).delete(remove_album_like))
        .route("/api/likes/artists", get(list_artist_likes))
        .route("/api/likes/artists/ids", get(artist_like_ids))
        .route("/api/likes/artists/{id}", post(add_artist_like).delete(remove_artist_like))
        .route("/api/manifest", get(manifest))
        .route("/api/events", post(record_events))
        .route("/api/stats", get(stats))
        .route("/api/playback", get(get_playback).post(put_playback))
        .route("/api/info", get(info))
        .route("/api/config/serve", post(set_serve))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(whoami))
        .route("/api/library/duplicates", get(list_duplicates))
        .route("/api/library/merge", post(merge_duplicate))
        .route("/api/library/normalize", post(normalize_names))
        .route("/api/library/tree", post(library_tree))
        // Human links: what you paste into a phone over Tailscale.
        .route("/a/{id}", get(artist_html))
        .route("/al/{id}", get(album_html))
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
    fn unauthorized(message: &str) -> Self {
        Self(StatusCode::UNAUTHORIZED, message.to_string())
    }
    fn bad_request(message: &str) -> Self {
        Self(StatusCode::BAD_REQUEST, message.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        // Closures cannot build an ApiError, so they bail with this marker and
        // it is translated here.
        if format!("{e:#}").contains(UNAUTHORIZED) {
            return Self::unauthorized("нужна фраза");
        }
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

#[derive(Debug, Clone, Serialize)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: i64,
    pub has_cover: bool,
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
    pub created_at: i64,
    pub rev: i64,
    /// EBU R128 integrated loudness, filled when features are built. The client
    /// evens out track-to-track level with it.
    pub loudness: Option<f64>,
}

pub const TRACK_COLUMNS: &str = "id, title, artist, album, album_id, has_cover, year, disc,
    track_no, duration_ms, codec, bitrate, sample_rate, bits, channels, bytes, master_count,
    created_at, rev, loudness_db";

pub fn map_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        album: r.get(3)?,
        album_id: r.get(4)?,
        has_cover: r.get(5)?,
        year: r.get(6)?,
        disc: r.get(7)?,
        track_no: r.get(8)?,
        duration_ms: r.get(9)?,
        codec: r.get(10)?,
        bitrate: r.get(11)?,
        sample_rate: r.get(12)?,
        bits: r.get(13)?,
        channels: r.get(14)?,
        bytes: r.get(15)?,
        master_count: r.get(16)?,
        created_at: r.get(17)?,
        rev: r.get(18)?,
        loudness: r.get(19).ok(),
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
        "libraries": st.libraries,
    })))
}

async fn run_scan(State(st): State<AppState>) -> Result<Json<scan::ScanReport>, ApiError> {
    let db = st.db.clone();
    let libraries = st.libraries.clone();
    let ignores = st.ignores.clone();
    let report = tokio::task::spawn_blocking(move || -> anyhow::Result<scan::ScanReport> {
        let mut total = scan::ScanReport::default();
        for root in &libraries {
            let r = scan::scan(&db, root, &ignores)?;
            total.scanned += r.scanned;
            total.masters_added += r.masters_added;
            total.masters_updated += r.masters_updated;
            total.masters_removed += r.masters_removed;
            total.tracks_created += r.tracks_created;
            total.unchanged += r.unchanged;
            total.errors.extend(r.errors);
            total.rev = r.rev.max(total.rev);
        }
        Ok(total)
    })
    .await??;
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
struct LimitQuery {
    limit: Option<i64>,
}

async fn list_artists(
    State(st): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<Vec<crate::index::ArtistSummary>>, ApiError> {
    let db = st.db.clone();
    let limit = q.limit.unwrap_or(5000).clamp(1, 20_000);
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let conn = db.open()?;
        crate::index::artists(&conn, limit)
    })
    .await??;
    Ok(Json(out))
}

async fn get_artist(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<crate::index::ArtistPage>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let conn = db.open()?;
        crate::index::artist(&conn, id)
    })
    .await??;
    out.map(Json).ok_or_else(|| ApiError::not_found("artist"))
}

async fn list_albums(
    State(st): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<Vec<crate::index::AlbumSummary>>, ApiError> {
    let db = st.db.clone();
    let limit = q.limit.unwrap_or(5000).clamp(1, 20_000);
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let conn = db.open()?;
        crate::index::albums(&conn, limit)
    })
    .await??;
    Ok(Json(out))
}

async fn get_album(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<crate::index::AlbumPage>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let conn = db.open()?;
        crate::index::album(&conn, id)
    })
    .await??;
    out.map(Json).ok_or_else(|| ApiError::not_found("album"))
}

async fn list_album_likes(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<crate::index::AlbumSummary>> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.title, ar.name, a.artist_id, a.year,
                    (SELECT count(*) FROM track t WHERE t.album_id=a.id),
                    coalesce((SELECT count(*) FROM play_event pe JOIN track t ON t.id=pe.track_id
                              WHERE t.album_id=a.id AND pe.event=0),0),
                    CASE WHEN a.cover_path IS NULL THEN 0 ELSE 1 END,
                    1, a.disc_total
             FROM album_like l JOIN album a ON a.id=l.album_id
             JOIN artist ar ON ar.id=a.artist_id
             WHERE l.user_id=?1 ORDER BY l.ts DESC",
        )?;
        let rows = stmt.query_map([user], |r| {
            Ok(crate::index::AlbumSummary {
                id: r.get(0)?,
                title: r.get(1)?,
                artist: r.get(2)?,
                artist_id: r.get(3)?,
                year: r.get(4)?,
                tracks: r.get(5)?,
                plays: r.get(6)?,
                has_cover: r.get(7)?,
                liked: r.get(8)?,
                disc_total: r.get(9)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(json!(out)))
}

async fn album_like_ids(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<i64>>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<i64>> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let mut stmt = conn.prepare("SELECT album_id FROM album_like WHERE user_id=?1")?;
        Ok(stmt.query_map([user], |r| r.get(0))?.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(out))
}

async fn add_album_like(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let exists: i64 =
            conn.query_row("SELECT count(*) FROM album WHERE id=?1", [id], |r| r.get(0))?;
        if exists == 0 {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO album_like(user_id,album_id,ts) VALUES(?1,?2,unixepoch())
             ON CONFLICT(user_id,album_id) DO NOTHING",
            rusqlite::params![user, id],
        )?;
        Ok(true)
    })
    .await??
    .then_some(())
    .ok_or_else(|| ApiError::not_found("album"))?;
    Ok(Json(json!({"liked": true})))
}

async fn remove_album_like(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        conn.execute(
            "DELETE FROM album_like WHERE user_id=?1 AND album_id=?2",
            rusqlite::params![user, id],
        )?;
        Ok(())
    })
    .await??;
    Ok(Json(json!({"liked": false})))
}

async fn list_artist_likes(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<crate::index::ArtistSummary>>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<crate::index::ArtistSummary>> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let list = crate::index::artists(&conn, 20_000)?;
        let mut liked = Vec::new();
        for mut a in list {
            let is: i64 = conn.query_row(
                "SELECT count(*) FROM artist_like WHERE user_id=?1 AND artist_id=?2",
                rusqlite::params![user, a.id],
                |r| r.get(0),
            )?;
            if is > 0 {
                a.liked = 1;
                liked.push(a);
            }
        }
        Ok(liked)
    })
    .await??;
    Ok(Json(out))
}

async fn artist_like_ids(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<i64>>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<i64>> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let mut stmt = conn.prepare("SELECT artist_id FROM artist_like WHERE user_id=?1")?;
        Ok(stmt.query_map([user], |r| r.get(0))?.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(out))
}

async fn add_artist_like(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let exists: i64 =
            conn.query_row("SELECT count(*) FROM artist WHERE id=?1", [id], |r| r.get(0))?;
        if exists == 0 {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO artist_like(user_id,artist_id,ts) VALUES(?1,?2,unixepoch())
             ON CONFLICT(user_id,artist_id) DO NOTHING",
            rusqlite::params![user, id],
        )?;
        Ok(true)
    })
    .await??
    .then_some(())
    .ok_or_else(|| ApiError::not_found("artist"))?;
    Ok(Json(json!({"liked": true})))
}

async fn remove_artist_like(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        conn.execute(
            "DELETE FROM artist_like WHERE user_id=?1 AND artist_id=?2",
            rusqlite::params![user, id],
        )?;
        Ok(())
    })
    .await??;
    Ok(Json(json!({"liked": false})))
}

async fn list_playlists(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<crate::playlists::Playlist>>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<crate::playlists::Playlist>> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::list(&conn, me)
    })
    .await??;
    Ok(Json(out))
}

async fn list_playlist_likes(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<crate::playlists::Playlist>>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<crate::playlists::Playlist>> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::liked(&conn, me)
    })
    .await??;
    Ok(Json(out))
}

async fn get_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<crate::playlists::PlaylistPage>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<crate::playlists::PlaylistPage>> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::get(&conn, me, id)
    })
    .await??;
    out.map(Json).ok_or_else(|| ApiError::not_found("playlist"))
}

async fn create_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Json(new): Json<crate::playlists::NewPlaylist>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let id = tokio::task::spawn_blocking(move || -> anyhow::Result<i64> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::create(&conn, me, &new)
    })
    .await??;
    Ok(Json(json!({ "id": id })))
}

async fn patch_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(patch): Json<crate::playlists::PlaylistPatch>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::update(&conn, me, id, &patch)
    })
    .await??;
    if !ok {
        return Err(ApiError::forbidden("не твой плейлист"));
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::delete(&conn, me, id)
    })
    .await??;
    if !ok {
        return Err(ApiError::forbidden("не твой плейлист"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
struct PlaylistTrackBody {
    track_id: i64,
    position: Option<i64>,
}

async fn list_duplicates(
    State(st): State<AppState>,
) -> Result<Json<Vec<crate::dupes::DupGroup>>, ApiError> {
    let db = st.db.clone();
    let groups = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<_>> {
        let conn = db.open()?;
        crate::dupes::find(&conn)
    })
    .await??;
    Ok(Json(groups))
}

#[derive(serde::Deserialize)]
struct MergeBody {
    kind: String,
    keep: i64,
    merge: i64,
}

async fn merge_duplicate(
    State(st): State<AppState>,
    Json(body): Json<MergeBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        crate::dupes::merge(&conn, &body.kind, body.keep, body.merge)
    })
    .await??;
    if !ok {
        return Err(ApiError::not_found("нечего сливать"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
struct CursorBody {
    track_id: i64,
    position_ms: i64,
}

#[derive(serde::Serialize)]
struct CursorOut {
    track_id: i64,
    position_ms: i64,
    updated_at: i64,
    track: Option<serde_json::Value>,
}

async fn get_playback(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Option<CursorOut>>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<CursorOut>> {
        let conn = db.open()?;
        let Some(me) = user_from(&headers, &conn, caller.local)? else {
            return Ok(None);
        };
        let row = conn
            .query_row(
                "SELECT track_id, position_ms, updated_at FROM playback_cursor
                 WHERE user_id=?1",
                [me],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
            )
            .ok();
        let Some((track_id, position_ms, updated_at)) = row else {
            return Ok(None);
        };
        let track = {
            let mut stmt =
                conn.prepare(&format!("SELECT {TRACK_COLUMNS} FROM v_track WHERE id=?1"))?;
            stmt.query_row([track_id], map_track).ok().map(|t| {
                serde_json::to_value(t).unwrap_or(serde_json::Value::Null)
            })
        };
        Ok(Some(CursorOut {
            track_id,
            position_ms,
            updated_at,
            track,
        }))
    })
    .await??;
    Ok(Json(out))
}

async fn put_playback(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Json(body): Json<CursorBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        let Some(me) = user_from(&headers, &conn, caller.local)? else {
            return Ok(());
        };
        conn.execute(
            "INSERT INTO playback_cursor(user_id, track_id, position_ms, updated_at)
             VALUES(?1, ?2, ?3, unixepoch())
             ON CONFLICT(user_id) DO UPDATE SET
               track_id=excluded.track_id,
               position_ms=excluded.position_ms,
               updated_at=excluded.updated_at",
            rusqlite::params![me, body.track_id, body.position_ms],
        )?;
        Ok(())
    })
    .await??;
    Ok(Json(json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
struct ReorderBody {
    track_ids: Vec<i64>,
}

async fn reorder_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<ReorderBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::reorder(&conn, me, id, &body.track_ids)
    })
    .await??;
    if !ok {
        return Err(ApiError::forbidden("не твой плейлист"));
    }
    Ok(Json(json!({ "ok": true })))
}

async fn add_playlist_track(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<PlaylistTrackBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::add_track(&conn, me, id, body.track_id, body.position)
    })
    .await??;
    if !ok {
        return Err(ApiError::forbidden("не твой плейлист или нет трека"));
    }
    Ok(Json(json!({ "ok": true })))
}

async fn remove_playlist_track(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path((id, track_id)): Path<(i64, i64)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::remove_track(&conn, me, id, track_id)
    })
    .await??;
    if !ok {
        return Err(ApiError::forbidden("не твой плейлист"));
    }
    Ok(Json(json!({ "ok": true })))
}

async fn like_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::set_like(&conn, me, id, true)
    })
    .await??;
    if !ok {
        return Err(ApiError::not_found("playlist"));
    }
    Ok(Json(json!({ "ok": true })))
}

async fn unlike_playlist(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::playlists::set_like(&conn, me, id, false)
    })
    .await??;
    if !ok {
        return Err(ApiError::not_found("playlist"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
struct WaveQuery {
    /// Start from this track instead of from the taste alone.
    seed: Option<i64>,
    /// Comma-separated ids to leave out (the queue and what just played).
    exclude: Option<String>,
    limit: Option<usize>,
    /// 0 keeps to what is known, 1 goes hunting for the unknown.
    novelty: Option<f64>,
}

#[derive(Debug, Serialize)]
struct WaveTrack {
    #[serde(flatten)]
    track: Track,
    reason: String,
    score: f64,
}

#[derive(Debug, Serialize)]
struct WaveReply {
    tracks: Vec<WaveTrack>,
    seed: Option<i64>,
    /// False on a fresh account: the wave is guessing, not knowing.
    taste_ready: bool,
}

async fn wave(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Query(q): Query<WaveQuery>,
) -> Result<Json<WaveReply>, ApiError> {
    let db = st.db.clone();
    let exclude: Vec<i64> = q
        .exclude
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let limit = q.limit.unwrap_or(12).clamp(1, 50);
    let novelty = q.novelty.unwrap_or(0.35).clamp(0.0, 1.0);

    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<WaveReply> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;

        // artist id and name per track, once, for the diversity rule and the
        // "you listen to X a lot" sentence.
        let mut artist_of: HashMap<i64, i64> = HashMap::new();
        let mut name_of_artist: HashMap<i64, String> = HashMap::new();
        let mut title_of: HashMap<i64, String> = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT t.id, ar.id, ar.name, t.title FROM track t
                 JOIN album al ON al.id = t.album_id
                 JOIN artist ar ON ar.id = al.artist_id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?;
            for row in rows.flatten() {
                artist_of.insert(row.0, row.1);
                name_of_artist.entry(row.1).or_insert(row.2);
                title_of.insert(row.0, row.3);
            }
        }

        let played: i64 = conn
            .query_row(
                "SELECT count(*) FROM play_event WHERE user_id=?1 AND event IN (0,3)",
                [me],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let seed_title: Option<String> = q
            .seed
            .and_then(|id| title_of.get(&id).cloned());

        let items = crate::wave::pick(
            &conn,
            &crate::wave::Request {
                user: me,
                seed: q.seed,
                exclude: &exclude,
                limit,
                novelty,
                seed_title: seed_title.as_deref(),
            },
            &artist_of,
            &name_of_artist,
            &title_of,
        )?;

        let ids: Vec<i64> = items.iter().map(|i| i.track_id).collect();
        let tracks = tracks_by_ids(&conn, &ids)?;
        let by_id: HashMap<i64, Track> =
            tracks.into_iter().map(|t| (t.id, t)).collect();
        let mut out = Vec::new();
        for item in items {
            if let Some(track) = by_id.get(&item.track_id) {
                out.push(WaveTrack {
                    track: (*track).clone(),
                    reason: item.reason,
                    score: item.score,
                });
            }
        }
        Ok(WaveReply {
            tracks: out,
            seed: q.seed,
            taste_ready: played >= 5,
        })
    })
    .await??;
    Ok(Json(out))
}

async fn artist_html(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Html<String>, ApiError> {
    let db = st.db.clone();
    let host = host_of(&headers, &st);
    let page = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
        let conn = db.open()?;
        crate::pages::artist_page(&conn, id, &host)
    })
    .await??;
    page.map(axum::response::Html)
        .ok_or_else(|| ApiError::not_found("artist"))
}

async fn album_html(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Html<String>, ApiError> {
    let db = st.db.clone();
    let host = host_of(&headers, &st);
    let page = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
        let conn = db.open()?;
        crate::pages::album_page(&conn, id, &host)
    })
    .await??;
    page.map(axum::response::Html)
        .ok_or_else(|| ApiError::not_found("album"))
}

/// The host the client used, so links in the page keep working from a phone.
fn host_of(headers: &axum::http::HeaderMap, _st: &AppState) -> String {
    headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1:8422")
        .to_string()
}

#[derive(Debug, Deserialize)]
struct TreeQuery {
    #[serde(default)]
    apply: bool,
    limit: Option<usize>,
    /// Target root; defaults to the first configured library.
    root: Option<String>,
}

/// Reorganises the whole library into Artist/Album (Year)/NN - Title.ext.
async fn library_tree(
    State(st): State<AppState>,
    Query(q): Query<TreeQuery>,
) -> Result<Json<crate::normalize::TreeReport>, ApiError> {
    let db = st.db.clone();
    // The first library root is the library; it already is the folder that
    // holds Artist/Album/…, so nothing is appended to it.
    let root = q
        .root
        .clone()
        .map(std::path::PathBuf::from)
        .or_else(|| st.libraries.first().cloned())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let limit = q.limit.unwrap_or(usize::MAX);
    let report = tokio::task::spawn_blocking(
        move || -> anyhow::Result<crate::normalize::TreeReport> {
            let conn = db.open()?;
            if q.apply {
                crate::normalize::tree_apply(&conn, &root, limit)
            } else {
                crate::normalize::tree_plan(&conn, &root)
            }
        },
    )
    .await??;
    Ok(Json(report))
}

#[derive(Debug, Deserialize)]
struct NormalizeQuery {
    #[serde(default)]
    apply: bool,
    limit: Option<usize>,
}

async fn normalize_names(
    State(st): State<AppState>,
    Query(q): Query<NormalizeQuery>,
) -> Result<Json<crate::normalize::Report>, ApiError> {
    let db = st.db.clone();
    let libraries = st.libraries.clone();
    let limit = q.limit.unwrap_or(usize::MAX);
    let report = tokio::task::spawn_blocking(move || -> anyhow::Result<crate::normalize::Report> {
        let conn = db.open()?;
        if q.apply {
            crate::normalize::apply(&conn, &libraries, limit)
        } else {
            crate::normalize::plan(&conn, &libraries)
        }
    })
    .await??;
    Ok(Json(report))
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    period: Option<String>,
    year: Option<i64>,
    month: Option<i64>,
}

async fn stats(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Query(q): Query<StatsQuery>,
) -> Result<Json<crate::stats::Stats>, ApiError> {
    let period = q.period.unwrap_or_else(|| "all".into());
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<crate::stats::Stats> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::stats::collect(&conn, me, &period, q.year, q.month)
    })
    .await??;
    Ok(Json(out))
}

#[derive(serde::Deserialize)]
struct ServeBody {
    quality: String,
}

async fn set_serve(
    State(st): State<AppState>,
    Json(body): Json<ServeBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let serve = crate::serve::Serve::parse(&body.quality)
        .ok_or_else(|| ApiError::bad_request("неизвестный формат"))?;
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        crate::serve::set(&conn, serve)
    })
    .await??;
    Ok(Json(json!({ "serve": serve.tag() })))
}

async fn info(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let libraries: Vec<String> = st
        .libraries
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let data = tokio::task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
        let conn = db.open()?;
        let tracks: i64 = conn.query_row("SELECT count(*) FROM track", [], |r| r.get(0))?;
        let albums: i64 = conn.query_row("SELECT count(*) FROM album", [], |r| r.get(0))?;
        let artists: i64 = conn.query_row("SELECT count(*) FROM artist", [], |r| r.get(0))?;
        let bytes: i64 = conn.query_row("SELECT coalesce(sum(bytes),0) FROM master", [], |r| r.get(0))?;
        let serve = crate::serve::get(&conn).tag().to_string();
        let embedded: i64 =
            conn.query_row("SELECT count(*) FROM embed WHERE model=?1", [crate::features::MODEL], |r| r.get(0))?;
        let lossless: i64 = conn.query_row(
            "SELECT count(*) FROM v_track WHERE codec IN ('flac','alac','wav','aiff')",
            [],
            |r| r.get(0),
        )?;
        let rev = crate::db::rev(&conn)?;
        Ok(json!({
            "tracks": tracks,
            "albums": albums,
            "artists": artists,
            "bytes": bytes,
            "lossless": lossless,
            "embedded": embedded,
            "serve": serve,
            "rev": rev,
        }))
    })
    .await??;

    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "libraries": libraries,
        "stats": data,
        "features_model": crate::features::MODEL,
        "derived_bytes": st.cache.size(),
    })))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<i64>,
}

async fn search_handler(
    State(st): State<AppState>,
    Query(sq): Query<SearchQuery>,
) -> Result<Json<crate::search::SearchResults>, ApiError> {
    let limit = sq.limit.unwrap_or(50).clamp(1, 200);
    let db = st.db.clone();
    let results =
        tokio::task::spawn_blocking(move || -> anyhow::Result<crate::search::SearchResults> {
            let conn = db.open()?;
            crate::search::search(&conn, &sq.q, limit)
        })
        .await??;
    Ok(Json(results))
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
    caller: Caller,
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
                .query_row("SELECT id FROM user WHERE name=?1", [l], |r| r.get(0))
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

#[derive(Debug, Deserialize)]
struct BuildQuery {
    limit: Option<i64>,
    loudness: Option<bool>,
}

/// Computes content embeddings for tracks that do not have one yet.
/// The client calls this after a scan; it is incremental and safe to repeat.
async fn build_features(
    State(st): State<AppState>,
    Query(q): Query<BuildQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = q.limit.unwrap_or(200).clamp(1, 5000);
    let db = st.db.clone();
    let st_db_path = st.db.path().to_path_buf();
    let loudness_only = q.loudness.unwrap_or(false);
    let stats = tokio::task::spawn_blocking(move || -> anyhow::Result<(usize, i64)> {
        let conn = db.open()?;
        let rows: Vec<(i64, String)> = {
            // `?loudness=1` is a different pass: loudness was added after the
            // library had already been embedded, and re-decoding every file for
            // its mel bands again would be pure waste.
            let sql = if loudness_only {
                "SELECT t.id, m.path
                 FROM track t
                 JOIN track_master tm ON tm.track_id = t.id
                 JOIN master m ON m.id = tm.master_id
                 WHERE t.loudness_db IS NULL
                 GROUP BY t.id
                 ORDER BY t.id
                 LIMIT ?2"
            } else {
                "SELECT t.id, m.path
                 FROM track t
                 JOIN track_master tm ON tm.track_id = t.id
                 JOIN master m ON m.id = tm.master_id
                 WHERE NOT EXISTS (
                     SELECT 1 FROM embed e WHERE e.track_id = t.id AND e.model = ?1)
                 GROUP BY t.id
                 ORDER BY t.id
                 LIMIT ?2"
            };
            let mut stmt = conn.prepare(sql)?;
            stmt.query_map(rusqlite::params![crate::features::MODEL, limit], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect()
        };
        // Decoding dominates, and this box has cores to spare: six workers
        // turn an 80-minute library pass into about fifteen.
        let workers = 6usize;
        let chunk = (rows.len() + workers - 1) / workers.max(1);
        let db_path = st_db_path.clone();
        let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        std::thread::scope(|scope| {
            for part in rows.chunks(chunk.max(1)) {
                let db_path = db_path.clone();
                let done = done.clone();
                let errors = errors.clone();
                scope.spawn(move || {
                    let Ok(conn) = rusqlite::Connection::open(&db_path) else {
                        return;
                    };
                    for (id, path) in part {
                        if loudness_only {
                            if let Ok(Some(l)) = crate::loudness::lufs(path) {
                                let _ = conn.execute(
                                    "UPDATE track SET loudness_db=?1 WHERE id=?2",
                                    rusqlite::params![l, id],
                                );
                            }
                            done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            continue;
                        }
                        match crate::features::extract(path) {
                            Ok(fp) => {
                                let _ = conn.execute(
                                    "INSERT OR REPLACE INTO embed(model, track_id, dim, vec)
                                     VALUES(?1, ?2, ?3, ?4)",
                                    rusqlite::params![
                                        crate::features::MODEL,
                                        id,
                                        crate::features::DIM as i64,
                                        crate::features::to_blob(&fp.vec)
                                    ],
                                );
                                let _ = conn.execute(
                                    "INSERT OR REPLACE INTO track_features
                                       (track_id, tempo_bpm, brightness, energy, mode)
                                     VALUES(?1, ?2, ?3, ?4, ?5)",
                                    rusqlite::params![
                                        id,
                                        fp.tempo_bpm as f64,
                                        fp.brightness as f64,
                                        fp.energy as f64,
                                        fp.mode as f64
                                    ],
                                );
                                // ReplayGain needs the integrated loudness, and
                                // the decode is already happening next door.
                                if let Ok(Some(l)) = crate::loudness::lufs(path) {
                                    let _ = conn.execute(
                                        "UPDATE track SET loudness_db=?1 WHERE id=?2",
                                        rusqlite::params![l, id],
                                    );
                                }
                                done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            }
                            Err(e) => {
                                if let Ok(mut list) = errors.lock() {
                                    list.push(format!("{path}: {e}"));
                                }
                            }
                        }
                    }
                });
            }
        });
        let done = done.load(std::sync::atomic::Ordering::Relaxed);
        let errors = errors.lock().map(|e| e.clone()).unwrap_or_default();
        let remaining: i64 = conn
            .query_row(
                "SELECT count(*) FROM track t WHERE NOT EXISTS (
                   SELECT 1 FROM embed e WHERE e.track_id = t.id AND e.model = ?1)",
                [crate::features::MODEL],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if !errors.is_empty() {
            tracing::warn!("features: {} files failed, first: {}", errors.len(), errors[0]);
        }
        Ok((done, remaining))
    })
    .await??;
    Ok(Json(json!({
        "computed": stats.0,
        "remaining": stats.1,
        "model": crate::features::MODEL,
    })))
}

#[derive(Debug, Serialize)]
struct Mix {
    id: String,
    title: String,
    note: String,
    tracks: Vec<Track>,
}

fn query_tracks(conn: &rusqlite::Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> anyhow::Result<Vec<Track>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, map_track)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn seed_vector(conn: &rusqlite::Connection, id: i64) -> anyhow::Result<Option<Vec<f32>>> {
    let blob: Option<Vec<u8>> = conn
        .query_row(
            "SELECT vec FROM embed WHERE track_id=?1 AND model=?2",
            rusqlite::params![id, crate::features::MODEL],
            |r| r.get(0),
        )
        .ok();
    Ok(blob.map(|b| crate::features::from_blob(&b)))
}

fn similar_scored(
    conn: &rusqlite::Connection,
    seed: &[f32],
    exclude: Option<i64>,
    n: usize,
) -> anyhow::Result<Vec<(i64, f32)>> {
    let mut stmt = conn.prepare(
        "SELECT track_id, vec FROM embed WHERE model=?1 AND (?2 IS NULL OR track_id<>?2)",
    )?;
    let mut scored: Vec<(i64, f32)> = stmt
        .query_map(
            rusqlite::params![crate::features::MODEL, exclude],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)),
        )?
        .filter_map(|r| r.ok())
        .map(|(id, blob)| (id, crate::features::cosine(seed, &crate::features::from_blob(&blob))))
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(n);
    Ok(scored)
}

fn tracks_by_ids(conn: &rusqlite::Connection, ids: &[i64]) -> anyhow::Result<Vec<Track>> {
    let mut stmt = conn.prepare(&format!("SELECT {TRACK_COLUMNS} FROM v_track WHERE id=?1"))?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(t) = stmt.query_row([id], map_track) {
            out.push(t);
        }
    }
    Ok(out)
}

/// The "for you" page: mixes that can be built from what the library already
/// knows, before any taste model exists. Empty mixes are dropped, so a fresh
/// library shows only what is honest.
async fn home_mixes(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<Mix>>, ApiError> {
    let db = st.db.clone();
    let mixes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Mix>> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let mut out: Vec<Mix> = Vec::new();
        let cols = TRACK_COLUMNS
            .split(',')
            .map(|c| format!("v.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");

        let recent = query_tracks(
            &conn,
            &format!("SELECT {cols} FROM v_track v ORDER BY v.created_at DESC, v.id DESC LIMIT 12"),
            &[],
        )?;
        if !recent.is_empty() {
            out.push(Mix {
                id: "new".into(),
                title: "новое в библиотеке".into(),
                note: "последние добавленные".into(),
                tracks: recent,
            });
        }

        let repeated = query_tracks(
            &conn,
            &format!(
                "SELECT {cols} FROM v_track v
                 JOIN (SELECT track_id, count(*) c FROM play_event
                       WHERE event=3 AND ts > unixepoch()-30*86400
                       GROUP BY track_id ORDER BY c DESC LIMIT 15) e ON e.track_id = v.id"
            ),
            &[],
        )?;
        if repeated.len() > 2 {
            out.push(Mix {
                id: "repeat".into(),
                title: "на повторе".into(),
                note: "дослушано за месяц".into(),
                tracks: repeated,
            });
        }

        let last: Option<i64> = conn
            .query_row("SELECT track_id FROM play_event ORDER BY id DESC LIMIT 1", [], |r| {
                r.get(0)
            })
            .ok();

        if let Some(last_id) = last {
            if let Some(seed) = seed_vector(&conn, last_id)? {
                let scored = similar_scored(&conn, &seed, Some(last_id), 20)?;
                let ids: Vec<i64> = scored.iter().map(|(id, _)| *id).collect();
                let tracks = tracks_by_ids(&conn, &ids)?;
                if !tracks.is_empty() {
                    out.push(Mix {
                        id: "radio".into(),
                        title: "продолжение".into(),
                        note: "похоже на последнее".into(),
                        tracks,
                    });
                }
            }
        }

        // Deep cuts: never played, ranked by closeness to the last five plays.
        let played_seeds: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT track_id FROM play_event GROUP BY track_id ORDER BY max(id) DESC LIMIT 5",
            )?;
            stmt.query_map([], |r| r.get(0))?.filter_map(|r| r.ok()).collect()
        };
        if !played_seeds.is_empty() {
            let mut best: std::collections::HashMap<i64, f32> = std::collections::HashMap::new();
            for seed_id in &played_seeds {
                if let Some(seed) = seed_vector(&conn, *seed_id)? {
                    for (id, score) in similar_scored(&conn, &seed, None, 60)? {
                        let e = best.entry(id).or_insert(0.0);
                        if score > *e {
                            *e = score;
                        }
                    }
                }
            }
            let mut unplayed: Vec<(i64, f32)> = best
                .into_iter()
                .filter(|(id, _)| {
                    conn.query_row(
                        "SELECT count(*) FROM play_event WHERE track_id=?1",
                        [id],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap_or(1)
                        == 0
                })
                .collect();
            unplayed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            unplayed.truncate(15);
            let ids: Vec<i64> = unplayed.iter().map(|(id, _)| *id).collect();
            let tracks = tracks_by_ids(&conn, &ids)?;
            if tracks.len() > 2 {
                out.push(Mix {
                    id: "deep".into(),
                    title: "глубокие нарезки".into(),
                    note: "ещё не играло, но похоже".into(),
                    tracks,
                });
            }
        }

        let random = query_tracks(
            &conn,
            &format!("SELECT {cols} FROM v_track v ORDER BY random() LIMIT 25"),
            &[],
        )?;
        if random.len() > 2 {
            out.push(Mix {
                id: "shuffle".into(),
                title: "перемешать всё".into(),
                note: "случайная выборка".into(),
                tracks: random,
            });
        }

        Ok(out)
    })
    .await??;
    Ok(Json(mixes))
}

/// Four numbered playlists, stable for the day.
async fn daily_mixes(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<Mix>>, ApiError> {
    let db = st.db.clone();
    let mixes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Mix>> {
        let conn = db.open()?;
        let me = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        crate::daily::ensure(&conn, crate::features::MODEL, me)?;
        let rows = crate::daily::load(&conn, me)?;
        let mut out = Vec::with_capacity(rows.len());
        for m in rows {
            let tracks = tracks_by_ids(&conn, &m.ids)?;
            if tracks.is_empty() {
                continue;
            }
            out.push(Mix {
                id: m.id,
                title: m.title,
                note: m.note,
                tracks,
            });
        }
        Ok(out)
    })
    .await??;
    Ok(Json(mixes))
}

/// The bearer token, if any, decides who is asking. Without one the request
/// belongs to the local user, so a single machine never has to log in.
/// Requests from this machine are the owner's; anything else must present a
/// token. Without the rule every phone silently inherits the desktop's likes
/// and history, and "sync by phrase" means nothing.
/// Whether the request came from this machine.
///
/// Read straight out of the request extensions, so it works the same whether
/// the server was mounted with connect info or not (tests mount without it and
/// therefore count as local).
pub struct Caller {
    pub local: bool,
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for Caller {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let local = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|axum::extract::ConnectInfo(addr)| addr.ip().is_loopback())
            .unwrap_or(true);
        Ok(Caller { local })
    }
}

pub const UNAUTHORIZED: &str = "unauthorized: нужна фраза";

fn user_from(
    headers: &axum::http::HeaderMap,
    conn: &rusqlite::Connection,
    local: bool,
) -> anyhow::Result<Option<i64>> {
    if let Some(value) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(text) = value.to_str() {
            if let Some(token) = text.strip_prefix("Bearer ") {
                if let Some(id) = crate::auth::user_for_token(conn, token.trim())? {
                    return Ok(Some(id));
                }
            }
        }
    }
    if local {
        return Ok(Some(crate::auth::local_user(conn)?));
    }
    Ok(None)
}

#[derive(Debug, Deserialize)]
struct RegisterBody {
    name: Option<String>,
}

async fn register(
    State(st): State<AppState>,
    Json(body): Json<RegisterBody>,
) -> Result<Json<crate::auth::Session>, ApiError> {
    let db = st.db.clone();
    let session = tokio::task::spawn_blocking(move || -> anyhow::Result<crate::auth::Session> {
        let conn = db.open()?;
        crate::auth::register(&conn, body.name.as_deref().unwrap_or(""))
    })
    .await??;
    Ok(Json(session))
}

#[derive(Debug, Deserialize)]
struct LoginBody {
    phrase: String,
}

async fn login(
    State(st): State<AppState>,
    Json(body): Json<LoginBody>,
) -> Result<Json<crate::auth::Session>, ApiError> {
    let db = st.db.clone();
    let phrase = body.phrase.clone();
    let session = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<crate::auth::Session>> {
        let conn = db.open()?;
        crate::auth::login(&conn, &phrase)
    })
    .await?;

    match session {
        Ok(Some(s)) => Ok(Json(s)),
        Ok(None) => Err(ApiError::unauthorized("фраза не подходит")),
        Err(e) => Err(ApiError::bad_request(&e.to_string())),
    }
}

async fn whoami(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let out = tokio::task::spawn_blocking(move || -> anyhow::Result<(i64, String)> {
        let conn = db.open()?;
        let id = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let name: String = conn.query_row("SELECT name FROM user WHERE id=?1", [id], |r| r.get(0))?;
        Ok((id, name))
    })
    .await??;
    Ok(Json(json!({"user_id": out.0, "name": out.1})))
}

async fn list_likes(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<Track>>, ApiError> {
    let db = st.db.clone();
    let tracks = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Track>> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM v_track t
             JOIN track_like l ON l.track_id = t.id
             WHERE l.user_id = ?1
             ORDER BY l.ts DESC"
        ))?;
        let rows = stmt.query_map([user], map_track)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(tracks))
}

async fn like_ids(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<i64>>, ApiError> {
    let db = st.db.clone();
    let ids = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<i64>> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let mut stmt = conn.prepare("SELECT track_id FROM track_like WHERE user_id = ?1")?;
        let rows = stmt.query_map([user], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(ids))
}

async fn add_like(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        let exists: i64 = conn.query_row("SELECT count(*) FROM track WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
        if exists == 0 {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO track_like(user_id, track_id, ts) VALUES(?1,?2,unixepoch())
             ON CONFLICT(user_id, track_id) DO NOTHING",
            rusqlite::params![user, id],
        )?;
        // The like table itself is the signal; event 4 is reserved for real
        // replays, so liking must not fabricate one.
        Ok(true)
    })
    .await??;
    if !ok {
        return Err(ApiError::not_found("track"));
    }
    Ok(Json(json!({"liked": true})))
}

async fn remove_like(
    State(st): State<AppState>,
    caller: Caller,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        let user = user_from(&headers, &conn, caller.local)?
            .ok_or_else(|| anyhow::anyhow!(UNAUTHORIZED))?;
        conn.execute(
            "DELETE FROM track_like WHERE user_id=?1 AND track_id=?2",
            rusqlite::params![user, id],
        )?;
        Ok(())
    })
    .await??;
    Ok(Json(json!({"liked": false})))
}

#[derive(Debug, Deserialize)]
struct SimilarQuery {
    n: Option<i64>,
}

/// Content-based neighbours: cosine over the stored embeddings. Brute force is
/// intentional — 10k × 56 floats is ~5 ms, and an ANN index becomes worth its
/// complexity somewhere past 100k tracks.
async fn similar(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<SimilarQuery>,
) -> Result<Json<Vec<Track>>, ApiError> {
    let n = q.n.unwrap_or(25).clamp(1, 200);
    let tracks = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Track>> {
        let conn = st.db.open()?;

        let seed = conn
            .query_row(
                "SELECT vec FROM embed WHERE track_id=?1 AND model=?2",
                rusqlite::params![id, crate::features::MODEL],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .ok();
        let seed = match seed {
            Some(blob) => crate::features::from_blob(&blob),
            None => {
                // Compute it on demand so the first click on a fresh library works.
                let path: String = conn.query_row(
                    "SELECT m.path FROM track_master tm JOIN master m ON m.id=tm.master_id
                     WHERE tm.track_id=?1 ORDER BY (m.codec='flac') DESC LIMIT 1",
                    [id],
                    |r| r.get(0),
                )?;
                let fp = crate::features::extract(&path)?;
                conn.execute(
                    "INSERT OR REPLACE INTO embed(model, track_id, dim, vec)
                     VALUES(?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        crate::features::MODEL,
                        id,
                        crate::features::DIM as i64,
                        crate::features::to_blob(&fp.vec)
                    ],
                )?;
                conn.execute(
                    "UPDATE track SET tempo_bpm=?1, brightness=?2, energy=?3, mode=?4
                     WHERE id=?5",
                    rusqlite::params![fp.tempo_bpm, fp.brightness, fp.energy, fp.mode, id],
                )?;
                fp.vec
            }
        };

        let mut scored: Vec<(i64, f32)> = {
            let mut stmt = conn.prepare(
                "SELECT track_id, vec FROM embed WHERE model=?1 AND track_id<>?2",
            )?;
            stmt.query_map(rusqlite::params![crate::features::MODEL, id], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
            })?
            .filter_map(|r| r.ok())
            .map(|(track, blob)| {
                let v = crate::features::from_blob(&blob);
                (track, crate::features::cosine(&seed, &v))
            })
            .collect()
        };
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n as usize);

        let mut out = Vec::with_capacity(scored.len());
        let mut stmt = conn.prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM v_track WHERE id=?1"
        ))?;
        for (track_id, _) in scored {
            if let Ok(t) = stmt.query_row([track_id], map_track) {
                out.push(t);
            }
        }
        Ok(out)
    })
    .await??;
    Ok(Json(tracks))
}

/// Waveform peaks, 160 buckets, computed once per track with ffmpeg and cached.
/// The client draws them mirrored around the centre line.
async fn peaks(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<u8>>, ApiError> {
    let db = st.db.clone();
    let cached = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<Vec<u8>>> {
        let conn = db.open()?;
        let row = conn
            .query_row("SELECT data FROM peaks WHERE track_id=?1", [id], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .ok();
        Ok(row)
    })
    .await??;
    if let Some(data) = cached {
        return Ok(Json(data));
    }

    let db = st.db.clone();
    let path = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
        let conn = db.open()?;
        Ok(conn
            .query_row(
                "SELECT m.path FROM track_master tm JOIN master m ON m.id=tm.master_id
                 WHERE tm.track_id=?1 ORDER BY (m.codec='flac') DESC LIMIT 1",
                [id],
                |r| r.get::<_, String>(0),
            )
            .ok())
    })
    .await??;
    let path = path.ok_or_else(|| ApiError::not_found("track"))?;

    let data = tokio::task::spawn_blocking(move || compute_peaks(&path)).await??;
    let db = st.db.clone();
    let store = data.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        conn.execute(
            "INSERT OR REPLACE INTO peaks(track_id, data) VALUES(?1, ?2)",
            rusqlite::params![id, store],
        )?;
        Ok(())
    })
    .await??;
    Ok(Json(data))
}

fn compute_peaks(path: &str) -> anyhow::Result<Vec<u8>> {
    use std::process::Command;
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i", path, "-ac", "1", "-ar", "8000", "-f", "s16le", "-"])
        .output()?;
    let bytes = out.stdout;
    let samples: Vec<f64> = bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64 / 32768.0)
        .collect();
    const BUCKETS: usize = 160;
    let mut peaks = Vec::with_capacity(BUCKETS);
    if samples.is_empty() {
        return Ok(vec![0; BUCKETS]);
    }
    let bucket = (samples.len() / BUCKETS).max(1);
    for i in 0..BUCKETS {
        let start = i * bucket;
        let end = ((i + 1) * bucket).min(samples.len());
        if start >= end {
            peaks.push(0);
            continue;
        }
        let sum: f64 = samples[start..end].iter().map(|s| s * s).sum();
        let rms = (sum / (end - start) as f64).sqrt();
        peaks.push((rms.powf(0.6) * 255.0).clamp(0.0, 255.0) as u8);
    }
    // Normalise to the track's own 95th percentile so quiet masters still show
    // a readable shape instead of a flat line.
    let mut sorted = peaks.clone();
    sorted.sort_unstable();
    let p95 = sorted[((sorted.len() as f64) * 0.95) as usize].max(1) as f64;
    for v in peaks.iter_mut() {
        *v = ((*v as f64 / p95) * 255.0).clamp(0.0, 255.0) as u8;
    }
    Ok(peaks)
}

#[derive(Debug, Deserialize)]
struct CoverQuery {
    /// Pixel width; defaults to 512, which is more than any list needs.
    w: Option<u32>,
    /// webp (default) or jpeg/png for clients that ask.
    fmt: Option<String>,
    /// `orig` serves the stored file untouched.
    size: Option<String>,
}

async fn cover(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<CoverQuery>,
) -> Result<Response, ApiError> {
    let path = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<String>> {
        let conn = st.db.open()?;
        Ok(conn
            .query_row("SELECT cover_path FROM album WHERE id=?1", [id], |r| {
                r.get::<_, Option<String>>(0)
            })
            .ok()
            .flatten())
    })
    .await??;
    let path = path.ok_or_else(|| ApiError::not_found("cover"))?;

    // Covers are the biggest thing a list downloads, so they are served scaled
    // and re-encoded once, then from cache. `size=orig` keeps the old path for
    // anything that needs the full picture.
    if q.size.as_deref() != Some("orig") {
        let width = q.w.unwrap_or(512).clamp(64, 2048);
        let fmt = q.fmt.unwrap_or_else(|| "webp".to_string());
        let cache = st.cache.clone();
        let source = std::path::PathBuf::from(&path);
        let derived = tokio::task::spawn_blocking(move || cache.cover(&source, width, &fmt))
            .await?
            .map_err(|e| ApiError::internal(format!("{e:#}")))?;
        let bytes = tokio::fs::read(&derived)
            .await
            .map_err(|_| ApiError::not_found("cover"))?;
        let mime = match derived.extension().and_then(|e| e.to_str()) {
            Some("webp") => "image/webp",
            Some("jpg") => "image/jpeg",
            Some("jxl") => "image/jxl",
            _ => "image/png",
        };
        return Response::builder()
            .header(header::CONTENT_TYPE, mime)
            .header(header::CONTENT_LENGTH, bytes.len())
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(axum::body::Body::from(bytes))
            .map_err(|e| ApiError::internal(e.to_string()));
    }

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::not_found("cover"))?;
    let mime = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .body(axum::body::Body::from(bytes))
        .map_err(ApiError::internal)
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

#[derive(Debug, Deserialize)]
struct StreamQuery {
    /// "flac" (default, the master) or opus96/opus128/opus160.
    quality: Option<String>,
}

async fn stream(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<StreamQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let db = st.db.clone();
    let found = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<(String, String)>> {
        let conn = db.open()?;
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

    // The policy caps the request: a box that only agrees to Opus never sends
    // the master, however the client asks.
    let policy = {
        let db = st.db.clone();
        tokio::task::spawn_blocking(move || -> anyhow::Result<crate::serve::Serve> {
            let conn = db.open()?;
            Ok(crate::serve::get(&conn))
        })
        .await??
    };
    let requested = q.quality.as_deref().and_then(crate::derive::Quality::parse);
    let effective = policy.cap(requested);

    // A phone on a metered connection asks for Opus; the master never changes.
    let (path, codec) = match effective {
        Some(quality) => {
            let cache = st.cache.clone();
            let source = std::path::PathBuf::from(&path);
            let mtime = std::fs::metadata(&source)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let derived = tokio::task::spawn_blocking(move || {
                cache.opus(&source, mtime, quality)
            })
            .await?
            .map_err(ApiError::internal)?;
            (derived.to_string_lossy().to_string(), "opus".to_string())
        }
        None => (path, codec),
    };
    let path = path.as_str();

    let file = tokio::fs::File::open(path)
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
