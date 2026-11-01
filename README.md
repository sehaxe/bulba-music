# bulba music

Personal music server and player. Local-first: the catalogue lives on the server,
the client keeps its own mirror and plays offline.

- `server/` — Rust (axum + SQLite). Library scanning, tag reading, range streaming.
- `app/` — Flutter client (Android, Linux, macOS).
- `DESIGN.md` — the design system. Read before touching UI.

## server

```sh
cd server
cargo run -- scan --library ~/Music                    # import / refresh catalogue
cargo run -- --library ~/Music --db ~/data/bulba.db     # serve on :8422
```

Endpoints: `/api/health`, `/api/scan`, `/api/tracks`, `/api/tracks/{id}`,
`/api/tracks/{id}/stream` (byte ranges), `/api/search`, `/api/manifest?since=rev`.

```sh
cargo test
```
