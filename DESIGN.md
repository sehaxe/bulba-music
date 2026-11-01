---
version: alpha
name: Bulba Music Design System
description: |
  A music player rendered as a man page. Every glyph is JetBrains Mono, the chrome is
  warm cream (`#fdfcfc`) with near-black ink (`#201d1d`), containers are sharp rectangles
  with 1px hairlines, and every interactive element sits in a 4px-radius rectangle.
  There are no shadows, no gradients, no glass, no icon set: icons are bracketed ASCII
  (`[>]`, `[||]`, `[+]`). Exactly one surface inverts to dark — the full-screen player —
  and exactly one color is allowed: the dominant hue lifted from the album art, used
  once, as the playhead. Album art is the only raster image in the product.

colors:
  canvas: "#fdfcfc"
  ink: "#201d1d"
  ink-deep: "#0f0000"
  charcoal: "#302c2c"
  body: "#424245"
  mute: "#646262"
  stone: "#6e6e73"
  ash: "#9a9898"
  surface-soft: "#f8f7f7"
  surface-card: "#f1eeee"
  surface-dark: "#201d1d"
  surface-dark-elevated: "#302c2c"
  hairline: "rgba(15,0,0,0.12)"
  hairline-strong: "#646262"
  on-dark: "#fdfcfc"
  on-dark-mute: "#9a9898"
  signal: "#c2410c"
  danger: "#ff3b30"
  success: "#30d158"
  warning: "#ff9f0a"

typography:
  font-family: "JetBrains Mono"
  fallback: "IBM Plex Mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
  display-hero:
    fontSize: 44px
    fontWeight: 700
    lineHeight: 1.1
  display-l:
    fontSize: 32px
    fontWeight: 700
    lineHeight: 1.2
  display-m:
    fontSize: 24px
    fontWeight: 700
    lineHeight: 1.25
  heading:
    fontSize: 16px
    fontWeight: 700
    lineHeight: 1.5
  body:
    fontSize: 15px
    fontWeight: 400
    lineHeight: 1.6
  body-strong:
    fontSize: 15px
    fontWeight: 500
    lineHeight: 1.6
  label:
    fontSize: 13px
    fontWeight: 500
    lineHeight: 1.5
  caption:
    fontSize: 12px
    fontWeight: 400
    lineHeight: 1.6
  micro:
    fontSize: 11px
    fontWeight: 500
    lineHeight: 1.4

rounded:
  none: 0px
  art: 2px
  sm: 4px
  full: 9999px

spacing:
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 32px
  section: 64px

components:
  nav-rail:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    width: 232px
    border-right: "1px solid {colors.hairline}"
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.mute}"
    padding: "6px 12px"
  nav-item-active:
    backgroundColor: "{colors.surface-card}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
  tab-strip:
    backgroundColor: "{colors.canvas}"
    border-bottom: "1px solid {colors.hairline-strong}"
  tab:
    textColor: "{colors.mute}"
    padding: "8px 16px"
  tab-active:
    textColor: "{colors.ink}"
    border-bottom: "2px solid {colors.ink}"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.canvas}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    height: 32px
    padding: "0 16px"
  button-primary-active:
    backgroundColor: "{colors.ink-deep}"
  button-bracket:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "4px 0"
  button-bracket-active:
    textColor: "{colors.signal}"
  text-input:
    backgroundColor: "{colors.surface-soft}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "8px 12px"
    height: 36px
    border: "1px solid {colors.hairline}"
  text-input-focused:
    backgroundColor: "{colors.canvas}"
    border: "1px solid {colors.ink}"
  list-row:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    padding: "8px 0"
    border-bottom: "1px solid {colors.hairline}"
  list-row-hover:
    backgroundColor: "{colors.surface-soft}"
  list-row-playing:
    textColor: "{colors.ink}"
    backgroundColor: "{colors.surface-card}"
  album-art:
    rounded: "{rounded.art}"
    border: "1px solid {colors.hairline}"
    width: 44px
  album-art-large:
    rounded: "{rounded.art}"
    border: "1px solid {colors.hairline}"
  progress-track:
    textColor: "{colors.ash}"
    typography: "{typography.body}"
  progress-played:
    textColor: "{colors.signal}"
    typography: "{typography.body}"
  player-bar:
    backgroundColor: "{colors.canvas}"
    border-top: "1px solid {colors.hairline-strong}"
    height: 72px
  player-full:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.on-dark}"
  player-full-meta:
    textColor: "{colors.on-dark-mute}"
  queue-row:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.on-dark}"
    padding: "6px 0"
    border-bottom: "1px solid rgba(253,252,252,0.08)"
  badge:
    backgroundColor: "{colors.surface-card}"
    textColor: "{colors.mute}"
    typography: "{typography.micro}"
    rounded: "{rounded.sm}"
    padding: "2px 6px"
  badge-signal:
    backgroundColor: "{colors.signal}"
    textColor: "{colors.canvas}"
  footer-status:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.mute}"
    typography: "{typography.caption}"
    border-top: "1px solid {colors.hairline}"
