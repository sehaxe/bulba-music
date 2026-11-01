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
        .route("/api/likes", get(list_likes))
        .route("/api/likes/ids", get(like_ids))
        .route("/api/likes/{id}", post(add_like).delete(remove_like))
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
}

const TRACK_COLUMNS: &str = "id, title, artist, album, album_id, has_cover, year, disc,
    track_no, duration_ms, codec, bitrate, sample_rate, bits, channels, bytes, master_count,
    created_at, rev";

fn map_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
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

#[derive(Debug, Deserialize)]
struct BuildQuery {
    limit: Option<i64>,
}

/// Computes content embeddings for tracks that do not have one yet.
/// The client calls this after a scan; it is incremental and safe to repeat.
async fn build_features(
    State(st): State<AppState>,
    Query(q): Query<BuildQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = q.limit.unwrap_or(200).clamp(1, 5000);
    let db = st.db.clone();
    let stats = tokio::task::spawn_blocking(move || -> anyhow::Result<(usize, i64)> {
        let conn = db.open()?;
        let rows: Vec<(i64, String)> = {
            let mut stmt = conn.prepare(
                "SELECT t.id, m.path
                 FROM track t
                 JOIN track_master tm ON tm.track_id = t.id
                 JOIN master m ON m.id = tm.master_id
                 WHERE NOT EXISTS (
                     SELECT 1 FROM embed e WHERE e.track_id = t.id AND e.model = ?1)
                 GROUP BY t.id
                 ORDER BY t.id
                 LIMIT ?2",
            )?;
            stmt.query_map(rusqlite::params![crate::features::MODEL, limit], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect()
        };
        let mut done = 0usize;
        for (id, path) in &rows {
            match crate::features::extract(path) {
                Ok(fp) => {
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
                    done += 1;
                }
                Err(e) => tracing::warn!("features failed for {path}: {e}"),
            }
        }
        let remaining: i64 = conn.query_row(
            "SELECT count(*) FROM track t WHERE NOT EXISTS (
                 SELECT 1 FROM embed e WHERE e.track_id = t.id AND e.model = ?1)",
            [crate::features::MODEL],
            |r| r.get(0),
        )?;
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
async fn home_mixes(State(st): State<AppState>) -> Result<Json<Vec<Mix>>, ApiError> {
    let db = st.db.clone();
    let mixes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Mix>> {
        let conn = db.open()?;
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
async fn daily_mixes(State(st): State<AppState>) -> Result<Json<Vec<Mix>>, ApiError> {
    let db = st.db.clone();
    let mixes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Mix>> {
        let conn = db.open()?;
        crate::daily::ensure(&conn, crate::features::MODEL)?;
        let rows = crate::daily::load(&conn)?;
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

fn current_user(conn: &rusqlite::Connection) -> anyhow::Result<i64> {
    crate::db::default_user(conn)
}

async fn list_likes(State(st): State<AppState>) -> Result<Json<Vec<Track>>, ApiError> {
    let db = st.db.clone();
    let tracks = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<Track>> {
        let conn = db.open()?;
        let user = current_user(&conn)?;
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

async fn like_ids(State(st): State<AppState>) -> Result<Json<Vec<i64>>, ApiError> {
    let db = st.db.clone();
    let ids = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<i64>> {
        let conn = db.open()?;
        let user = current_user(&conn)?;
        let mut stmt = conn.prepare("SELECT track_id FROM track_like WHERE user_id = ?1")?;
        let rows = stmt.query_map([user], |r| r.get(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    })
    .await??;
    Ok(Json(ids))
}

async fn add_like(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    let ok = tokio::task::spawn_blocking(move || -> anyhow::Result<bool> {
        let conn = db.open()?;
        let user = current_user(&conn)?;
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
        // A like is the strongest positive signal we get; log it as an event so
        // the taste model sees it even if the track is never played again.
        if let Some(track) = conn
            .query_row("SELECT id FROM track WHERE id=?1", [id], |r| r.get::<_, i64>(0))
            .ok()
        {
            conn.execute(
                "INSERT INTO play_event(ts,user_id,track_id,session_id,event,position_ms,
                    position_ratio,source,device,offline,quality,client_id)
                 VALUES(unixepoch(),?1,?2,0,4,0,0,0,3,0,0,'like')",
                rusqlite::params![user, track],
            )?;
        }
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
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = st.db.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let conn = db.open()?;
        let user = current_user(&conn)?;
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

async fn cover(
    State(st): State<AppState>,
    Path(id): Path<i64>,
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
