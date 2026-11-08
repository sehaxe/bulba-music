//! Live probe for the playlist calls. Run against the local server:
//! `cargo run --bin playlist_probe`.

#![allow(dead_code)]

#[path = "../api.rs"]
mod api;
#[path = "../playlists.rs"]
mod playlists;
#[path = "../theme.rs"]
mod theme;

use playlists::Playlists;

fn main() -> anyhow::Result<()> {
    let base = std::env::var("BULBA_SERVER").unwrap_or_else(|_| api::DEFAULT_SERVER.into());
    let api = api::Api::new(base);

    let before = Playlists::load(&api)?;
    println!("lists before: {:#?}", before.items);

    let name = format!("проверка {}", std::process::id());
    let id = Playlists::create(&api, &name)?;
    println!("create → {id} {name:?}");

    Playlists::add_track(&api, id, 2)?;
    println!("add track 2 → ok");

    let renamed = "проверка переименования";
    Playlists::rename(&api, id, renamed)?;
    println!("rename → {renamed:?}");

    let found = Playlists::load(&api)?
        .items
        .into_iter()
        .find(|p| p.id == id)
        .expect("created playlist is listed");
    println!("list → {found:?}");
    assert_eq!(found.name, renamed);
    assert_eq!(found.tracks, 1);

    let tracks = Playlists::tracks(&api, id)?;
    println!(
        "tracks → {:#?}",
        tracks
            .iter()
            .map(|t| (t.id, t.title.as_str(), t.artist.as_str()))
            .collect::<Vec<_>>()
    );
    assert_eq!(tracks.first().map(|t| t.id), Some(2));

    Playlists::remove(&api, id)?;
    println!("delete → ok");

    let after = Playlists::load(&api)?;
    println!("lists after: {:#?}", after.items);
    assert!(!after.items.iter().any(|p| p.id == id));
    Ok(())
}
