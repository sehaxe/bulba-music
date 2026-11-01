use axum::body::Body;
use axum::http::{Request, StatusCode};
use bulba_music_server::api::{router, AppState};
use bulba_music_server::db::Db;
use http_body_util::BodyExt;
use std::io::Write;
use tower::ServiceExt;

/// A fixture: one real file on disk, one row in the catalogue pointing at it.
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
    conn.execute(
        "INSERT INTO tracks (path,size,mtime,title,artist,album,codec,rev,added_at)
         VALUES (?1, 4096, 0, 'Xtal', 'Aphex Twin', 'SAW 85-92', 'flac', 7, 0)",
        [audio_path.to_string_lossy().to_string()],
    )
    .unwrap();
    let id = conn.last_insert_rowid();

    Fixture {
        _dir: dir,
        state: AppState {
            db,
            library: std::path::PathBuf::from("/nonexistent"),
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
    assert_eq!(json["rev"], 7);
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
async fn full_request_returns_whole_file() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(
            Request::get(format!("/api/tracks/{}/stream", fx.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_bytes(resp).await.len() as u64, fx.size);
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
async fn missing_track_is_404() {
    let fx = fixture();
    let resp = router(fx.state)
        .oneshot(Request::get("/api/tracks/999/stream").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
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
