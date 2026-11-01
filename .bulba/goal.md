# GOAL — bulba music, autonomous pass

MODE: AWAY
Started: 2026-11-02 01:55 (owner went to sleep)

## Task from the owner

«работай автономно реализуй все фичи рекомендации также сделай вкладку волна
чтобы типо оно полностью давала песни на то как думает и думала что давать на
базе того скипает лайкает ли и тд»

Translation of the intent: finish the recommendation surface, and add a «Волна»
tab — an endless personal stream that keeps deciding the next track from what the
listener skips, likes and finishes, not from a fixed playlist.

## Acceptance

1. «Волна» is a first-class tab (nav), starts from taste alone or from a seed
   track, never stops, and refills itself before the queue runs dry.
2. Every track it picks carries a short, honest reason («потому что ты часто
   слушаешь Tame Impala», «похоже по звуку на X», «новое, но рядом с твоим
   вкусом»), shown in the wave screen.
3. Skips and likes change the next picks within the same session — measurable,
   not decorative: a skipped track's artist and neighbours lose weight, a like
   gains it.
4. Recommendations are per listener: a fresh device sees an empty wave, not the
   desktop's history. No stats, mixes or taste leak between accounts.
5. `cargo test` and `flutter test` green; `flutter analyze` clean; release
   published through CI.

## Rules I work by

- One feature at a time, commit per task, no push without it being green.
- No invented numbers in the final report: every claim points at a test or a
  measured run.
- If something is cut for time, say so and leave it as a named next step.

---

## STATUS: DONE

Finished 2026-11-02 02:40.

### What was built

- **The embedding was broken and is fixed.** The mel mean was taken over sums
  that had not been averaged per frame, so every track produced the same
  vector and every "similar" list in the app was noise. Tests now embed a sine
  and white noise and require them to differ, the 440 Hz sine to land in the
  low mel bands, and vectors to stay normalised. All 504 embeddings were
  recomputed with a six-worker build pass (endpoint decodes in parallel now).
- **`/api/recs/wave`** — the stream the owner asked for: scores from plays,
  completions, likes and skips; at most two tracks per artist and one per song;
  every pick carries the term that won as a sentence. Cold start tours the
  library with farthest-point sampling instead of guessing.
- **«Волна» tab** — endless, refills itself before the queue ends, novelty
  slider from familiar to unknown, inline like/skip, reason on every row.
  «Радио от этого трека» now seeds the same stream.
- **Tags and categories** — live, remix, acoustic, demo, instrumental,
  remaster, edit, deluxe, ost from title/album markers, plus a format badge;
  shown in rows, the details sheet and the player.
- **Per-listener everything** — stats, daily mixes and taste are scoped to the
  user; a device that is not the server's machine is asked who is listening and
  starts blank.
- **Phone layout** — the player art sizes from the screen, the waveform stops
  clipping, the library header wraps, the dock is opaque and its highlight is
  centred on the icon, album rows stop repeating the album name.
- **Opus on demand and webp covers** (landed earlier in the same pass): 2.9 MB
  per track at 128k instead of 28 MB of FLAC, 65 KB covers instead of 2 MB.

### Evidence

- `cargo test` — 68 tests, green; includes skip penalty, cold-start spread,
  artist cap, non-empty reasons, per-user isolation of the auth fallback.
- `flutter test` — 7 tests, green (waveform geometry and seek mapping,
  crossfade equal-power maths).
- `flutter analyze` — no issues.
- End-to-end with fresh tokens: fresh device → `taste_ready=false`, zero stats,
  six tracks across four artists; after liking one Nirvana track the next batch
  opened with two Nirvana tracks and said «новое, но рядом с Nirvana».
- Releases v0.0.2 … v0.0.7 built by CI on tag, three artefacts each (APK arm64
  24 MB, Linux bundle 9 MB, server binary 10 MB).

### Left for the owner

- Offline downloads on the phone are still not implemented (streams only).
- Gapless and crossfade work but were not listened to on a real device.
- The Android back gesture and the adaptive icon are in the build but need a
  real phone to confirm.
- 99 FLAC files in the old downloads folder are corrupt and were left alone.
