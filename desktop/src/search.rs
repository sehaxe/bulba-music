//! Search: one field, three shelves. The field is a controlled div — the
//! window forwards keys through `Search::on_key`, so there is no hidden input
//! widget to keep in sync with the query string.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, Context, CursorStyle, FocusHandle, Focusable, FontWeight,
    SharedString, div, prelude::*, px,
};

use crate::api::{SearchResults, Track};
use crate::theme::Palette;

pub struct Search {
    pub query: String,
    pub results: Option<SearchResults>,
    pub loading: bool,
    pub focus: FocusHandle,
}

impl Search {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            query: String::new(),
            results: None,
            loading: false,
            focus: cx.focus_handle(),
        }
    }

    /// Runs the query on a background thread and stores results (no debounce here;
    /// the caller decides when to call it).
    pub fn run(&mut self, api: &crate::api::Api, cx: &mut Context<Self>) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            self.results = None;
            self.loading = false;
            cx.notify();
            return;
        }

        self.loading = true;
        cx.notify();

        let api = api.clone();
        cx.spawn(async move |this, cx| {
            let asked = query.clone();
            let found = cx
                .background_executor()
                .spawn(async move { api.search(&query) })
                .await;
            let _ = this.update(cx, |this, cx| {
                // A slower answer for an older query must not overwrite a newer one.
                if this.query.trim() != asked {
                    return;
                }
                this.loading = false;
                this.results = Some(found.unwrap_or_default());
                cx.notify();
            });
        })
        .detach();
    }

    /// Applies one keystroke to the query. Returns true when the key belonged
    /// to the field, so the caller knows whether to run the search again.
    pub fn on_key(&mut self, keystroke: &gpui::Keystroke) -> bool {
        if keystroke.key == "backspace" {
            return self.query.pop().is_some();
        }

        if keystroke.modifiers.control || keystroke.modifiers.platform || keystroke.modifiers.alt {
            return false;
        }

        let Some(text) = keystroke.key_char.as_deref() else {
            return false;
        };
        if text.chars().any(|c| c.is_control()) {
            return false;
        }

        self.query.push_str(text);
        true
    }
}

impl Focusable for Search {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Renders the field + grouped results (tracks, albums, artists).
pub fn render(
    search: &Search,
    p: Palette,
    on_play: impl Fn(i64) + Clone + 'static,
    on_open_album: impl Fn(i64) + Clone + 'static,
) -> impl gpui::IntoElement {
    let focus = search.focus.clone();
    let empty = search.query.is_empty();

    let mut line = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(2.))
        .text_size(px(20.));

    if empty {
        line = line.child(caret(p)).child(
            div()
                .whitespace_nowrap()
                .text_color(p.ash.opacity(0.75))
                .child("поиск по библиотеке"),
        );
    } else {
        line = line
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(p.ink)
                    .child(search.query.clone()),
            )
            .child(caret(p));
    }

    let field = div()
        .id("search-field")
        .w_full()
        .h(px(52.))
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(18.))
        .rounded(px(14.))
        .bg(p.card)
        .border_1()
        .border_color(p.hairline)
        .track_focus(&search.focus)
        .focus(|el| el.border_color(p.accent.opacity(0.55)))
        .cursor(CursorStyle::IBeam)
        .on_click(move |_, window, _| window.focus(&focus))
        .child(
            div()
                .flex_shrink_0()
                .size(px(12.))
                .rounded(px(999.))
                .border_1()
                .border_color(p.ash),
        )
        .child(line)
        .when(search.loading, |el| {
            el.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(p.ash)
                    .child("ищу…"),
            )
        });

    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(24.))
        .pt(px(18.))
        .child(field);

    if let Some(found) = &search.results {
        if !found.tracks.is_empty() {
            let mut list = div().flex().flex_col();
            for (i, track) in found.tracks.iter().enumerate() {
                list = list.child(track_row(p, track, i, on_play.clone()));
            }
            column = column.child(group(p, "треки", list));
        }

        if !found.albums.is_empty() {
            let mut list = div().flex().flex_col();
            for album in &found.albums {
                list = list.child(album_row(p, album, on_open_album.clone()));
            }
            column = column.child(group(p, "альбомы", list));
        }

        if !found.artists.is_empty() {
            let mut list = div().flex().flex_col();
            for artist in &found.artists {
                list = list.child(artist_row(p, artist));
            }
            column = column.child(group(p, "артисты", list));
        }

        let nothing =
            found.tracks.is_empty() && found.albums.is_empty() && found.artists.is_empty();
        if nothing && !search.loading {
            column = column.child(
                div()
                    .py(px(48.))
                    .text_size(px(13.))
                    .text_color(p.ash)
                    .child("ничего не найдено"),
            );
        }
    }

    column
}

