//! The window: a rail, a room made of the current cover, and one player bar.

use std::cell::Cell;
use std::collections::HashSet;
use std::sync::Arc;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, Bounds, BoxShadow, Context, Entity, FontWeight,
    KeyDownEvent, MouseButton, ObjectFit, Render, SharedString, Transformation, Window, canvas,
    div, ease_in_out, img, point, prelude::*, px, quad, svg, transparent_black,
};

use crate::api::{AlbumPage, Api, Track};
use crate::audio::{self, Audio};
use crate::covers::Covers;
use crate::fx;
use crate::theme::{FONT, Palette};
use crate::mpris::Mpris;
use crate::search;
use crate::wave::{self, Wave};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Home,
    Wave,
    Search,
    Library,
    Liked,
    Settings,
}

const TAB_ICONS: [(Tab, &str, &str); 6] = [
    (Tab::Home, "home", "главная"),
    (Tab::Wave, "wave", "волна"),
    (Tab::Search, "search", "поиск"),
    (Tab::Library, "library", "библиотека"),
    (Tab::Liked, "heart", "любимое"),
    (Tab::Settings, "settings", "настройки"),
];

pub struct Root {
    api: Api,
    audio: Audio,
    covers: Covers,
    palette: Palette,
    tab: Tab,
    album: Option<i64>,
    albums: Vec<crate::api::AlbumSummary>,
    tracks: Vec<Track>,
    liked: HashSet<i64>,
    page: Option<AlbumPage>,
    snap: audio::Snapshot,
    ready: bool,
    error: Option<String>,
    /// Which cover lights the room right now.
    room: Option<i64>,
    muted: Option<f32>,
    frame: u64,
    /// Review harness, same idea as the phone's BULBA_* variables: a screen
    /// that can only be reached by three clicks is a screen nobody looks at.
    boot_album: Option<i64>,
    boot_play: Option<i64>,
    wave: Wave,
    wave_loading: bool,
    wave_started: bool,
    nav_tx: std::sync::mpsc::Sender<i64>,
    nav_rx: std::sync::mpsc::Receiver<i64>,
    search: Entity<search::Search>,
    mpris: Option<Mpris>,
    grain: Option<Arc<gpui::RenderImage>>,
    vignette: Option<Arc<gpui::RenderImage>>,
    glow: Option<(i32, Arc<gpui::RenderImage>)>,
    /// Set every frame from the real viewport: the hero shows a track list only
    /// when there is room for it, instead of squeezing the title.
    wide: bool,
}

impl Root {
    pub fn new(api: Api, cx: &mut Context<Self>) -> Self {
        let audio = Audio::start(api.clone());
        let nav = std::sync::mpsc::channel();
        let search = cx.new(|cx| search::Search::new(cx));
        // Media keys and the desktop's own player widget. Failure is quiet: a
        // session without a bus still plays music.
        let mpris = {
            let (play, pause, next, prev) = (
                audio.clone(),
                audio.clone(),
                audio.clone(),
                audio.clone(),
            );
            let (seek, volume) = (audio.clone(), audio.clone());
            Mpris::start(
                move || play.resume(),
                move || pause.pause(),
                move || next.next(),
                move || prev.prev(),
                move |secs| seek.seek(std::time::Duration::from_secs_f64(secs.max(0.0))),
                move |v| volume.set_volume(v as f32),
            )
            .ok()
        };
        let mut this = Self {
            api,
            audio,
            covers: Covers::new(),
            palette: Palette::default(),
            tab: Tab::Home,
            album: None,
            albums: Vec::new(),
            tracks: Vec::new(),
            liked: HashSet::new(),
            page: None,
            snap: audio::Snapshot::default(),
            ready: false,
            error: None,
            room: None,
            muted: None,
            frame: 0,
            boot_album: std::env::var("BULBA_ALBUM").ok().and_then(|v| v.parse().ok()),
            boot_play: std::env::var("BULBA_PLAY").ok().and_then(|v| v.parse().ok()),
            wave: Wave::new(),
            wave_loading: false,
            wave_started: false,
            nav_tx: nav.0,
            nav_rx: nav.1,
            search,
            mpris,
            grain: None,
            vignette: None,
            glow: None,
            wide: true,
        };
        // Results arrive on the Search entity; the shell has to repaint when
        // they do, or the field shows a new query over an old answer.
        let watched = this.search.clone();
        cx.observe(&watched, |_, _, cx| cx.notify()).detach();
        if let Ok(tab) = std::env::var("BULBA_TAB") {
            this.tab = match tab.as_str() {
                "library" => Tab::Library,
                "liked" => Tab::Liked,
                "wave" => Tab::Wave,
                "search" => Tab::Search,
                "settings" => Tab::Settings,
                _ => Tab::Home,
            };
        }
        this.album = this.boot_album;
        if this.tab == Tab::Wave {
            this.ensure_wave(cx);
        }
        let seed = Palette::default();
        this.glow = Some((
            (seed.accent.h * 360.0).round() as i32,
            fx::glow(512, seed.accent.h * 360.0, 0.4, 0.55),
        ));
        this.vignette = Some(fx::vignette(512, 0.62));
        this.grain = Some(fx::grain(1024, 0.09, 0x5eed));
        this.load(cx);
        this.ticker(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| {
            let (albums, tracks, likes) = {
                let api = api.clone();
                cx.background_executor()
                    .spawn(async move {
                        (
                            api.albums(),
                            api.tracks(),
                            api.like_ids().unwrap_or_default(),
                        )
                    })
                    .await
            };
            this.update(cx, |this, cx| {
                match albums {
                    Ok(list) => this.albums = list,
                    Err(e) => this.error = Some(format!("альбомы: {e}")),
                }
                match tracks {
                    Ok(list) => this.tracks = list,
                    Err(e) => this.error = Some(format!("треки: {e}")),
                }
                this.liked = likes.into_iter().collect();
                this.ready = !this.albums.is_empty();
                this.room = this.snap.track.as_ref().map(|t| t.album_id).or_else(|| {
                    this.albums
                        .iter()
                        .max_by_key(|a| a.plays)
                        .map(|a| a.id)
                });
                if let Some(id) = this.boot_album {
                    this.room = Some(id);
                    this.ensure_art(id, cx);
                }
                if let Ok(q) = std::env::var("BULBA_SEARCH")
                    && !q.trim().is_empty()
                {
                    let api = this.api.clone();
                    let search = this.search.clone();
                    let _ = search.update(cx, |s, cx| {
                        s.query = q;
                        s.run(&api, cx);
                    });
                }
                if let Some(track_id) = this.boot_play
                    && let Some(track) = this.tracks.iter().find(|t| t.id == track_id).cloned()
                {
                    let list = this.album_tracks(track.album_id);
                    let index = list.iter().position(|t| t.id == track_id).unwrap_or(0);
                    this.audio.play(list, index);
                }
                cx.notify();
                this.load_art(cx);
            })
            .ok();
        })
        .detach();
    }

