//! Playlists: the cards, one playlist's tracks, and the five calls that keep
//! them in sync with the server.
//!
//! Every network call is blocking and belongs on a background thread; `render`
//! is pure. The client only has GET and POST, so PATCH and DELETE are spoken to
//! `ureq` here instead of growing `api.rs` for two endpoints.

#![allow(dead_code)] // the shell wires this screen in a later pass

use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::{FontWeight, ObjectFit, RenderImage, SharedString, div, img, prelude::*, px, svg};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::api::{Api, Track};
use crate::theme::Palette;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PlaylistSummary {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tracks: i64,
    #[serde(default, deserialize_with = "de_flag")]
    pub has_cover: bool,
}

pub struct Playlists {
    pub items: Vec<PlaylistSummary>,
    pub loading: bool,
}

#[derive(Deserialize)]
struct Created {
    id: i64,
}

#[derive(Deserialize, Default)]
struct Page {
    #[serde(default)]
    tracks: Vec<Track>,
}

impl Playlists {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            loading: false,
        }
    }

    /// Every playlist the server will show this listener. Blocking.
    pub fn load(api: &Api) -> Result<Self> {
        let items: Vec<PlaylistSummary> = parse(api, "/api/playlists")?;
        Ok(Self {
            items,
            loading: false,
        })
    }

    pub fn create(api: &Api, name: &str) -> Result<i64> {
        let reply: Created = api.post(
            "/api/playlists",
            &json!({ "name": name, "is_public": false }),
        )?;
        Ok(reply.id)
    }

    pub fn add_track(api: &Api, playlist: i64, track: i64) -> Result<()> {
        let _: Value = api.post(
            &format!("/api/playlists/{playlist}/tracks"),
            &json!({ "track_id": track }),
        )?;
        Ok(())
    }

    pub fn remove_track(api: &Api, playlist: i64, track: i64) -> Result<()> {
        delete(api, &format!("/api/playlists/{playlist}/tracks/{track}"))
    }

    pub fn rename(api: &Api, playlist: i64, name: &str) -> Result<()> {
        patch(
            api,
            &format!("/api/playlists/{playlist}"),
            &json!({ "name": name }),
        )
    }

    pub fn remove(api: &Api, playlist: i64) -> Result<()> {
        delete(api, &format!("/api/playlists/{playlist}"))
    }

    /// The playlist's tracks in their saved order. Blocking.
    pub fn tracks(api: &Api, playlist: i64) -> Result<Vec<Track>> {
        let page: Page = parse(api, &format!("/api/playlists/{playlist}"))?;
        Ok(page.tracks)
    }
}

/// `api.rs` keeps its JSON reader private; the public byte call is enough.
fn parse<T: DeserializeOwned>(api: &Api, path: &str) -> Result<T> {
    let bytes = api.get_bytes(path)?;
    serde_json::from_slice(&bytes).with_context(|| format!("{path}: bad json"))
}

fn delete(api: &Api, path: &str) -> Result<()> {
    let url = format!("{}{}", api.base, path);
    let mut req = ureq::delete(&url);
    if let Some(token) = &api.token {
        req = req.header("authorization", &format!("Bearer {token}"));
    }
    req.call().with_context(|| format!("DELETE {url}"))?;
    Ok(())
}

fn patch(api: &Api, path: &str, body: &Value) -> Result<()> {
    let url = format!("{}{}", api.base, path);
    let payload = serde_json::to_string(body)?;
    let mut req = ureq::patch(&url).header("content-type", "application/json");
    if let Some(token) = &api.token {
        req = req.header("authorization", &format!("Bearer {token}"));
    }
    req.send(payload.as_str())
        .with_context(|| format!("PATCH {url}"))?;
    Ok(())
}

/// The server writes `has_cover` as a bool on playlists and as 0/1 on albums.
fn de_flag<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<bool, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Bool(b) => b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        _ => false,
    })
}

/// The grid of playlist cards. `covers` gives up to four cover images for the
/// mosaic; the first is shown full when a playlist only has one.
pub fn render_list(
    p: Palette,
    lists: &[PlaylistSummary],
    covers: impl Fn(i64) -> Vec<Arc<RenderImage>> + Clone + 'static,
    on_open: impl Fn(i64) + Clone + 'static,
    on_create: impl Fn() + Clone + 'static,
) -> impl IntoElement {
    let mut grid = div().flex().flex_wrap().gap(px(20.));

    for item in lists {
        let id = item.id;
        let arts = covers(id);
        let face = match arts.len() {
            0 => div().size_full().bg(p.card_hover).into_any_element(),
            1 => div()
                .size_full()
                .child(
                    img(gpui::ImageSource::Render(
                        arts.into_iter().next().expect("one cover"),
                    ))
                    .size_full()
                    .object_fit(ObjectFit::Cover),
                )
                .into_any_element(),
            _ => {
                let mut mosaic = div().flex().flex_wrap().gap(px(1.));
                for i in 0..4 {
                    mosaic = mosaic.child(cell(p, arts.get(i).cloned()));
                }
                mosaic.into_any_element()
            }
        };
        let open = on_open.clone();
        let count = item.tracks;
        let subtitle = format!("{count} {}", tracks_word(count));

        grid = grid.child(
            div()
                .id(SharedString::from(format!("pl-card-{id}")))
                .w(px(244.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .on_click(move |_, _, _| open(id))
                .child(
                    div()
                        .id(SharedString::from(format!("pl-cover-{id}")))
                        .w_full()
                        .h(px(244.))
                        .rounded(px(12.))
                        .overflow_hidden()
                        .bg(p.card_hover)
                        .cursor_pointer()
                        .hover(|el| el.opacity(0.86))
                        .child(face),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.ink)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(item.name.clone()),
                )
                .child(div().text_size(px(11.)).text_color(p.ash).child(subtitle)),
        );
    }

    let create = on_create.clone();
    grid = grid.child(
        div()
            .id("pl-new")
            .w(px(244.))
            .h(px(244.))
            .rounded(px(12.))
            .border_1()
            .border_dashed()
            .border_color(p.hairline)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|el| el.bg(p.card))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(p.mute)
                    .child("＋ новый плейлист"),
            )
            .on_click(move |_, _, _| create()),
    );

    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .when(lists.is_empty(), |el| {
            el.child(
                div()
                    .py(px(24.))
                    .text_size(px(12.5))
                    .text_color(p.ash)
                    .child("плейлистов пока нет"),
            )
        })
        .child(grid)
}