fn caret(p: Palette) -> impl IntoElement {
    div()
        .flex_shrink_0()
        .w(px(1.5))
        .h(px(20.))
        .rounded(px(1.))
        .bg(p.accent)
        .with_animation(
            "search-caret",
            Animation::new(Duration::from_millis(1060)).repeat(),
            |el, delta| if delta < 0.5 { el } else { el.opacity(0.0) },
        )
}

fn group(p: Palette, label: &str, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.ash)
                .child(label.to_uppercase()),
        )
        .child(body)
}

fn track_row(
    p: Palette,
    track: &Track,
    index: usize,
    on_play: impl Fn(i64) + 'static,
) -> impl IntoElement {
    let id = track.id;
    let number = format!("{:02}", track.track_no.unwrap_or(index as i64 + 1));
    div()
        .id(SharedString::from(format!("search-track-{id}")))
        .flex()
        .items_center()
        .gap(px(12.))
        .h(px(44.))
        .px(px(10.))
        .rounded(px(9.))
        .hover(|el| el.bg(p.card_hover))
        .child(
            div()
                .w(px(22.))
                .text_size(px(11.))
                .text_color(p.ash)
                .child(number),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.5))
                .text_color(p.ink)
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
        .on_click(move |_, _, _| on_play(id))
}

fn album_row(
    p: Palette,
    album: &crate::api::AlbumSummary,
    on_open_album: impl Fn(i64) + 'static,
) -> impl IntoElement {
    let id = album.id;
    div()
        .id(SharedString::from(format!("search-album-{id}")))
        .h(px(40.))
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(10.))
        .rounded(px(9.))
        .hover(|el| el.bg(p.card_hover))
        .child(
            div()
                .flex_shrink_0()
                .size(px(28.))
                .rounded(px(7.))
                .bg(p.card_hover),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.5))
                .text_color(p.ink)
                .overflow_hidden()
                .text_ellipsis()
                .child(album.title.clone()),
        )
        .child(
            div()
                .max_w(px(200.))
                .text_size(px(11.5))
                .text_color(p.ash)
                .overflow_hidden()
                .text_ellipsis()
                .child(album.artist.clone()),
        )
        .child(
            div()
                .w(px(70.))
                .text_size(px(11.))
                .text_color(p.ash)
                .child(format!("{} треков", album.tracks)),
        )
        .on_click(move |_, _, _| on_open_album(id))
}

fn artist_row(p: Palette, artist: &crate::api::ArtistSummary) -> impl IntoElement {
    div()
        .h(px(40.))
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(10.))
        .rounded(px(9.))
        .child(
            div()
                .flex_shrink_0()
                .size(px(28.))
                .rounded(px(999.))
                .bg(p.card_hover),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.5))
                .text_color(p.ink)
                .overflow_hidden()
                .text_ellipsis()
                .child(artist.name.clone()),
        )
        .child(div().text_size(px(11.)).text_color(p.ash).child(format!(
            "{} альбомов · {} треков",
            artist.albums, artist.tracks
        )))
}

fn fmt_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