---

## Overview

The product is a personal music server and player, and its interface is a **man page that plays**.
Nothing is drawn that could be typed: separators are hairline rules, icons are bracketed ASCII,
and the only images are album covers. The chrome never leaves monochrome cream-and-ink. The
single inversion to dark — the full-screen player — is the product's one dramatic gesture, the
way OpenCode's hero TUI is its one dark block.

**The three rules that decide every dispute:**

1. **Type is the interface.** If a control can be a word in brackets, it is a word in brackets.
   `[play]` beats a triangle icon. `[+]` beats a heart.
2. **Color appears once.** The album cover's dominant hue becomes `{colors.signal}` and is used
   for exactly two things: the played portion of the progress line, and the active-state accent
   on the currently playing row. Never for backgrounds, never for chrome.
3. **Dark is a room, not a theme.** `{colors.surface-dark}` exists for one screen: the full-screen
   player. It is not a dark mode and the rest of the app never inverts.

**Key characteristics**
- 100% monospace typography — no proportional font anywhere, including numbers and controls
- Cream canvas (`#fdfcfc`) as the only light surface; no gray section bands, no cards with shadows
- 1px hairline rules as the only separators; 4px radius only on interactive elements
- Bracketed ASCII as the entire icon language (`[>]`, `[||]`, `[>>]`, `[<<]`, `[~]`, `[+]`, `[-]`)
- ASCII meters instead of sliders — progress and volume are rows of block glyphs
- One dark surface (full-screen player), one accent color (from the cover), zero drop shadows
- Motion is typographic: characters flip, rows redraw, a cursor sweeps. No blur, no glass.

## Colors

### Canvas & ink
- **Canvas** (`{colors.canvas}` — `#fdfcfc`): every light surface. Warm, faintly pink cast. Never `#ffffff`.
- **Ink** (`{colors.ink}` — `#201d1d`): type, hairlines of emphasis, primary button fill.
- **Ink deep** (`{colors.ink-deep}` — `#0f0000`): pressed primary button.
- **Body** (`{colors.body}` — `#424245`): secondary prose, metadata under a title.
- **Mute / Stone / Ash**: the three-step de-emphasis ladder for labels, counts and disabled text.

### Surfaces
- **Soft** (`{colors.surface-soft}` — `#f8f7f7`): hovered rows, input fields.
- **Card** (`{colors.surface-card}` — `#f1eeee`): active nav item, currently-playing row tint, badges.
- **Dark** (`{colors.surface-dark}` — `#201d1d`): full-screen player only.
- **Dark elevated** (`{colors.surface-dark-elevated}` — `#302c2c`): queue rows and inset panels inside the dark player.

### Signal
- **Signal** (`{colors.signal}` — runtime): the dominant hue of the current cover, quantized to
  OKLCH with chroma forced to 0.12–0.16 and lightness to 0.55 so it never fails contrast on cream
  or on dark. Used for: played progress glyphs, the `[playing]` marker, the active tab underline
  on the player screen. Two places. That is the whole color budget.

### Semantic
`{colors.danger}` `{colors.success}` `{colors.warning}` appear only in status text — an offline
badge, a failed download, a scan error. Never as fills, never as icons.

## Typography

**JetBrains Mono** — free, and within ~3% of Berkeley Mono's metrics at body sizes. Weights 400,
500, 700. The variable axis is used for one effect only: weight animating 400→700 on the track
title during a track change (see Motion).