fn cell(p: Palette, art: Option<Arc<RenderImage>>) -> gpui::AnyElement {
    div()
        .size(px(121.5))
        .overflow_hidden()
        .bg(p.card_hover)
        .child(match art {
            Some(art) => img(gpui::ImageSource::Render(art))
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div().size_full().into_any_element(),
        })
        .into_any_element()
}

/// One playlist: its tracks and actions.
pub fn render_one(
    p: Palette,
    name: &str,
    tracks: &[Track],
    playing: Option<i64>,
    on_play: impl Fn(usize) + Clone + 'static,
    on_remove_track: impl Fn(i64) + Clone + 'static,
    on_rename: impl Fn() + Clone + 'static,
    on_delete: impl Fn() + Clone + 'static,
) -> impl IntoElement {
    let count = tracks.len();
    let mut rows = div().flex().flex_col().gap(px(2.));

    for (index, track) in tracks.iter().enumerate() {
        let id = track.id;
        let is_playing = playing == Some(id);
        let group = SharedString::from(format!("pl-row-{id}"));
        let play = on_play.clone();
        let remove = on_remove_track.clone();
        let subtitle = match (track.artist.as_str(), track.album.as_str()) {
            ("", "") => String::new(),
            ("", album) => album.to_string(),
            (artist, "") => artist.to_string(),
            (artist, album) => format!("{artist} · {album}"),
        };

        rows = rows.child(
            div()
                .id(SharedString::from(format!("pl-track-{id}")))
                .group(group.clone())
                .h(px(40.))
                .px(px(10.))
                .rounded(px(9.))
                .flex()
                .items_center()
                .gap(px(12.))
                .when(is_playing, |el| el.bg(p.card))
                .hover(|el| el.bg(p.card_hover))
                .cursor_pointer()
                .on_click(move |_, _, _| play(index))
                .child(
                    div()
                        .w(px(26.))
                        .text_size(px(11.))
                        .text_color(if is_playing { p.accent } else { p.ash })
                        .child(if is_playing {
                            "▶".to_string()
                        } else {
                            format!("{:02}", index + 1)
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.))
                        .text_color(if is_playing { p.accent } else { p.ink })
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(track.title.clone()),
                )
                .child(
                    div()
                        .max_w(px(340.))
                        .text_size(px(11.5))
                        .text_color(p.ash)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(subtitle),
                )
                .child(
                    div()
                        .w(px(46.))
                        .text_size(px(11.))
                        .text_color(p.ash)
                        .text_right()
                        .child(fmt_duration(track.duration_ms)),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("pl-remove-{id}")))
                        .group_hover(group, |el| el.opacity(1.0))
                        .opacity(0.0)
                        .h(px(24.))
                        .px(px(6.))
                        .rounded(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_size(px(10.5))
                        .text_color(p.mute)
                        .hover(|el| el.bg(p.card_hover))
                        .child("убрать")
                        .child(div().text_size(px(13.)).child("×"))
                        .on_click(move |_, _, cx| {
                            remove(id);
                            cx.stop_propagation();
                        }),
                ),
        );
    }

    let play = on_play.clone();
    let empty = count == 0;
    let pill = div()
        .id("pl-play")
        .h(px(34.))
        .px(px(14.))
        .rounded(px(999.))
        .flex()
        .items_center()
        .gap(px(7.))
        .bg(p.ink)
        .when(empty, |el| el.opacity(0.4))
        .hover(|el| el.opacity(0.88))
        .child(
            svg()
                .path(SharedString::from("assets/icons/play.svg"))
                .size(px(12.))
                .text_color(p.canvas),
        )
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.canvas)
                .child("играть"),
        )
        .when(!empty, |el| el.on_click(move |_, _, _| play(0)));

    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(26.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.ink)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(name.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(p.ash)
                                .child(format!("{count} {}", tracks_word(count as i64))),
                        ),
                )
                .child(pill)
                .child(div().flex_1())
                .child(text_button("pl-rename", "переименовать", p, on_rename))
                .child(text_button("pl-delete", "удалить", p, on_delete)),
        )
        .child(div().h(px(1.)).w_full().bg(p.hairline))
        .child(if empty {
            div()
                .w_full()
                .py(px(60.))
                .flex()
                .justify_center()
                .text_size(px(12.5))
                .text_color(p.ash)
                .child("плейлист пуст")
                .into_any_element()
        } else {
            rows.into_any_element()
        })
}

fn text_button(
    id: &'static str,
    label: &'static str,
    p: Palette,
    cb: impl Fn() + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id))
        .h(px(30.))
        .px(px(10.))
        .rounded(px(8.))
        .flex()
        .items_center()
        .text_size(px(11.5))
        .text_color(p.mute)
        .hover(|el| el.bg(p.card_hover))
        .child(label)
        .on_click(move |_, _, _| cb())
}

fn tracks_word(n: i64) -> &'static str {
    if n == 1 { "трек" } else { "треков" }
}

fn fmt_duration(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}
