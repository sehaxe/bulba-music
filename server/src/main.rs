use anyhow::Result;
use bulba_music_server::api::{router, AppState};
use bulba_music_server::db::Db;
use bulba_music_server::scan;
use std::net::SocketAddr;
use std::path::PathBuf;

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());

    let library = arg(&args, "--library")
        .or_else(|| std::env::var("BULBA_LIBRARY").ok())
        .unwrap_or(format!("{home}/Music"));
    let db_path = arg(&args, "--db")
        .or_else(|| std::env::var("BULBA_DB").ok())
        .unwrap_or(format!("{home}/.local/share/bulba-music/bulba.db"));

    let db = Db::new(db_path);
    db.init()?;

    if args.iter().any(|a| a == "scan") {
        let report = scan::scan(&db, &PathBuf::from(&library))?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let port: u16 = arg(&args, "--port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(8422);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let state = AppState {
        db,
        library: PathBuf::from(library),
    };

    tracing::info!("bulba music server on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}
