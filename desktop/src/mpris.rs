//! MPRIS2 over D-Bus, so media keys, the GNOME/KDE media widget and `playerctl`
//! drive the player.
//!
//! Everything D-Bus lives on one dedicated thread with its own zbus connection,
//! so nothing here can block the UI. `set` only pushes a snapshot; that thread
//! owns the authoritative state and emits `PropertiesChanged`.
//!
//! Control callbacks run on the MPRIS thread: keep them cheap, send to the UI.
//! `on_seek` gets an **absolute** position in seconds. MPRIS has both `Seek`
//! (a relative offset) and `SetPosition` (absolute); both become an absolute
//! target here, clamped to the track.

use std::{
    sync::{Arc, Mutex, MutexGuard, mpsc},
    thread,
    time::Instant,
};

use anyhow::{Result, anyhow};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Property, RootInterface,
    Server, Time, TrackId, Volume,
    zbus::{self, fdo},
};

use crate::api::Track;

const TRACK_PATH: &str = "/org/bulba/music/track/";

pub struct Mpris {
    tx: async_channel::Sender<Snapshot>,
}

/// What the app pushes on `set`; plain owned data, no D-Bus types on the UI side.
struct Snapshot {
    track: Option<Track>,
    playing: bool,
    position: f64,
    volume: f64,
}

struct State {
    track: Option<Track>,
    playing: bool,
    position: f64,
    /// When `position` was taken; the rest is interpolated from the clock.
    at: Instant,
    volume: f64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            track: None,
            playing: false,
            position: 0.0,
            at: Instant::now(),
            volume: 1.0,
        }
    }
}

impl State {
    fn apply(&mut self, s: Snapshot) {
        self.track = s.track;
        self.playing = s.playing;
        self.position = s.position.max(0.0);
        self.volume = s.volume.clamp(0.0, 1.0);
        self.at = Instant::now();
    }

    fn position(&self) -> f64 {
        if self.playing {
            self.position + self.at.elapsed().as_secs_f64()
        } else {
            self.position
        }
    }

    fn status(&self) -> PlaybackStatus {
        if self.track.is_none() {
            PlaybackStatus::Stopped
        } else if self.playing {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Paused
        }
    }

    fn metadata(&self) -> Metadata {
        let Some(t) = &self.track else {
            return Metadata::new();
        };
        let mut b = Metadata::builder()
            .trackid(track_id(t.id))
            .title(t.title.clone());
        if !t.artist.is_empty() {
            b = b.artist([t.artist.clone()]);
        }
        if !t.album.is_empty() {
            b = b.album(t.album.clone());
        }
        if t.duration_ms > 0 {
            b = b.length(Time::from_millis(t.duration_ms));
        }
        b.build()
    }
}

fn track_id(id: i64) -> TrackId {
    TrackId::try_from(format!("{TRACK_PATH}{id}").as_str()).unwrap_or(TrackId::NO_TRACK)
}

type Action = Box<dyn Fn() + Send>;

struct Callbacks {
    play: Action,
    pause: Action,
    next: Action,
    prev: Action,
    seek: Box<dyn Fn(f64) + Send>,
    volume: Box<dyn Fn(f64) + Send>,
}

struct Backend {
    state: Arc<Mutex<State>>,
    cbs: Mutex<Callbacks>,
}

/// A callback panicking must not brick the rest of the widget forever.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Backend {
    fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    fn cbs(&self) -> MutexGuard<'_, Callbacks> {
        lock(&self.cbs)
    }

    fn seek_to(&self, target: f64) {
        let mut st = self.state();
        let end = st
            .track
            .as_ref()
            .filter(|t| t.duration_ms > 0)
            .map_or(f64::INFINITY, |t| t.duration().as_secs_f64());
        let target = target.clamp(0.0, end);
        st.position = target;
        st.at = Instant::now();
        drop(st);
        (self.cbs().seek)(target);
    }
}