| Token | Size | Weight | LH | Use |
|---|---|---|---|---|
| `{typography.display-hero}` | 44 | 700 | 1.1 | Album title on the dark player, desktop |
| `{typography.display-l}` | 32 | 700 | 1.2 | Album title on the dark player, phone |
| `{typography.display-m}` | 24 | 700 | 1.25 | Section headers, empty states |
| `{typography.heading}` | 16 | 700 | 1.5 | Section labels, `library` / `downloads` |
| `{typography.body}` | 15 | 400 | 1.6 | Track titles, rows, prose |
| `{typography.body-strong}` | 15 | 500 | 1.6 | Artist name, emphasis |
| `{typography.label}` | 13 | 500 | 1.5 | Buttons, tabs, nav items |
| `{typography.caption}` | 12 | 400 | 1.6 | Durations, counts, file sizes |
| `{typography.micro}` | 11 | 500 | 1.4 | Badges, keybind hints, status bar |

Rules: durations, track numbers and counts are **tabular-aligned right** with the last column's
right edge on the container's content edge. Never truncate a title with `…` in the player — wrap
to two lines max, then the second line ellipsizes. Uppercase is forbidden; the system stays
lowercase like a man page, except the ASCII wordmark.

## Layout

### Grid
Base unit 8px (with 2/4px steps for tight inline). Content column max 960px on desktop, flush
left inside the nav rail. Mobile padding `{spacing.lg}`; desktop `{spacing.xl}`.

### Shell — desktop
```
┌───────────┬──────────────────────────────────────────────┐
│ [bulba]   │  library                                     │
│           │  ───────────────────────────────────────────  │
│ ▸ library │  a                                           │
│   albums  │  ───────────────────────────────────────────  │
│   radio   │  artist ─────────────────────  albums  plays  │
│   downloads│ Aphex Twin ──────────────────  14     312   │
│   settings│  Autechre ────────────────────  22     108   │
│           │                                              │
├───────────┴──────────────────────────────────────────────┤
│ [>]  Xtal — Aphex Twin   ████████░░░░░░░░░ 04:12 / 04:52 │
└──────────────────────────────────────────────────────────┘
```
- **Rail** (`{component.nav-rail}`, 232px): wordmark, nav items as lowercase words with a `▸`
  marker for the active one, a hairline right edge.
- **Player bar** (`{component.player-bar}`, 72px): hairline top, one row — `[>]` control, title,
  ASCII progress, time. Clicking the title area opens the dark player.
- **Status bar** (`{component.footer-status}`, 24px): library counts, sync state, keybind hints
  (`/ search   q queue`) — always visible, like a TUI status line.

### Mobile
Bottom `{component.tab-strip}` with four items; the mini-player sits above it, 56px, hairline top.
The rail disappears, the wordmark moves into the library header.

### Whitespace
Sections are separated by `{spacing.section}` (64px) and a hairline; rows are dense (8px vertical)
because a man page is dense. Air lives **between** blocks, never inside rows.

## Elevation & Depth

There is no elevation. The ladder is:

| Level | Treatment | Use |
|---|---|---|
| 0 | flat on canvas | everything |
| 1 | 1px `{colors.hairline}` | row separators, card edges |
| 2 | 1px `{colors.hairline-strong}` | player bar top, tab strip bottom |
| 3 | `{colors.surface-card}` fill | active row, badge, active nav item |
| 4 | `{colors.surface-dark}` | full-screen player — the only "above" |

No shadow token exists. If a design needs a shadow to work, the design is wrong.

## Shapes

Two radii: `{rounded.art}` (2px) on covers, `{rounded.sm}` (4px) on interactive elements and
inputs, `{rounded.none}` (0px) on every container. Avatars do not exist in the product.

**Album art** is the only raster: square, 1px hairline border, 2px radius. It is never blurred,
never tinted, never used as a background. Missing art renders an ASCII hatch block:

```
+----------+
| //////// |
| //////// |
| //////// |
+----------+
```

## Components

### Navigation
**`{component.nav-rail}`** — wordmark in block-pixel ASCII at the top (five rows, see Assets),
then nav. Items are lowercase words; the active one gets the `▸ ` prefix and `{colors.surface-card}`
fill. Counts sit right-aligned in `{typography.caption}` `{colors.ash}` (`library 2,140`).

