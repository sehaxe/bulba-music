use axum::body::Body;
use axum::http::{Request, StatusCode};
use bulba_music_server::api::{router, AppState};
use bulba_music_server::db::Db;
use http_body_util::BodyExt;
use std::io::Write;
use tower::ServiceExt;

/// A fixture: one real file on disk, one artist → album → track → master chain.
struct Fixture {
    _dir: tempfile::TempDir,
    state: AppState,
    id: i64,
    size: u64,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = dir.path().join("track.flac");
    let mut f = std::fs::File::create(&audio_path).unwrap();
    f.write_all(&vec![0x5au8; 4096]).unwrap();
    drop(f);

    let db = Db::new(dir.path().join("test.db"));
    db.init().unwrap();
    let conn = db.open().unwrap();
    conn.execute(
        "INSERT INTO meta(key,value) VALUES('rev','7') ON CONFLICT(key) DO UPDATE SET value='7'",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO artist(name,norm) VALUES('Aphex Twin','aphex twin')", [])
        .unwrap();
    let artist = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO album(title,artist_id,year,norm) VALUES('SAW 85-92',?1,1992,'aphex twin/saw 85-92')",
        [artist],
    )
    .unwrap();
    let album = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO track(title,album_id,disc,track_no,duration_ms,rev,created_at)
         VALUES('Xtal',?1,1,1,5000,7,0)",
        [album],
    )
    .unwrap();
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO master(path,bytes,mtime,codec,duration_ms,sample_rate,channels,bits,ingested_at)
         VALUES(?1,4096,0,'flac',5000,44100,2,16,0)",
        [audio_path.to_string_lossy().to_string()],
    )
    .unwrap();
    let master = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO track_master(track_id,master_id,role) VALUES(?1,?2,'primary')",
        [id, master],
    )
    .unwrap();
    conn.execute("INSERT INTO track_fts(rowid,title,artist,album) VALUES(?1,'Xtal','Aphex Twin','SAW 85-92')", [id])
        .unwrap();

    Fixture {
        _dir: dir,
        state: AppState {
            db,
            libraries: vec![std::path::PathBuf::from("/nonexistent")],
            ignores: vec![],
        },
        id,
        size: 4096,
    }
}

async fn body_bytes(resp: axum::response::Response) -> Vec<u8> {
    resp.into_body().collect().await.unwrap().to_bytes().to_vec()
}

#[tokio::test]
async fn health_reports_catalogue_size() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(json["tracks"], 1);
    assert_eq!(json["artists"], 1);
    assert_eq!(json["albums"], 1);
    assert_eq!(json["rev"], 7);
}

#[tokio::test]
async fn track_view_joins_artist_album_and_master() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(Request::get("/api/tracks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let tracks: Vec<serde_json::Value> =
        serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["title"], "Xtal");
    assert_eq!(tracks[0]["artist"], "Aphex Twin");
    assert_eq!(tracks[0]["album"], "SAW 85-92");
    assert_eq!(tracks[0]["codec"], "flac");
    assert_eq!(tracks[0]["year"], 1992);
}

#[tokio::test]
async fn range_request_returns_206_and_exact_bytes() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(
            Request::get(format!("/api/tracks/{}/stream", fx.id))
                .header("range", "bytes=100-199")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers()["content-range"],
        format!("bytes 100-199/{}", fx.size)
    );
    assert_eq!(body_bytes(resp).await.len(), 100);
}

#[tokio::test]
async fn unsatisfiable_range_returns_416() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(
            Request::get(format!("/api/tracks/{}/stream", fx.id))
                .header("range", "bytes=999999999-")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}

#[tokio::test]
async fn search_finds_track_and_ignores_fts_operators() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(Request::get("/api/search?q=xtal").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    let tracks = json["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["title"], "Xtal");
    // A track match must not invent an artist section for a query that does not
    // match the artist's name.
    assert!(json["artists"].as_array().unwrap().is_empty());

    let by_artist = router(fixture().state)
        .oneshot(
            Request::get("/api/search?q=aphex")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&body_bytes(by_artist).await).unwrap();
    assert_eq!(json["artists"].as_array().unwrap().len(), 1);
    assert_eq!(json["tracks"].as_array().unwrap().len(), 1);

    let admin = fixture();
    let resp = router(admin.state)
        .oneshot(
            Request::get("/api/search?q=%22title%3A+OR+%2A")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn manifest_only_returns_changes_after_rev() {
    let fx = fixture();
    let resp = router(fx.state.clone())
        .oneshot(Request::get("/api/manifest?since=7").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(json["tracks"].as_array().unwrap().len(), 0);

    let resp = router(fx.state)
        .oneshot(Request::get("/api/manifest?since=0").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(json["tracks"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn play_events_are_appended_verbatim() {
    let fx = fixture();
    let body = serde_json::json!([{
        "track_id": fx.id,
        "session_id": 1,
        "event": 3,
        "position_ms": 4980,
        "position_ratio": 0.996,
        "source": 4,
        "device": 0,
        "offline": true,
        "quality": 2,
        "ts": 1700000000
    }]);
    let resp = router(fx.state.clone())
        .oneshot(
            Request::post("/api/events")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(json["accepted"], 1);

    let conn = fx.state.db.open().unwrap();
    let (ts, source, offline): (i64, i64, i64) = conn
        .query_row("SELECT ts, source, offline FROM play_event", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((ts, source, offline), (1_700_000_000, 4, 1));
}

#[tokio::test]
async fn tailscale_user_outside_allowlist_is_forbidden() {
    let fx = fixture();
    let body = serde_json::json!([]);
    let resp = router(fx.state)
        .oneshot(
            Request::post("/api/events")
                .header("tailscale-user-login", "stranger@example.com")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    // hello@example.com is not in the allowlist, so writes are rejected.
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn like_roundtrip_and_taste_signal() {
    let fx = fixture();
    let resp = router(fx.state.clone())
        .oneshot(
            Request::post(format!("/api/likes/{}", fx.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router(fx.state.clone())
        .oneshot(Request::get("/api/likes/ids").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let ids: Vec<i64> = serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(ids, vec![fx.id]);

    let resp = router(fx.state.clone())
        .oneshot(Request::get("/api/likes").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let tracks: Vec<serde_json::Value> =
        serde_json::from_slice(&body_bytes(resp).await).unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["id"], fx.id);

    // Liking must not fabricate a replay event: event 4 means "played again".
    let conn = fx.state.db.open().unwrap();
    let events: i64 = conn
        .query_row("SELECT count(*) FROM play_event WHERE event=4", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events, 0);

    let resp = router(fx.state)
        .oneshot(
            Request::delete(format!("/api/likes/{}", fx.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn liking_a_missing_track_is_404() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(
            Request::post("/api/likes/9999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_remote_device_without_a_phrase_gets_401() {
    let fx = fixture();
    let mut req = Request::get("/api/likes").body(Body::empty()).unwrap();
    // A tailnet address instead of loopback: this is the phone.
    req.extensions_mut().insert(axum::extract::ConnectInfo(
        std::net::SocketAddr::from(([100, 64, 0, 1], 5555)),
    ));
    let resp = router(fx.state.clone()).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // The same request from this machine is the owner, no phrase needed.
    let mut req = Request::get("/api/likes").body(Body::empty()).unwrap();
    req.extensions_mut().insert(axum::extract::ConnectInfo(
        std::net::SocketAddr::from(([127, 0, 0, 1], 5555)),
    ));
    let resp = router(fx.state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
