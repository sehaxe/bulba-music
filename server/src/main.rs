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

    let mut libraries: Vec<PathBuf> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--library" {
            if let Some(v) = args.get(i + 1) {
                libraries.push(PathBuf::from(v));
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    if libraries.is_empty() {
        // The library lives inside the project, untracked by git: one place to
        // back up and one place the phone syncs from.
        let env = std::env::var("BULBA_LIBRARY")
            .unwrap_or_else(|_| format!("{}/bulba-music/library", home));
        libraries = env.split(':').map(PathBuf::from).collect();
    }
    for lib in &libraries {
        std::fs::create_dir_all(lib).ok();
    }
    let mut ignores: Vec<String> = vec![
        "Telegram Desktop".into(),
        "ChatExport_".into(),
        // The organiser parks duplicates here; they must not be indexed twice.
        "/_duplicates/".into(),
    ];
    let mut j = 0;
    while j < args.len() {
        if args[j] == "--ignore" {
            if let Some(v) = args.get(j + 1) {
                ignores.push(v.clone());
            }
            j += 2;
            continue;
        }
        j += 1;
    }
    let db_path = arg(&args, "--db")
        .or_else(|| std::env::var("BULBA_DB").ok())
        .unwrap_or(format!("{home}/.local/share/bulba-music/bulba.db"));

    let db = Db::new(db_path);
    db.init()?;

    if args.iter().any(|a| a == "scan") {
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
        println!("{}", serde_json::to_string_pretty(&total)?);
        return Ok(());
    }

    let port: u16 = arg(&args, "--port")
        .and_then(|p| p.parse().ok())
        .unwrap_or(8422);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let state = AppState {
        db,
        libraries,
        ignores,
    };

    tracing::info!("bulba music server on http://{addr}");
    tracing::info!("library: {:?}", state.libraries);

    // The library belongs to the server: it scans itself on start and then on a
    // timer, so no client ever has to know about folders on disk.
    {
        let db = state.db.clone();
        let roots = state.libraries.clone();
        let ignores = state.ignores.clone();
        tokio::spawn(async move {
            loop {
                let (db, roots, ignores) = (db.clone(), roots.clone(), ignores.clone());
                let res = tokio::task::spawn_blocking(move || -> Result<scan::ScanReport> {
                    let mut total = scan::ScanReport::default();
                    for root in &roots {
                        let r = scan::scan(&db, root, &ignores)?;
                        total.scanned += r.scanned;
                        total.masters_added += r.masters_added;
                        total.tracks_created += r.tracks_created;
                        total.masters_removed += r.masters_removed;
                        total.errors.extend(r.errors);
                        total.rev = total.rev.max(r.rev);
                    }
                    Ok(total)
                })
                .await;
                match res {
                    Ok(Ok(r)) => tracing::info!(
                        "library scan: {} files, +{} masters, +{} tracks",
                        r.scanned,
                        r.masters_added,
                        r.tracks_created
                    ),
                    Ok(Err(e)) => tracing::warn!("library scan failed: {e}"),
                    Err(e) => tracing::warn!("library scan task: {e}"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
            }
        });
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}