### Buttons
**`{component.button-primary}`** — ink fill, canvas text, 32px, 4px radius. Reserved for the one
destructive-adjacent action on a screen (`scan library`, `download 14 tracks`).
**`{component.button-bracket}`** — the default. Plain text in brackets: `[play]` `[shuffle]`
`[add to queue]`. Active state turns `{colors.signal}`. Pressed state shifts 1px down and swaps
the bracket pair for `⟨ ⟩` (no animation, instant — terminals don't ease).

### List rows
**`{component.list-row}`** — the workhorse. Four columns:
`index  title / artist                          format  duration`
Index is `{colors.ash}`, right-aligned in a 3ch gutter. Playing row swaps the index for `[>]` in
`{colors.signal}` and gets `{colors.surface-card}` fill. No hover lift, only background change.

### Meters
Progress and volume are **glyph rows**, not sliders:
```
progress   ████████████░░░░░░░░░░░░░░  04:12
volume     ███████░░░░░░░░  0.7
```
`█` played, `░` remaining, rendered in `{typography.body}`. Played span uses `{colors.signal}` on
the player screen and `{colors.ink}` in the bar. Seeking = tap/drag on the glyph row; the glyph
under the pointer flips to `▌` while dragging — a caret, not a knob.

### Inputs
**`{component.text-input}`** — a `>` prompt prefix inside the field, monospace, no placeholder
in gray italics. Search focus is signalled by the prefix turning `{colors.signal}` and a blinking
`▌` at the caret. `esc` clears, `enter` plays the first result.

### Player
**`{component.player-bar}`** — desktop, 72px, hairline top:
`[>]  title — artist   ████░░░░ 04:12  [=]  [+]:`  (queue, like, volume)
**`{component.player-full}`** — the dark room. Layout, phone:
```
                    [x]
        ┌───────────────────┐
        │                   │
        │      cover        │
        │                   │
        └───────────────────┘
         track title
         artist · album · year
   ███████████░░░░░░░░░░░░░
   01:24                          03:48
   [<<]      [||]      [>>]
   [~]   [+]   [=]   [↓]
```
Cover is 78% of screen width, centered, hairline border. Title in
`{typography.display-l}`, artist in `{colors.on-dark-mute}`. Controls are bracket text at
`{typography.body}`, 44px tap targets, no circles, no fills.

**`{component.queue-row}`** — inside the dark player, opened by `[=]`: a `ps`-style listing,
`▸` marking the current item, durations right-aligned.

#### Badges
**`{component.badge}`** — `[flac]`, `[320]`, `[offline]`, `[cached]` in `{typography.micro}`.
Downloaded tracks show `[↓]` as a 1ch marker before the title, not a filled chip.
**`{component.badge-signal}`** — used only for `[playing]` during a transition, appears for 2s.

### Format & quality
The master file is never re-encoded. The UI always states what you are listening to:

```
[flac]  lossless master, streamed on wi-fi, ~28 MB/track
[alac]  lossless master (apple container)
[320]   lossy master, mp3 320
[256]   lossy master, aac/mp3 256
[opus128] access copy produced by the server, ~3.8 MB/track
[mix ready] stems cached on disk, transitions can be stem-level
```

Format sits in its own right-aligned column before the duration, in `{typography.micro}`
`{colors.ash}`; on the playing row it turns `{colors.ink}`. The download sheet is a two-option
list, never a dropdown:

```
+------------------------------------------+
| download  Xtal                           |
| [original flac · 28 MB]                  |
| [opus 128 · 3.8 MB]                      |
+------------------------------------------+
```

Stems are prepared lazily: `[prepare mix]` on a queue row, or automatically for the next 5 tracks
when crossfade is enabled. Until then the track shows no stems badge at all — absence is the state.

## Motion

Motion is **typographic**, not spatial: glyphs change state instead of things sliding through
space. All timings are instant-feeling; nothing exists to be admired.

| Event | Behaviour | Duration |
|---|---|---|
| Row tap | row background flips to `{colors.surface-card}`, no scale | 90ms |
| Track change (title) | characters resolve left→right from a random glyph (terminal redraw), weight 400→700→400 | 240ms |
| Cover art reveal | ordered 4×4 Bayer dither dissolves the art in from `{colors.surface-card}` | 240ms |
| Screen change (desktop) | new content fades in over a 1px vertical `▌` cursor that sweeps left→right | 180ms |
| Screen change (mobile) | horizontal push, 8px overshoot, hairline still visible during travel | 240ms |
| Player open (bar → full) | dark surface expands from the bar's top hairline upward; content fades 60ms later | 320ms |
| Player close | reverse, with velocity from the drag | spring, stiffness 380 / damping 32 |
| Pause | progress glyphs stop; the playhead `▌` blinks at 1Hz | — |
| Scan / sync | status bar shows a 12-frame spinner made of `|/-\` at 8fps | — |

No easing curves with bounce except the player drag spring. No parallax. No blur. No shaders.
`MediaQuery.disableAnimations` → every entry collapses to a 60ms crossfade.

## Assets

**Wordmark** (block-pixel, 5 rows, `{colors.ink}` on cream, `{colors.on-dark}` on dark):

```
█▀▄ █ █ █   █▀▄ █▀█
█▀▄ █ █ █   █▀▄ █▀█
▀▀  ▀▀▀ ▀▀  ▀▀  ▀ ▀
```

**Icon set** — complete, no others exist:
```
[>]  play      [||] pause     [>>] next      [<<] prev
[~]  shuffle   [↻]  repeat    [+]  like      [-]  unliked
[=]  queue     [↓]  download  [x]  close     [/]  search
[▸]  current   [ ]  empty     [!]  warning   [·]  idle
```

**Meters**: `█` `▉` `▊` `▋` `▌` (played gradient at sub-cell resolution), `░` remainder.

## Screens

1. **library** — index of artists: `name · albums · tracks`, hairline rows, alphabetical jump
   strip on the right edge (`a b c … z` at `{typography.micro}`).
2. **artist** — header with name and counts, then albums as a grid of covers 160px, each with a
   two-line caption (`album`, `year · n tracks`).
3. **album** — cover 220px left, title/artist/year right, then the track table. `[play all]`.
4. **player** — the dark room described above.
5. **queue** — dark, `ps` listing, drag to reorder by the `▸` gutter.
6. **radio** — seed selector, then a chain: `[seed] → 8 tracks`, each row showing why
   (`sim 0.91 · key ok · 122 bpm`) in `{colors.ash}`. This screen makes the algorithm legible.
7. **sound map** — a monochrome scatter of the library, one glyph per 12 tracks (`·`), the
   current cluster brighter; drag = roam. Click a region and a radio starts there.
8. **downloads** — `df -h` style table: `album · tracks · size · state (queued/↓/cached)`.
9. **search** — `/` prompt at the top, results grouped `tracks / albums / artists`, keyboard-first.
10. **settings** — server address, sync, crossfade seconds, `[x]` checkboxes as `[x]`/`[ ]` text.

## Do / Don't

**Do**
- Type every control as a word in brackets before drawing anything.
- Keep the cream canvas as the only light background, always.
- Right-align every numeric column, tabular, same width.
- Let album art be the only color and the only image.
- Use the status bar for machine truth (counts, sync, bitrate).

**Don't**
- No shadows, gradients, glass, blur, rounded cards, floating action buttons.
- No proportional font, no italic, no uppercase labels, no icon font.
- No second dark theme; dark exists only for the player.
- No `#ffffff`; the canvas is warm cream.
- No spinner PNG/GIF; spinners are ASCII frames.
- No color outside `{colors.signal}` and the status text.

## Responsive behaviour

| Width | Shell |
|---|---|
| 1280+ | rail 232px + content 960px + player bar |
| 1024 | rail 200px, content fluid |
| 768 | rail collapses to 56px icon-free rail (`lib`, `alb`, `rad`, `dwn`) |
| <600 | bottom tab strip + mini-player, no rail |

Status bar stays on every width — it is the system's voice.

## Known gaps

- Cover-derived `{colors.signal}` needs a runtime quantizer (median-cut → OKLCH clamp); fallback
  when art is missing: `#c2410c` (burnt orange) as the house signal.
- `{component.sound-map}` projection algorithm (UMAP vs PaCMAP) is undecided; the screen ships
  after embeddings land.
- Terminal glyph coverage differs across platforms; JetBrains Mono covers `█░▌▸·↻↓`, and every
  glyph in the icon set is verified against that font before use.
