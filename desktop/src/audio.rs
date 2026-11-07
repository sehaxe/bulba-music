//! Playback: two decks on one mixer, so a track can end while the next is
//! already audible. The blend length is chosen from the two Camelot codes, the
//! same rule the phone uses — a mix that only ever fades for the same number of
//! seconds is a crossfade, not a DJ.
//!
//! Files are fetched once into a cache directory and decoded from disk. The
//! server is a LAN machine; a 30 MB flac arrives faster than the first second of
//! it plays, and from disk every seek is instant.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rodio::{DeviceSinkBuilder, Player};

use crate::api::{Api, Track};

pub const PLAY_START: i64 = 0;
pub const PLAY_SKIP: i64 = 2;
pub const PLAY_END: i64 = 3;

pub const SOURCE_QUEUE: i64 = 5;

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub track: Option<Track>,
    pub position: Duration,
    pub length: Duration,
    pub playing: bool,
    pub loading: bool,
    pub volume: f32,
    pub queue_len: usize,
    pub index: usize,
    pub error: Option<String>,
}

#[derive(Debug)]
enum Cmd {
    Play(Vec<Track>, usize),
    Toggle,
    SetPlaying(bool),
    Next,
    Prev,
    Seek(Duration),
    Volume(f32),
    Append(Vec<Track>),
    Stop,
}

#[derive(Clone)]
pub struct Audio {
    tx: Sender<Cmd>,
    snap: Arc<Mutex<Snapshot>>,
}

impl Audio {
    pub fn start(api: Api) -> Self {
        let (tx, rx) = channel();
        let snap = Arc::new(Mutex::new(Snapshot {
            volume: 0.85,
            ..Default::default()
        }));
        let cache = Cache::new(api.clone());
        let thread_snap = snap.clone();
        std::thread::Builder::new()
            .name("bulba-audio".into())
            .spawn(move || {
                if let Err(e) = run(rx, thread_snap.clone(), api, cache) {
                    if let Ok(mut s) = thread_snap.lock() {
                        s.error = Some(e.to_string());
                        s.loading = false;
                    }
                }
            })
            .expect("audio thread");
        Self { tx, snap }
    }

    pub fn play(&self, queue: Vec<Track>, index: usize) {
        let _ = self.tx.send(Cmd::Play(queue, index));
    }

    pub fn append(&self, tracks: Vec<Track>) {
        let _ = self.tx.send(Cmd::Append(tracks));
    }

    pub fn toggle(&self) {
        let _ = self.tx.send(Cmd::Toggle);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(Cmd::SetPlaying(true));
    }

    pub fn pause(&self) {
        let _ = self.tx.send(Cmd::SetPlaying(false));
    }

    pub fn next(&self) {
        let _ = self.tx.send(Cmd::Next);
    }

    pub fn prev(&self) {
        let _ = self.tx.send(Cmd::Prev);
    }

    pub fn seek(&self, to: Duration) {
        let _ = self.tx.send(Cmd::Seek(to));
    }

    pub fn set_volume(&self, v: f32) {
        let _ = self.tx.send(Cmd::Volume(v));
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

struct Cache {
    dir: PathBuf,
    api: Api,
    locks: Mutex<std::collections::HashMap<i64, Arc<Mutex<()>>>>,
}

impl Cache {
    fn new(api: Api) -> Self {
        let base = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache")
            });
        let dir = base.join("bulba-music").join("tracks");
        let _ = fs::create_dir_all(&dir);
        Self {
            dir,
            api,
            locks: Mutex::new(Default::default()),
        }
    }

    fn path(&self, t: &Track) -> PathBuf {
        let ext = t.codec.clone().unwrap_or_else(|| "flac".into());
        self.dir.join(format!("{}-{}.{}", t.id, t.rev, ext))
    }

