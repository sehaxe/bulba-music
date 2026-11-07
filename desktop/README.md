# bulba music — desktop

A native client for the same server the phone talks to. Rust, [GPUI](https://gpui.rs)
(the renderer behind Zed), Vulkan through Blade, Wayland first and X11 as a
fallback. No webview, no Electron, no Flutter engine: the window is a GPU surface
and every pixel of the interface is drawn by this crate.

The phone keeps its Flutter shell. GPUI has no Android backend (the published
crate ships `linux`, `mac`, `windows` and nothing else), so a desktop client is
the honest way to use it — and it is the only client that can be translucent
over the real desktop.

## Build and run

```sh
cd desktop
cargo build --release
BULBA_SERVER=http://127.0.0.1:8422 ./target/release/bulba-desktop
```

Needs Rust 1.88+, Vulkan, fontconfig and either Wayland or X11. The server URL
defaults to `http://127.0.0.1:8422`.

The binary reads icons from `assets/icons/` next to the crate (dev) or next to
the executable (release).

## What it does

- library: albums, artists, liked, search, one endless recommendation wave
- playback: two rodio decks on one mixer with an equal-power crossfade whose
  length comes from the two Camelot codes; next-track prefetch; ReplayGain from
  the stored LUFS; play/skip/completion events posted back to the taste model
- covers: JPEG XL for the room, webp for the grid, and a blurred 256 px version
  of every cover becomes the "room" the whole window is lit by
- MPRIS: media keys and the desktop's player widget (verified with `playerctl`
  and raw `dbus-send`)

## The glass

The window is transparent (`WindowBackgroundAppearance::Blurred`) and asks the
compositor for a blurred backdrop. Under niri 26.04 the blur is real — measured
σ ≈ 9.5 px, and the alpha compositing matches the arithmetic to within 3 RGB.

Two things worth knowing:

- **Server-side decorations, not client-side.** niri draws its focus ring as a
  solid slab *behind* client-decorated windows, which replaces the desktop with
  a blue rectangle when the window has focus. With `WindowDecorations::Server`
  there is no slab, no titlebar, and the desktop stays visible.
- **Fullscreen is opaque.** niri's `fullscreen_backdrop_color` is black, so a
  fullscreen window has nothing to be transparent over. The glass only shows in
  windowed mode.

The album art is still the room: the desktop shows through it at ~35 %, the art
keeps its colour, and the palette flips to a light room for bright covers.

## Opus, and why there is a decoder in here

The server's streaming policy caps every client at Opus, so `?quality=flac`
still returns an Ogg Opus file. `rodio` has no Opus decoder, so `track_source.rs`
sniffs the container and decodes Ogg Opus with [`opus-pure`](https://crates.io/crates/opus-pure)
— pure Rust, no libopus, pre-skip handled, and a seek that decodes from the top
(a second per four minutes skipped).

## Review harness

Same idea as the phone's `BULBA_*` variables: a screen reachable only by three
clicks is a screen nobody looks at.

| variable | effect |
|---|---|
| `BULBA_TAB=home\|wave\|search\|library\|liked\|settings` | open that tab |
| `BULBA_ALBUM=<id>` | open an album |
| `BULBA_PLAY=<track id>` | start that track's album |
| `BULBA_SEARCH=<query>` | run a search |
| `BULBA_COVER_LOG=1` | cover fetch/decode timings on stderr |
| `BULBA_AUDIO_LOG=1` | playback engine state on stderr |

## Not wired yet

Settings, playlists, casting and a release pipeline. The wire formats, the
player and the room are done.