    /// One background pass over every album cover, notifying as each arrives.
    /// 49 covers at 512 px is ~25 MB — small enough to keep them all; a library
    /// of thousands will need a viewport window instead.
    fn load_art(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let wanted: Vec<(i64, String)> = self
            .albums
            .iter()
            .map(|a| (a.id, a.title.clone()))
            .collect();
        // Four lanes, not one queue: the server encodes each JXL on first
        // request, and a strictly sequential pass left half the grid grey for
        // ten seconds on a 49-album library.
        for lane in 0..4usize {
            let api = api.clone();
            let wanted: Vec<(i64, String)> = wanted
                .iter()
                .skip(lane)
                .step_by(4)
                .cloned()
                .collect();
            cx.spawn(async move |this, cx| {
                for (id, title) in wanted {
                    let begin = this
                        .update(cx, |this, _| this.covers.begin(id))
                        .unwrap_or(false);
                    if !begin {
                        continue;
                    }
                    let api = api.clone();
                    let art = cx
                        .background_executor()
                        .spawn(async move { crate::covers::load(&api, id, &title) })
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        this.covers.finish(id, art);
                        this.refit_room(id);
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }

    fn ticker(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let alive = this.update(cx, |this, cx| {
                    this.frame += 1;
                    this.drain_nav(cx);
                    let snap = this.audio.snapshot();
                    let track_changed =
                        snap.track.as_ref().map(|t| t.id) != this.snap.track.as_ref().map(|t| t.id);
                    let play_changed = snap.playing != this.snap.playing;
                    let volume_changed = (snap.volume - this.snap.volume).abs() > 0.001;
                    // The ticker itself runs every 100 ms, so a 120 ms threshold
                    // could never fire: the bar would freeze until something else
                    // repainted it.
                    let moved = snap.position.abs_diff(this.snap.position)
                        > Duration::from_millis(45);
                    this.snap = snap;
                    let changed = track_changed || play_changed || volume_changed || moved;
                    // MPRIS interpolates position itself; pushing ten times a
                    // second only makes the bus shout.
                    if track_changed || play_changed || volume_changed {
                        if let (Some(mpris), snapshot) = (&this.mpris, &this.snap) {
                            mpris.set(
                                snapshot.track.as_ref(),
                                snapshot.playing,
                                snapshot.position.as_secs_f64(),
                                snapshot.volume as f64,
                            );
                        }
                    }
                    if changed {
                        cx.notify();
                    }
                    if let Some(track) = this.snap.track.clone() {
                        if this.room != Some(track.album_id) {
                            this.room = Some(track.album_id);
                            this.ensure_art(track.album_id, cx);
                            cx.notify();
                        }
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn ensure_art(&mut self, album_id: i64, cx: &mut Context<Self>) {
        // Cached art still has to re-tint the room: `begin` refuses an album it
        // already has, which used to leave the palette from the previous record.
        if self.covers.get(album_id).is_some() {
            self.refit_room(album_id);
            return;
        }
        if !self.covers.begin(album_id) {
            return;
        }
        let api = self.api.clone();
        let title = self
            .albums
            .iter()
            .find(|a| a.id == album_id)
            .map(|a| a.title.clone())
            .unwrap_or_default();
        cx.spawn(async move |this, cx| {
            let art = cx
                .background_executor()
                .spawn(async move { crate::covers::load_cover(&api, album_id, &title, true) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.covers.finish(album_id, art);
                this.refit_room(album_id);
                cx.notify();
            });
        })
        .detach();
    }

    /// The room is lit by whichever cover is playing. Called both when art
    /// arrives lazily and when the playing track changes, because either can be
    /// the first to know.
    fn refit_room(&mut self, album_id: i64) {
        if self.room != Some(album_id) {
            return;
        }
        if let Some(art) = self.covers.get(album_id) {
            self.palette = Palette::from_art(art.hue, art.sat, art.lum);
            let hue = art.hue.round() as i32;
            self.glow = Some((hue, fx::glow(512, art.hue, art.sat.max(0.35), 0.55)));
        }
    }

    /// A baked radial light in the current accent, cached per hue: GPUI has no
    /// shaders, so the glow behind the record is a texture, not a blur.
    fn glow_for(&self, p: Palette) -> Option<Arc<gpui::RenderImage>> {
        let hue = (p.accent.h * 360.0).round() as i32;
        self.glow
            .as_ref()
            .filter(|(h, _)| *h == hue)
            .map(|(_, image)| image.clone())
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.album = None;
        if tab == Tab::Wave {
            self.ensure_wave(cx);
        }
        cx.notify();
    }

    /// The wave is fetched once per session and kept: coming back to the tab must
    /// show the same stream you left, not a new dice roll.
    fn ensure_wave(&mut self, cx: &mut Context<Self>) {
        if self.wave_started {
            return;
        }
        self.wave_started = true;
        self.wave_loading = true;
        let api = self.api.clone();
        let seed = self.room;
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move { Wave::load(&api, seed) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.wave_loading = false;
                match loaded {
                    Ok(w) => this.wave = w,
                    Err(e) => this.error = Some(format!("волна: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn append_wave(&mut self, cx: &mut Context<Self>) {
        if self.wave_loading {
            return;
        }
        self.wave_loading = true;
        let api = self.api.clone();
        let seed = self.wave.seed;
        let exclude: Vec<i64> = self.wave.tracks.iter().map(|t| t.track.id).collect();
        cx.spawn(async move |this, cx| {
            let reply = cx
                .background_executor()
                .spawn(async move { api.wave(seed, &exclude, 60) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.wave_loading = false;
                if let Ok(reply) = reply {
                    let mut seen: HashSet<i64> =
                        this.wave.tracks.iter().map(|t| t.track.id).collect();
                    this.wave
                        .tracks
                        .extend(reply.tracks.into_iter().filter(|t| seen.insert(t.track.id)));
                    this.wave.seed = reply.seed.or(this.wave.seed);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn drain_nav(&mut self, cx: &mut Context<Self>) {
        let mut opened = false;
        while let Ok(id) = self.nav_rx.try_recv() {
            self.album = Some(id);
            self.room = Some(id);
            self.ensure_art(id, cx);
            opened = true;
        }
        if opened {
            cx.notify();
        }
    }

    fn search_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let audio = self.audio.clone();
        let nav = self.nav_tx.clone();
        let search = self.search.read(cx);
        let queue: Vec<Track> = search
            .results
            .as_ref()
            .map(|r| r.tracks.clone())
            .unwrap_or_default();
        let play_queue = queue.clone();
        let on_play = move |id: i64| {
            let index = play_queue.iter().position(|t| t.id == id).unwrap_or(0);
            audio.play(play_queue.clone(), index);
        };
        let on_open = move |album_id: i64| {
            let _ = nav.send(album_id);
        };
        let focus = search.focus.clone();
        let focus_click = search.focus.clone();
        div()
            .id("search-panel")
            .pt(px(14.))
            .track_focus(&focus)
            .on_click(move |_, window, _| window.focus(&focus_click))
            .child(search::render(&search, p, on_play, on_open))
    }

    fn wave_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let audio = self.audio.clone();
        let nav = self.nav_tx.clone();
        let queue: Vec<Track> = self.wave.tracks.iter().map(|w| w.track.clone()).collect();
        let play_queue = queue.clone();
        let on_play = move |id: i64| {
            let index = play_queue.iter().position(|t| t.id == id).unwrap_or(0);
            audio.play(play_queue.clone(), index);
        };
        let on_open = move |album_id: i64| {
            let _ = nav.send(album_id);
        };
        let playing = self.snap.track.as_ref().map(|t| t.id);

        div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .pt(px(10.))
            .child(wave::render(
                &self.wave,
                p,
                playing,
                {
                    let covers = self.covers.cache.clone();
                    move |album_id| covers.get(&album_id).map(|a| a.cover.clone())
                },
                on_play,
                on_open,
            ))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .pt(px(10.))
                    .child(
                        div()
                            .id("wave-more")
                            .h(px(36.))
                            .px(px(18.))
                            .rounded(px(999.))
                            .bg(p.card)
                            .border_1()
                            .border_color(p.hairline)
                            .flex()
                            .items_center()
                            .hover(|el| el.bg(p.card_hover))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(if self.wave_loading { p.ash } else { p.mute })
                                    .child(if self.wave_loading {
                                        "подбираю…"
                                    } else {
                                        "ещё"
                                    }),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.append_wave(cx))),
                    ),
            )
    }

    fn open_album(&mut self, id: i64, cx: &mut Context<Self>) {
        self.album = Some(id);
        self.page = None;
        self.room = Some(id);
        self.ensure_art(id, cx);
        cx.notify();
    }

    fn album_tracks(&self, album_id: i64) -> Vec<Track> {
        let mut list: Vec<Track> = self
            .tracks
            .iter()
            .filter(|t| t.album_id == album_id)
            .cloned()
            .collect();
        list.sort_by_key(|t| (t.track_no.unwrap_or(0), t.id));
        list
    }

    fn play_album(&self, album_id: i64, from: usize) {
        let list = self.album_tracks(album_id);
        if list.is_empty() {
            return;
        }
        self.audio.play(list, from.min(999));
    }

    fn toggle_track(&mut self, track: &Track) {
        if self.snap.track.as_ref().map(|t| t.id) == Some(track.id) {
            self.audio.toggle();
        } else {
            let list = self.album_tracks(track.album_id);
            let index = list.iter().position(|t| t.id == track.id).unwrap_or(0);
            self.audio.play(list, index);
        }
    }

    fn toggle_like(&mut self, id: i64, cx: &mut Context<Self>) {
        let liked = !self.liked.contains(&id);
        if liked {
            self.liked.insert(id);
        } else {
            self.liked.remove(&id);
        }
        let api = self.api.clone();
        cx.background_executor()
            .spawn(async move {
                let _ = api.set_like(id, liked);
            })
            .detach();
        cx.notify();
    }

    fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        match self.muted.take() {
            Some(v) => self.audio.set_volume(v),
            None => {
                self.muted = Some(self.snap.volume);
                self.audio.set_volume(0.0);
            }
        }
        cx.notify();
    }

    // ---------------------------------------------------------------- visuals

    fn backdrop(&self) -> impl IntoElement {
        let p = self.palette;
        let art = self.room.and_then(|id| self.covers.get(id));
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(match art {
                Some(art) => div()
                    .absolute()
                    .inset_0()
                    .child(
                        img(art.backdrop.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .opacity(if p.light { 0.40 } else { 0.50 }),
                    )
                    .with_animation(
                        SharedString::from(format!("room-{}", art.hue as i32)),
                        Animation::new(Duration::from_millis(1200)).with_easing(ease_in_out),
                        |el, delta| el.opacity(delta),
                    )
                    .into_any_element(),
                None => div().into_any_element(),
            })
            .child(div().absolute().inset_0().bg(p.scrim()))
            .child(match &self.vignette {
                Some(v) => img(v.clone())
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .into_any_element(),
                None => div().into_any_element(),
            })
            .child(match &self.grain {
                Some(g) => img(g.clone())
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill)
                    .opacity(if p.light { 0.30 } else { 0.65 })
                    .into_any_element(),
                None => div().into_any_element(),
            })
    }

    fn rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let mut items = Vec::new();
        for (tab, icon_name, _label) in TAB_ICONS {
            let active = self.tab == tab;
            items.push(
                div()
                    .id(SharedString::from(format!("nav-{}", icon_name)))
                    .w_full()
                    .h(px(46.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(14.))
                    .when(active, |el| {
                        el.bg(p.card).border_1().border_color(p.hairline)
                    })
                    .hover(|el| el.bg(p.card_hover))
                    .child(
                        svg()
                            .path(asset(icon_name))
                            .size(px(19.))
                            .text_color(if active { p.accent } else { p.icon }),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx))),
            );
        }

        div()
            .w(px(70.))
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(16.))
            .pb(px(14.))
            .gap(px(6.))
            .child(
                div()
                    .size(px(28.))
                    .mb(px(10.))
                    .rounded(px(9.))
                    .bg(p.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.canvas)
                            .child("b"),
                    ),
            )
            .children(items)
            .child(div().flex_1())
    }

    fn titlebar(&self) -> impl IntoElement {
        let p = self.palette;
        div()
            .h(px(38.))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.mute)
                            .child("bulba music"),
                    )
                    .child(
                        div()
                            .size(px(3.))
                            .rounded(px(999.))
                            .bg(if self.error.is_some() { gpui::red() } else { p.accent }),
                    )
                    .child(
                        div()
                            .text_size(px(10.5))
                            .text_color(p.mute.opacity(0.9))
                            .child(
                                self.api
                                    .base
                                    .trim_start_matches("http://")
                                    .trim_start_matches("https://")
                                    .to_string(),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.mute.opacity(0.85))
                    .child(format!("{} треков · {} альбомов", self.tracks.len(), self.albums.len())),
            )
            .on_mouse_down(MouseButton::Left, |_, window, _| {
                window.start_window_move()
            })
    }

    fn content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let body = if let Some(id) = self.album {
            self.album_view(id, cx).into_any_element()
        } else {
            match self.tab {
                Tab::Home => self.home(cx).into_any_element(),
                Tab::Library => self.library(cx).into_any_element(),
                Tab::Liked => self.liked_view(cx).into_any_element(),
                Tab::Search => self.search_view(cx).into_any_element(),
                Tab::Wave => self.wave_view(cx).into_any_element(),
                Tab::Settings => self
                    .placeholder("настройки", "сервер, качество, звук")
                    .into_any_element(),
            }
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(self.titlebar())
            .child(
                div()
                    .id("scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(1240.))
                            .mx_auto()
                            .px(px(36.))
                            .pb(px(40.))
                            .child(body),
                    ),
            )
            .text_color(p.ink)
    }

    fn placeholder(&self, title: &str, subtitle: &str) -> impl IntoElement {
        let p = self.palette;
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(34.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.mute)
                    .child(title.to_string()),
            )
            .child(div().text_size(px(13.)).text_color(p.ash).child(subtitle.to_string()))
    }

    fn heading(&self, title: &str, subtitle: Option<String>) -> impl IntoElement {
        let p = self.palette;
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(30.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.ink)
                    .child(title.to_string()),
            )
            .child(match subtitle {
                Some(s) => div().text_size(px(12.)).text_color(p.ash).child(s),
                None => div().text_size(px(1.)),
            })
    }

    fn home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let hour = chrono_hour();
        let greeting = match hour {
            5..=11 => "доброе утро",
            12..=17 => "добрый день",
            18..=22 => "добрый вечер",
            _ => "ещё не спишь",
        };

        let hero_album = self.room.or_else(|| {
            self.albums
                .iter()
                .max_by_key(|a| a.plays)
                .map(|a| a.id)
        });

        let mut column = div().flex().flex_col().gap(px(34.)).pt(px(18.));
        column = column.child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(27.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.ink)
                                .child(greeting),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(p.ash)
                                .child("тихо, без спешки".to_string()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.ash)
                        .child(format!("{} треков · {} альбомов", self.tracks.len(), self.albums.len())),
                ),
        );

        if let Some(id) = hero_album {
            column = column.child(self.hero(id, cx));
        }

        column = column.child(self.section_title("часто слушаешь", None));
        let mut grid = div().flex().flex_wrap().gap(px(20.));
        let mut recent: Vec<_> = self.albums.iter().collect();
        recent.sort_by_key(|a| -a.plays);
        for album in recent.into_iter().take(8) {
            grid = grid.child(self.album_tile(album.id, &album.title, &album.artist, cx));
        }
        column = column.child(grid);
        column
    }

    fn hero(&self, album_id: i64, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let art = self.covers.get(album_id);
        let album = self.albums.iter().find(|a| a.id == album_id);
        let tracks = self.album_tracks(album_id);
        let title = album.map(|a| a.title.clone()).unwrap_or_default();
        let artist = album.map(|a| a.artist.clone()).unwrap_or_default();
        let year = album.and_then(|a| a.year);
        let count = tracks.len();
        let minutes = tracks.iter().map(|t| t.duration_ms).sum::<i64>() / 60000;
        let playing_here = self
            .snap
            .track
            .as_ref()
            .map(|t| t.album_id == album_id)
            .unwrap_or(false);
        let is_playing = playing_here && self.snap.playing;

        div()
            .id("hero")
            .relative()
            .w_full()
            .h(px(350.))
            .rounded(px(26.))
            .overflow_hidden()
            .border_1()
            .border_color(p.hairline)
            .bg(p.card)
            .child(match &art {
                Some(art) => div()
                    .absolute()
                    .inset_0()
                    .child(
                        img(art.backdrop.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .opacity(0.75),
                    )
                    .into_any_element(),
                None => div().into_any_element(),
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(gpui::linear_gradient(
                        70.0,
                        gpui::linear_color_stop(p.canvas.opacity(0.05), 0.0),
                        gpui::linear_color_stop(p.canvas.opacity(if p.light { 0.80 } else { 0.92 }), 1.0),
                    )),
            )
            .child(match self.glow_for(p) {
                Some(g) => div()
                    .absolute()
                    .left(px(-60.))
                    .bottom(px(-120.))
                    .size(px(520.))
                    .child(img(g).size_full().object_fit(ObjectFit::Contain))
                    .into_any_element(),
                None => div().into_any_element(),
            })
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .items_end()
                    .gap(px(30.))
                    .p(px(30.))
                    .child(
                        div()
                            .w(px(232.))
                            .h(px(232.))
                            .rounded(px(16.))
                            .overflow_hidden()
                            .flex_shrink_0()
                            .shadow(vec![BoxShadow {
                                color: p.shadow,
                                offset: point(px(0.), px(28.)),
                                blur_radius: px(60.),
                                spread_radius: px(-10.),
                            }])
                            .child(match art {
                                Some(art) => img(art.cover.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .into_any_element(),
                                None => div()
                                    .size_full()
                                    .bg(p.card_hover)
                                    .into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .pb(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(p.accent)
                                            .child(if playing_here {
                                                "сейчас играет"
                                            } else {
                                                "альбом"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(p.ash)
                                            .child(match self.snap.track.as_ref() {
                                                Some(t) if playing_here => {
                                                    format!("· {}", t.title)
                                                }
                                                _ => String::new(),
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(46.))
                                    .line_height(px(52.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.ink)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(9.))
                                    .text_size(px(13.))
                                    .text_color(p.mute)
                                    .child(artist)
                                    .child(div().text_color(p.ash.opacity(0.5)).child("·"))
                                    .child(match year {
                                        Some(y) => y.to_string(),
                                        None => "—".into(),
                                    })
                                    .child(div().text_color(p.ash.opacity(0.5)).child("·"))
                                    .child(format!("{count} треков"))
                                    .child(div().text_color(p.ash.opacity(0.5)).child("·"))
                                    .child(format!("{minutes} мин")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(self.pill(
                                        if is_playing { "pause" } else { "play" },
                                        if is_playing { "пауза" } else { "играть" },
                                        true,
                                        cx.listener(move |this, _, _, _| {
                                            if this
                                                .snap
                                                .track
                                                .as_ref()
                                                .map(|t| t.album_id == album_id)
                                                .unwrap_or(false)
                                            {
                                                this.audio.toggle();
                                            } else {
                                                this.play_album(album_id, 0);
                                            }
                                        }),
                                    ))
                                    .child(self.pill(
                                        "wave",
                                        "волна отсюда",
                                        false,
                                        cx.listener(move |this, _, _, cx| {
                                            this.set_tab(Tab::Wave, cx);
                                        }),
                                    )),
                            ),
                    )
                    .when(self.wide && tracks.len() > 1, |el| {
                        el.child(self.hero_tracks(album_id, &tracks, cx))
                    }),
            )
    }

    /// The album's first tracks, as a column of quiet rows inside the hero.
    /// It is the one thing a person wants from an album card that a giant title
    /// does not give them: track two.
    fn hero_tracks(
        &self,
        album_id: i64,
        tracks: &[Track],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.palette;
        let mut list = div()
            .w(px(320.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(1.))
            .pb(px(4.));
        for (i, t) in tracks.iter().take(6).enumerate() {
            let id = t.id;
            let playing = self.snap.track.as_ref().map(|x| x.id) == Some(id);
            list = list.child(
                div()
                    .id(SharedString::from(format!("hero-track-{id}")))
                    .h(px(30.))
                    .px(px(8.))
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .when(playing, |el| el.bg(p.card))
                    .hover(|el| el.bg(p.card_hover))
                    .child(
                        div()
                            .w(px(14.))
                            .text_size(px(10.))
                            .text_color(if playing { p.accent } else { p.ash })
                            .child(format!("{:02}", i + 1)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.5))
                            .text_color(if playing { p.accent } else { p.body })
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(t.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(p.ash)
                            .child(fmt_duration(t.duration())),
                    )
                    .on_click(cx.listener(move |this, _, _, _| this.play_album(album_id, i))),
            );
        }
        list
    }

    fn section_title(&self, title: &str, action: Option<&str>) -> impl IntoElement {
        let p = self.palette;
        div()
            .flex()
            .items_end()
            .justify_between()
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.ink)
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.ash)
                    .child(action.unwrap_or("").to_string()),
            )
    }

    fn album_tile(
        &self,
        id: i64,
        title: &str,
        artist: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = self.palette;
        let art = self.covers.get(id);
        div()
            .id(SharedString::from(format!("tile-{id}")))
            .w(px(172.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .w_full()
                    .h(px(172.))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(p.card)
                    .hover(|el| el.opacity(0.86))
                    .child(match art {
                        Some(art) => img(art.cover.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .size_full()
                            .bg(p.canvas)
                            .into_any_element(),
                    }),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.ink)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(p.ash)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(artist.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open_album(id, cx)))
    }

    fn library(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let mut grid = div().flex().flex_wrap().gap(px(20.));
        for album in self.albums.iter() {
            grid = grid.child(self.album_tile(album.id, &album.title, &album.artist, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .pt(px(10.))
            .child(self.heading("библиотека", Some(format!("{} альбомов", self.albums.len()))))
            .child(grid)
            .when(self.albums.is_empty(), |el| {
                el.child(
                    div()
                        .py(px(80.))
                        .text_size(px(13.))
                        .text_color(p.ash)
                        .child(if self.ready {
                            "пусто".to_string()
                        } else {
                            "загружаю…".to_string()
                        }),
                )
            })
    }

    fn liked_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let mut list = div().flex().flex_col();
        let liked: Vec<Track> = self
            .tracks
            .iter()
            .filter(|t| self.liked.contains(&t.id))
            .cloned()
            .collect();
        for (i, t) in liked.iter().enumerate() {
            list = list.child(self.track_row(t, i, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .pt(px(10.))
            .child(self.heading("любимое", Some(format!("{} треков", liked.len()))))
            .child(list)
            .when(liked.is_empty(), |el| {
                el.child(
                    div()
                        .py(px(60.))
                        .text_size(px(13.))
                        .text_color(p.ash)
                        .child("здесь пока пусто"),
                )
            })
    }

    fn album_view(&self, id: i64, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let album = self.albums.iter().find(|a| a.id == id);
        let art = self.covers.get(id);
        let title = album.map(|a| a.title.clone()).unwrap_or_default();
        let artist = album.map(|a| a.artist.clone()).unwrap_or_default();
        let tracks = self.album_tracks(id);
        let mut list = div().flex().flex_col();
        for (i, t) in tracks.iter().enumerate() {
            list = list.child(self.track_row(t, i, cx));
        }

        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .pt(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("back")
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(999.))
                            .hover(|el| el.bg(p.card))
                            .child(
                                svg()
                                    .path(asset("chevron"))
                                    .size(px(13.))
                                    .text_color(p.mute)
                                    .with_transformation(Transformation::rotate(gpui::percentage(0.75))),
                            )
                            .child(div().text_size(px(12.)).text_color(p.mute).child("назад"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.album = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(26.))
                    .items_end()
                    .child(
                        div()
                            .w(px(200.))
                            .h(px(200.))
                            .rounded(px(16.))
                            .overflow_hidden()
                            .bg(p.card)
                            .shadow(vec![BoxShadow {
                                color: p.shadow,
                                offset: point(px(0.), px(20.)),
                                blur_radius: px(50.),
                                spread_radius: px(-10.),
                            }])
                            .child(match art {
                                Some(art) => img(art.cover.clone())
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .into_any_element(),
                                None => div().size_full().into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap(px(8.))
                            .pb(px(6.))
                            .child(
                                div()
                                    .text_size(px(42.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(div().text_size(px(13.)).text_color(p.mute).child(artist))
                            .child(
                                div()
                                    .pt(px(8.))
                                    .flex()
                                    .child(self.pill(
                                        "play",
                                        "играть",
                                        true,
                                        cx.listener(move |this, _, _, _| this.play_album(id, 0)),
                                    )),
                            ),
                    ),
            )
            .child(list)
    }

    fn track_row(&self, track: &Track, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let playing = self.snap.track.as_ref().map(|t| t.id) == Some(track.id);
        let liked = self.liked.contains(&track.id);
        let id = track.id;
        let number = format!("{:02}", track.track_no.unwrap_or(index as i64 + 1));
        div()
            .id(SharedString::from(format!("track-{id}")))
            .flex()
            .items_center()
            .gap(px(12.))
            .h(px(44.))
            .px(px(10.))
            .rounded(px(9.))
            .when(playing, |el| el.bg(p.card))
            .hover(|el| el.bg(p.card_hover))
            .child(
                div()
                    .w(px(22.))
                    .text_size(px(11.))
                    .text_color(if playing { p.accent } else { p.ash })
                    .child(if playing { "▶".to_string() } else { number }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.5))
                    .text_color(if playing { p.accent } else { p.ink })
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(track.title.clone()),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.ash)
                    .child(track.format()),
            )
            .child(
                div()
                    .w(px(46.))
                    .text_size(px(11.))
                    .text_color(p.ash)
                    .child(fmt_duration(track.duration())),
            )
            .child(
                div()
                    .id(SharedString::from(format!("like-{id}")))
                    .w(px(24.))
                    .flex()
                    .justify_center()
                    .child(
                        svg()
                            .path(asset("heart"))
                            .size(px(13.))
                            .text_color(if liked { p.accent } else { p.ash.opacity(0.0) }),
                    )
                    .hover(|el| el.opacity(1.0))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_like(id, cx);
                        cx.stop_propagation();
                    })),
            )
            .on_click(cx.listener(move |this, _, _, _| {
                let t = this.tracks.iter().find(|t| t.id == id).cloned();
                if let Some(t) = t {
                    this.toggle_track(&t);
                }
            }))
    }

    fn pill(
        &self,
        icon_name: &str,
        label: &str,
        primary: bool,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let p = self.palette;
        div()
            .id(SharedString::from(format!("pill-{label}")))
            .flex()
            .items_center()
            .gap(px(7.))
            .h(px(34.))
            .px(px(14.))
            .rounded(px(999.))
            // The primary action is the loudest thing on the screen, so it is
            // plain ink rather than the cover's accent: the accent belongs to
            // the record, not to a button.
            .when(primary, |el| el.bg(p.ink))
            .when(!primary, |el| {
                el.bg(p.card).border_1().border_color(p.hairline)
            })
            .hover(|el| el.opacity(0.88))
            .child(
                svg()
                    .path(asset(icon_name))
                    .size(px(12.))
                    .text_color(if primary { p.canvas } else { p.mute }),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if primary { p.canvas } else { p.mute })
                    .child(label.to_string()),
            )
            .on_click(on_click)
    }

    fn player_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let snap = &self.snap;
        let track = snap.track.clone();
        let art = track
            .as_ref()
            .and_then(|t| self.covers.get(t.album_id))
            .map(|a| a.cover.clone());
        let playing = snap.playing;
        let title = track.as_ref().map(|t| t.title.clone()).unwrap_or_default();
        let artist = track
            .as_ref()
            .map(|t| {
                if t.album.is_empty() {
                    t.artist.clone()
                } else {
                    format!("{} · {}", t.artist, t.album)
                }
            })
            .unwrap_or_default();

        div().px(px(16.)).pb(px(16.)).child(
            div()
                .h(px(84.))
                .w_full()
                .rounded(px(20.))
                .bg(p.bar)
                .border_1()
                .border_color(p.hairline)
                .shadow(vec![BoxShadow {
                    color: p.shadow.opacity(0.45),
                    offset: point(px(0.), px(14.)),
                    blur_radius: px(40.),
                    spread_radius: px(-10.),
                }])
                .flex()
                .items_center()
                .px(px(16.))
                .gap(px(18.))
                // left: what is playing
                .child(
                    div()
                        .w(px(300.))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .id("bar-cover")
                                .w(px(50.))
                                .h(px(50.))
                                .rounded(px(11.))
                                .overflow_hidden()
                                .flex_shrink_0()
                                .bg(p.card_hover)
                                .child(match art {
                                    Some(art) => img(art)
                                        .size_full()
                                        .object_fit(ObjectFit::Cover)
                                        .into_any_element(),
                                    None => div().size_full().into_any_element(),
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(if track.is_some() { p.ink } else { p.ash })
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(if track.is_some() {
                                            title
                                        } else {
                                            "ничего не играет".to_string()
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.ash)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(artist),
                                ),
                        ),
                )
                // centre: transport + seek
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(self.transport(
                                    "prev",
                                    15.0,
                                    p.mute,
                                    cx.listener(|this, _, _, _| this.audio.prev()),
                                ))
                                .child(
                                    div()
                                        .id("play")
                                        .size(px(38.))
                                        .rounded(px(999.))
                                        .bg(p.ink)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .hover(|el| el.opacity(0.88))
                                        .child(
                                            svg()
                                                .path(asset(if playing { "pause" } else { "play" }))
                                                .size(px(14.))
                                                .text_color(p.canvas),
                                        )
                                        .on_click(cx.listener(|this, _, _, _| this.audio.toggle())),
                                )
                                .child(self.transport(
                                    "next",
                                    15.0,
                                    p.mute,
                                    cx.listener(|this, _, _, _| this.audio.next()),
                                )),
                        )
                        .child(self.seek_bar(p, cx)),
                )
                // right: volume and cast, kept quiet
                .child(
                    div()
                        .w(px(150.))
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(4.))
                        .child(
                            div()
                                .id("volume")
                                .size(px(32.))
                                .rounded(px(9.))
                                .hover(|el| el.bg(p.card_hover))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    svg()
                                        .path(asset("volume"))
                                        .size(px(15.))
                                        .text_color(if snap.volume <= 0.001 {
                                            p.accent
                                        } else {
                                            p.mute
                                        }),
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_mute(cx))),
                        )
                        .child(
                            div()
                                .id("queue")
                                .size(px(32.))
                                .rounded(px(9.))
                                .hover(|el| el.bg(p.card_hover))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    svg()
                                        .path(asset("queue"))
                                        .size(px(15.))
                                        .text_color(p.mute),
                                ),
                        ),
                ),
        )
    }

    fn transport(
        &self,
        icon_name: &str,
        size: f32,
        color: gpui::Hsla,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("t-{icon_name}")))
            .size(px(30.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .hover(|el| el.opacity(0.7))
            .child(
                svg()
                    .path(asset(icon_name))
                    .size(px(size))
                    .text_color(color),
            )
            .on_click(on_click)
    }

    fn seek_bar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.snap.clone();
        let length = snap.length.as_secs_f32().max(0.001);
        let ratio = (snap.position.as_secs_f32() / length).clamp(0.0, 1.0);
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let read = bounds.clone();
        let write = bounds.clone();
        let elapsed = fmt_duration(snap.position);
        let total = fmt_duration(snap.length);

        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .w(px(38.))
                    .text_size(px(10.5))
                    .text_color(p.ash)
                    .child(elapsed),
            )
            .child(
                div()
                    .id("seek")
                    .flex_1()
                    .h(px(18.))
                    .relative()
                    .flex()
                    .items_center()
                    .child(
                        canvas(
                            move |bounds, _, _| read.set(bounds),
                            move |bounds, _, window, _| {
                                let h = px(3.);
                                let track = Bounds {
                                    origin: point(bounds.origin.x, bounds.center().y - h / 2.0),
                                    size: gpui::size(bounds.size.width, h),
                                };
                                window.paint_quad(quad(
                                    track,
                                    px(2.),
                                    p.ink.opacity(0.14),
                                    px(0.),
                                    transparent_black(),
                                    Default::default(),
                                ));
                                let filled = Bounds {
                                    origin: track.origin,
                                    size: gpui::size(
                                        px(f32::from(track.size.width) * ratio),
                                        track.size.height,
                                    ),
                                };
                                window.paint_quad(quad(
                                    filled,
                                    px(2.),
                                    p.accent,
                                    px(0.),
                                    transparent_black(),
                                    Default::default(),
                                ));
                                if ratio > 0.0 {
                                    let dot = px(7.);
                                    window.paint_quad(quad(
                                        Bounds {
                                            origin: point(
                                                filled.origin.x + filled.size.width - dot / 2.0,
                                                bounds.center().y - dot / 2.0,
                                            ),
                                            size: gpui::size(dot, dot),
                                        },
                                        px(999.),
                                        p.accent,
                                        px(0.),
                                        transparent_black(),
                                        Default::default(),
                                    ));
                                }
                            },
                        )
                        .size_full(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, _| {
                            let b = write.get();
                            let width = f32::from(b.size.width);
                            if width <= 0.0 {
                                return;
                            }
                            let t = ((f32::from(event.position.x) - f32::from(b.origin.x)) / width)
                                .clamp(0.0, 1.0);
                            let to = Duration::from_secs_f32(t * this.snap.length.as_secs_f32());
                            this.audio.seek(to);
                        }),
                    ),
            )
            .child(
                div()
                    .w(px(38.))
                    .text_size(px(10.5))
                    .text_color(p.ash)
                    .child(total),
            )
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.wide = f32::from(window.viewport_size().width) >= 1180.0;
        let p = self.palette;
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(p.canvas.opacity(if p.light { 0.50 } else { 0.34 }))
            .font_family(FONT)
            .text_color(p.ink)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if this.tab != Tab::Search {
                    return;
                }
                let api = this.api.clone();
                let search = this.search.clone();
                let _ = search.update(cx, |s, cx| {
                    if s.on_key(&event.keystroke) {
                        s.run(&api, cx);
                    }
                });
            }))
            .child(self.backdrop())
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(self.rail(cx))
                            .child(self.content(cx)),
                    )
                    .child(self.player_bar(cx)),
            )
    }
}

fn asset(name: &str) -> SharedString {
    SharedString::from(format!("assets/icons/{name}.svg"))
}

fn fmt_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn chrono_hour() -> u32 {
    // No time crate for one number: libc's localtime is enough.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let local = unsafe {
        let t = now as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm.tm_hour
    };
    local.clamp(0, 23) as u32
}