impl Mpris {
    /// Start serving `org.mpris.MediaPlayer2.bulba` on the session bus.
    ///
    /// `on_seek` gets an **absolute** position in seconds; the relative MPRIS
    /// `Seek` offset is resolved against the current position first.
    ///
    /// Fails if there is no session bus; the caller can ignore that.
    pub fn start(
        on_play: impl Fn() + Send + 'static,
        on_pause: impl Fn() + Send + 'static,
        on_next: impl Fn() + Send + 'static,
        on_prev: impl Fn() + Send + 'static,
        on_seek: impl Fn(f64) + Send + 'static,
        on_volume: impl Fn(f64) + Send + 'static,
    ) -> Result<Self> {
        let state = Arc::new(Mutex::new(State::default()));
        let (tx, rx) = async_channel::unbounded::<Snapshot>();
        let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(), String>>();

        thread::Builder::new()
            .name("mpris".into())
            .spawn(move || {
                let backend = Backend {
                    state: Arc::clone(&state),
                    cbs: Mutex::new(Callbacks {
                        play: Box::new(on_play),
                        pause: Box::new(on_pause),
                        next: Box::new(on_next),
                        prev: Box::new(on_prev),
                        seek: Box::new(on_seek),
                        volume: Box::new(on_volume),
                    }),
                };
                zbus::block_on(async move {
                    let server = match Server::new("bulba", backend).await {
                        Ok(server) => server,
                        Err(e) => {
                            let _ = ready_tx.send(Err(e.to_string()));
                            return;
                        }
                    };
                    let _ = ready_tx.send(Ok(()));

                    while let Ok(snapshot) = rx.recv().await {
                        let changes = {
                            let mut st = lock(&state);
                            st.apply(snapshot);
                            [
                                Property::Metadata(st.metadata()),
                                Property::PlaybackStatus(st.status()),
                                Property::Volume(st.volume),
                            ]
                        };
                        if let Err(e) = server.properties_changed(changes).await {
                            eprintln!("mpris: PropertiesChanged: {e}");
                        }
                    }
                });
            })?;

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self { tx }),
            Ok(Err(e)) => Err(anyhow!("MPRIS unavailable: {e}")),
            Err(_) => Err(anyhow!("MPRIS thread stopped before serving")),
        }
    }

    /// Push the current state. Cheap enough for every track change and
    /// play/pause; position is interpolated by clients from `Position` and
    /// `PlaybackStatus`, so do not call this for progress.
    pub fn set(&self, track: Option<&Track>, playing: bool, position_secs: f64, volume: f64) {
        let _ = self.tx.try_send(Snapshot {
            track: track.cloned(),
            playing,
            position: position_secs,
            volume,
        });
    }
}

impl RootInterface for Backend {
    async fn raise(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("no window to raise".into()))
    }

    async fn quit(&self) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("the ui owns the process".into()))
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("bulba music".into())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("bulba-music".into())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![])
    }
}

impl PlayerInterface for Backend {
    async fn next(&self) -> fdo::Result<()> {
        let cbs = self.cbs();
        (cbs.next)();
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let cbs = self.cbs();
        (cbs.prev)();
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        let cbs = self.cbs();
        (cbs.pause)();
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        let playing = self.state().playing;
        let cbs = self.cbs();
        if playing {
            (cbs.pause)();
        } else {
            (cbs.play)();
        }
        Ok(())
    }

    /// No stop in the app; pausing is the closest honest thing.
    async fn stop(&self) -> fdo::Result<()> {
        let cbs = self.cbs();
        (cbs.pause)();
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        let cbs = self.cbs();
        (cbs.play)();
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let now = self.state().position();
        self.seek_to(now + offset.as_micros() as f64 / 1e6);
        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> fdo::Result<()> {
        {
            let st = self.state();
            if let Some(t) = &st.track
                && track_id.to_string() != format!("{TRACK_PATH}{}", t.id)
            {
                return Ok(()); // stale: the track changed under the client
            }
        }
        self.seek_to(position.as_micros() as f64 / 1e6);
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(fdo::Error::NotSupported("no uri handler".into()))
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(self.state().status())
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.state().metadata())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.state().volume)
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        self.state().volume = volume;
        (self.cbs().volume)(volume);
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(
            (self.state().position() * 1e6) as i64,
        ))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
