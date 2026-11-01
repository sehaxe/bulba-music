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
