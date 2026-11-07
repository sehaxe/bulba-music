//! Headless check for the MPRIS server. Starts it, pushes a fake track, then
//! asks the real session bus — no window, no GUI.
//!
//! `cargo run --bin mpris_probe`

#[path = "../api.rs"]
#[allow(dead_code)] // the probe only needs `Track`
mod api;
#[path = "../mpris.rs"]
mod mpris;

use std::{process::Command, sync::mpsc, thread, time::Duration};

fn dbus(args: &str) -> anyhow::Result<String> {
    let out = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--dest=org.mpris.MediaPlayer2.bulba",
            "/org/mpris/MediaPlayer2",
        ])
        .args(args.split_whitespace())
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "dbus-send {args}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn micros(reply: &str) -> anyhow::Result<i64> {
    let tail = reply
        .rsplit("int64 ")
        .next()
        .ok_or_else(|| anyhow::anyhow!("no int64 in reply: {reply}"))?;
    Ok(tail.trim().parse()?)
}

fn main() -> anyhow::Result<()> {
    let (next_tx, next_rx) = mpsc::channel();
    let (seek_tx, seek_rx) = mpsc::channel();
    let (vol_tx, vol_rx) = mpsc::channel();

    let mpris = mpris::Mpris::start(
        || {},
        || {},
        move || {
            let _ = next_tx.send(());
        },
        || {},
        move |secs| {
            let _ = seek_tx.send(secs);
        },
        move |vol| {
            let _ = vol_tx.send(vol);
        },
    )?;

    let track = api::Track {
        id: 71,
        title: "Probe Track".into(),
        artist: "Probe Artist".into(),
        album: "Probe Album".into(),
        duration_ms: 200_000,
        ..Default::default()
    };
    mpris.set(Some(&track), true, 42.0, 0.7);

    // The server thread applies snapshots asynchronously.
    let mut status = String::new();
    for _ in 0..50 {
        status = dbus(
            "org.freedesktop.DBus.Properties.Get string:org.mpris.MediaPlayer2.Player \
             string:PlaybackStatus",
        )?;
        if status.contains("Playing") {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    println!("--- PlaybackStatus ---\n{status}");
    anyhow::ensure!(
        status.contains("Playing"),
        "expected Playing, got: {status}"
    );

    let meta = dbus(
        "org.freedesktop.DBus.Properties.Get string:org.mpris.MediaPlayer2.Player \
         string:Metadata",
    )?;
    println!("--- Metadata ---\n{meta}");
    anyhow::ensure!(
        meta.contains("Probe Track") && meta.contains("xesam:artist") && meta.contains("Probe Album"),
        "metadata incomplete: {meta}"
    );

    dbus("org.mpris.MediaPlayer2.Player.Next")?;
    next_rx.recv_timeout(Duration::from_secs(2))?;
    println!("Next -> callback fired");

    // Seek is a relative offset: on_seek must get position + 10s.
    let before = micros(&dbus(
        "org.freedesktop.DBus.Properties.Get string:org.mpris.MediaPlayer2.Player \
         string:Position",
    )?)?;
    dbus("org.mpris.MediaPlayer2.Player.Seek int64:10000000")?;
    let secs = seek_rx.recv_timeout(Duration::from_secs(2))?;
    println!("Seek(+10s) from {}us -> on_seek({secs}s)", before);
    anyhow::ensure!(
        ((secs * 1e6) as i64 - before - 10_000_000).abs() < 1_000_000,
        "expected ~{}s, got {secs}",
        before as f64 / 1e6 + 10.0
    );

    dbus(
        "org.freedesktop.DBus.Properties.Set string:org.mpris.MediaPlayer2.Player \
         string:Volume variant:double:0.5",
    )?;
    let vol = vol_rx.recv_timeout(Duration::from_secs(2))?;
    println!("SetVolume(0.5) -> on_volume({vol})");
    anyhow::ensure!((vol - 0.5).abs() < 1e-9, "expected 0.5, got {vol}");

    println!("OK");
    Ok(())
}