    fn ensure(&self, t: &Track) -> anyhow::Result<PathBuf> {
        let path = self.path(t);
        if path.exists() {
            return Ok(path);
        }
        let lock = {
            let mut locks = self.locks.lock().unwrap();
            locks
                .entry(t.id)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().unwrap();
        if path.exists() {
            return Ok(path);
        }
        // The master, not the server's streaming policy: this is a desktop on a
        // LAN, there is no bandwidth to save and lossless is what the library is.
        let bytes = self
            .api
            .get_bytes(&format!("/api/tracks/{}/stream?quality=flac", t.id))?;
        let tmp = path.with_extension("part");
        fs::write(&tmp, &bytes)?;
        fs::rename(&tmp, &path)?;
        Ok(path)
    }
}

fn prefetch(cache: &Arc<Cache>, t: &Track) {
    let cache = cache.clone();
    let t = t.clone();
    std::thread::spawn(move || {
        let _ = cache.ensure(&t);
    });
}

struct Deck {
    player: Player,
}

impl Deck {
    /// rodio's own position is already absolute: `try_seek` moves it. Adding an
    /// offset here doubled every seek and made the track look finished.
    fn position(&self) -> Duration {
        self.player.get_pos()
    }
}

/// How much room the two keys give a blend. Same code: the whole length.
/// Relative major/minor and the wheel's neighbours: most of it. A clash: a
/// short kiss, just enough not to cut.
fn harmonic_factor(a: Option<&str>, b: Option<&str>) -> f32 {
    let (Some(a), Some(b)) = (a, b) else {
        return 0.66;
    };
    if a == b {
        return 1.0;
    }
    let split = |code: &str| -> Option<(i32, char)> {
        let mut chars = code.chars();
        let letter = chars.next_back()?;
        let number = chars.as_str().parse::<i32>().ok()?;
        Some((number, letter))
    };
    let (Some((an, al)), Some((bn, bl))) = (split(a), split(b)) else {
        return 0.66;
    };
    let d = (an - bn).abs();
    let dist = d.min(12 - d);
    match (al == bl, dist) {
        (false, 0) => 0.8,  // relative major/minor, same slot
        (true, 1) => 0.66,  // one step around the wheel
        (true, 7) => 0.6,   // perfect fifth
        (true, 2) => 0.4,
        _ => 0.3,
    }
}

enum Cursor {
    Idle,
    Played,
}

fn run(rx: Receiver<Cmd>, snap: Arc<Mutex<Snapshot>>, api: Api, cache: Cache) -> anyhow::Result<()> {
    let cache = Arc::new(cache);
    let sink = DeviceSinkBuilder::open_default_sink()
        .map_err(|e| anyhow::anyhow!("нет аудиовыхода: {e}"))?;
    let verbose = std::env::var("BULBA_AUDIO_LOG").is_ok();
    if verbose {
        eprintln!("[audio] sink open");
    }
    let mixer = sink.mixer();

    let mut queue: Vec<Track> = Vec::new();
    let mut index = 0usize;
    let mut decks: [Option<Deck>; 2] = [None, None];
    let mut active = 0usize;
    let mut playing = true;
    let mut volume = 0.85f32;
    let mut blend: Option<f32> = None;
    let mut fade = Duration::from_secs(8);
    let mut loading = false;
    let mut error: Option<String> = None;
    let session = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut last_tick = Instant::now();
    let mut cursor = Cursor::Idle;

    let stop_both = |decks: &mut [Option<Deck>; 2]| {
        for slot in decks.iter_mut() {
            if let Some(deck) = slot.take() {
                deck.player.stop();
            }
        }
    };

    let load = |track: &Track, from: Duration| -> anyhow::Result<Deck> {
        let path = cache.ensure(track)?;
        let source = crate::track_source::open(&path, track.duration())?;
        let player = Player::connect_new(mixer);
        player.append(source);
        player.set_volume(0.0);
        if from > Duration::ZERO {
            let _ = player.try_seek(from);
        }
        Ok(Deck { player })
    };

    loop {
        while let Ok(cmd) = rx.try_recv() {
            if verbose {
                eprintln!("[audio] cmd {cmd:?}");
            }
            match cmd {
                Cmd::Play(q, i) => {
                    queue = q;
                    index = i.min(queue.len().saturating_sub(1));
                    blend = None;
                    playing = true;
                    cursor = Cursor::Idle;
                    stop_both(&mut decks);
                    if let Some(track) = queue.get(index).cloned() {
                        loading = true;
                        error = None;
                        match load(&track, Duration::ZERO) {
                            Ok(deck) => decks[active] = Some(deck),
                            Err(e) => error = Some(e.to_string()),
                        }
                        loading = false;
                        if let Some(next) = queue.get(index + 1) {
                            prefetch(&cache, next);
                        }
                    }
                }
                Cmd::Append(mut more) => {
                    let was_empty = queue.is_empty();
                    queue.append(&mut more);
                    if was_empty {
                        index = 0;
                        playing = true;
                        cursor = Cursor::Idle;
                        if let Some(track) = queue.first().cloned() {
                            if let Ok(deck) = load(&track, Duration::ZERO) {
                                decks[active] = Some(deck);
                            }
                        }
                    }
                }
                Cmd::Toggle => {
                    if decks[active].is_none() {
                        // The last track finished and took its deck with it; the
                        // play button must start it again rather than do nothing.
                        if let Some(track) = queue.get(index).cloned()
                            && let Ok(deck) = load(&track, Duration::ZERO)
                        {
                            decks[active] = Some(deck);
                            playing = true;
                            cursor = Cursor::Idle;
                        }
                    } else {
                        for deck in decks.iter().flatten() {
                            if playing {
                                deck.player.pause();
                            } else {
                                deck.player.play();
                            }
                        }
                        playing = !playing;
                    }
                }
                Cmd::SetPlaying(on) => {
                    if decks[active].is_none() && on {
                        if let Some(track) = queue.get(index).cloned()
                            && let Ok(deck) = load(&track, Duration::ZERO)
                        {
                            decks[active] = Some(deck);
                            cursor = Cursor::Idle;
                        }
                    }
                    for deck in decks.iter().flatten() {
                        if on {
                            deck.player.play();
                        } else {
                            deck.player.pause();
                        }
                    }
                    playing = on;
                }
                Cmd::Next => {
                    if index + 1 < queue.len() {
                        let pos = decks[active]
                            .as_ref()
                            .map(|d| d.position())
                            .unwrap_or_default();
                        if let Some(t) = queue.get(index).cloned() {
                            signal(&api, &t, PLAY_SKIP, pos, session);
                        }
                        index += 1;
                        blend = None;
                        cursor = Cursor::Idle;
                        stop_both(&mut decks);
                        if let Some(track) = queue.get(index).cloned() {
                            if let Ok(deck) = load(&track, Duration::ZERO) {
                                if !playing {
                                    deck.player.pause();
                                }
                                decks[active] = Some(deck);
                            }
                        }
                    }
                }
                Cmd::Prev => {
                    index = index.saturating_sub(1);
                    blend = None;
                    cursor = Cursor::Idle;
                    stop_both(&mut decks);
                    if let Some(track) = queue.get(index).cloned() {
                        if let Ok(deck) = load(&track, Duration::ZERO) {
                            if !playing {
                                deck.player.pause();
                            }
                            decks[active] = Some(deck);
                        }
                    }
                }
                Cmd::Seek(to) => {
                    if let Some(deck) = decks[active].as_mut() {
                        let _ = deck.player.try_seek(to);
                    }
                }
                Cmd::Volume(v) => volume = v.clamp(0.0, 1.0),
                Cmd::Stop => {
                    stop_both(&mut decks);
                    playing = false;
                    queue.clear();
                    index = 0;
                }
            }
        }

        let dt = last_tick.elapsed();
        last_tick = Instant::now();

        let current = queue.get(index).cloned();
        let (mut position, mut remaining, mut audible) = (Duration::ZERO, Duration::ZERO, false);
        if let (Some(deck), Some(t)) = (decks[active].as_ref(), current.as_ref()) {
            position = deck.position();
            remaining = t.duration().saturating_sub(position);
            audible = !deck.player.is_paused();
        }

        if audible && matches!(cursor, Cursor::Idle) {
            if let Some(t) = current.as_ref() {
                signal(&api, t, PLAY_START, position, session);
                cursor = Cursor::Played;
            }
        }

        if blend.is_none() && audible && remaining <= fade && index + 1 < queue.len() {
            let (Some(t), Some(next)) = (current.as_ref(), queue.get(index + 1).cloned()) else {
                unreachable!()
            };
            fade = Duration::from_secs_f32(8.0 * harmonic_factor(
                t.camelot.as_deref(),
                next.camelot.as_deref(),
            ));
            let other = 1 - active;
            prefetch(&cache, &next);
            if let Ok(deck) = load(&next, Duration::ZERO) {
                if let Some(old) = decks[other].take() {
                    old.player.stop();
                }
                if !playing {
                    deck.player.pause();
                }
                decks[other] = Some(deck);
                blend = Some(0.0);
            }
        }

        if let Some(mut p) = blend {
            let other = 1 - active;
            p = (p + dt.as_secs_f32() / fade.as_secs_f32().max(0.1)).min(1.0);
            blend = Some(p);
            let angle = p * std::f32::consts::FRAC_PI_2;
            if let Some(a) = decks[active].as_ref() {
                a.player.set_volume(volume * angle.cos());
            }
            if let Some(b) = decks[other].as_ref() {
                b.player.set_volume(volume * angle.sin());
            }
            if p >= 1.0 {
                if let Some(t) = current.as_ref() {
                    signal(&api, t, PLAY_END, t.duration(), session);
                }
                if let Some(a) = decks[active].take() {
                    a.player.stop();
                }
                active = other;
                index += 1;
                cursor = Cursor::Idle;
                blend = None;
                if let Some(next) = queue.get(index + 1) {
                    prefetch(&cache, next);
                }
            }
        } else if decks[active].is_some() && remaining.is_zero() && audible {
            if let Some(t) = current.as_ref() {
                signal(&api, t, PLAY_END, t.duration(), session);
            }
            if let Some(a) = decks[active].take() {
                a.player.stop();
            }
            if index + 1 < queue.len() {
                index += 1;
                cursor = Cursor::Idle;
                if let Some(track) = queue.get(index).cloned() {
                    if let Ok(deck) = load(&track, Duration::ZERO) {
                        decks[active] = Some(deck);
                    }
                }
            } else {
                playing = false;
                cursor = Cursor::Idle;
            }
        } else if blend.is_none() {
            if let Some(deck) = decks[active].as_ref() {
                deck.player.set_volume(volume);
            }
        }

        if verbose && last_tick.elapsed() >= Duration::ZERO {
            static mut N: u64 = 0;
            unsafe {
                N += 1;
                if N % 50 == 0 {
                    eprintln!(
                        "[audio] pos={:?} remaining={:?} playing={} blend={:?} deck={} err={:?}",
                        position, remaining, playing, blend, decks[active].is_some(), error
                    );
                }
            }
        }

        if let Ok(mut s) = snap.lock() {
            s.track = current;
            s.position = position;
            s.length = queue.get(index).map(|t| t.duration()).unwrap_or_default();
            s.playing = playing && decks[active].is_some();
            s.loading = loading;
            s.volume = volume;
            s.queue_len = queue.len();
            s.index = index;
            s.error = error.clone();
        }

        std::thread::sleep(Duration::from_millis(20));
    }
}

fn signal(api: &Api, track: &Track, event: i64, position: Duration, session: i64) {
    let api = api.clone();
    let track = track.clone();
    std::thread::spawn(move || {
        let ratio = if track.duration_ms <= 0 {
            0.0
        } else {
            (position.as_millis() as f64 / track.duration_ms as f64).clamp(0.0, 1.0)
        };
        let ev = serde_json::json!([{
            "track_id": track.id,
            "session_id": session,
            "event": event,
            "position_ms": position.as_millis() as i64,
            "position_ratio": ratio,
            "source": SOURCE_QUEUE,
            "prev_track_id": null,
            "device": 3,
            "offline": false,
            "quality": if track.codec.as_deref() == Some("flac") { 0 } else { 1 },
            "client_id": "gpui-desktop",
        }]);
        // Every call inserts a row: retrying a success would triple the play
        // count, the skip rate and the taste model.
        for attempt in 0..3 {
            let sent: anyhow::Result<serde_json::Value> = api.post("/api/events", &ev);
            if sent.is_ok() {
                break;
            }
            if attempt < 2 {
                std::thread::sleep(Duration::from_millis(800));
            }
        }
    });
}
