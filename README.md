# bulba music

Self-hosted music: the library lives on the server, the phone is a thin client
over Tailscale, and the recommendations are computed from what you actually
play — no catalogue, no cloud, no account on someone else's machine.

## Layout

```
server/    Rust: axum + SQLite, scanning, features, recommendations, pages
app/       Flutter: Linux/macOS/Android client
library/   the music itself — Artist/Album (Year)/NN - Title.ext, gitignored
```

`library/` is never committed. It is the one thing worth backing up.

## Run

```sh
cd server && cargo run --release              # http://0.0.0.0:8422
cd app && flutter run -d linux                # or: flutter build apk
```

The server scans `library/` on start and every five minutes. Clients never scan
a disk; they ask the server.

Overrides: `--library <path>` (repeatable), `--port`, `--db`, `BULBA_LIBRARY`,
`BULBA_SERVER` for the client.

## What is inside

- **Library**: tags, covers, FLAC-master first, per-track format shown honestly.
- **Index**: artists and albums grouped by letter with a rail; artist pages with
  albums in release order and what you actually replay; album pages in disc order.
- **Recommendations**: embeddings from the audio, four axes (tempo, brightness,
  library-relative loudness, major/minor), daily mixes per mood, novelty capped
  per list, taste as a weighted centroid of plays, skips, repeats and likes.
- **Accounts**: twelve words, wallet style; the same phrase on two devices is one
  listener. Only the hash is stored.
- **Links**: `/a/<artist>`, `/al/<album>` — server-rendered, open from a phone.

## Tests

```sh
cd server && cargo test      # 60+ tests, no network
cd app && flutter test
```
