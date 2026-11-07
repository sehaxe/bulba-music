//! The wave screen: an endless recommendation stream.
//!
//! The server decides what comes next and says why; this module only holds the
//! list and draws it. Fetching stays blocking and belongs to a background
//! thread — `render` is pure.

use std::collections::HashSet;

use gpui::{FontWeight, SharedString, div, prelude::*, px};

use crate::api::{Api, WaveTrack};
use crate::theme::Palette;

const BATCH: usize = 60;

pub struct Wave {
    pub tracks: Vec<WaveTrack>,
    pub seed: Option<i64>,
    pub loading: bool,
}

impl Wave {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            seed: None,
            loading: false,
        }
    }

    /// Fetch the first batch. Blocking; call it from a background thread.
    pub fn load(api: &Api, seed: Option<i64>) -> anyhow::Result<Self> {
        let reply = api.wave(seed, &[], BATCH)?;
        Ok(Self {
            tracks: reply.tracks,
            seed: reply.seed.or(seed),
            loading: false,
        })
    }

    /// Fetch more, excluding ids already in self.tracks. Blocking.
    pub fn append(&mut self, api: &Api) -> anyhow::Result<()> {
        let exclude: Vec<i64> = self.tracks.iter().map(|t| t.track.id).collect();
        let reply = api.wave(self.seed, &exclude, BATCH)?;
        self.seed = reply.seed.or(self.seed);
        let mut seen: HashSet<i64> = exclude.into_iter().collect();
        self.tracks
            .extend(reply.tracks.into_iter().filter(|t| seen.insert(t.track.id)));
        Ok(())
    }
}

/// Renders the list. `on_play` is called with the track id, `on_open_album`
/// with album id. Both must be 'static + Clone.
pub fn render(
    wave: &Wave,
    p: Palette,
    playing: Option<i64>,
    covers: impl Fn(i64) -> Option<std::sync::Arc<gpui::RenderImage>> + Clone + 'static,
    on_play: impl Fn(i64) + Clone + 'static,
    on_open_album: impl Fn(i64) + Clone + 'static,
) -> impl IntoElement {
    let count = wave.tracks.len();
    let mut rows = div().flex().flex_col().gap(px(2.));

    for (index, item) in wave.tracks.iter().enumerate() {
        let track = &item.track;
        let id = track.id;
        let album_id = track.album_id;
        let is_playing = playing == Some(id);
        let play = on_play.clone();
        let open_album = on_open_album.clone();
        let cover = covers(album_id);
        let reason = clamp_reason(&item.reason);
        let key = track
            .camelot
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_uppercase);
        let subtitle = match (track.artist.as_str(), track.album.as_str()) {
            ("", "") => String::new(),
            ("", album) => album.to_string(),
            (artist, "") => artist.to_string(),
            (artist, album) => format!("{artist} · {album}"),
        };

        rows = rows.child(
            div()
                .id(SharedString::from(format!("wave-row-{index}-{id}")))
                .h(px(56.))
                .w_full()
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(10.))
                .rounded(px(10.))
                .when(is_playing, |el| el.bg(p.card))
                .hover(|el| el.bg(p.card_hover))
                .child(
                    div()
                        .id(SharedString::from(format!("wave-art-{index}-{id}")))
                        .relative()
                        .size(px(44.))
                        .flex_shrink_0()
                        .rounded(px(9.))
                        .overflow_hidden()
                        .bg(p.card)
                        .cursor_pointer()
                        .hover(|el| el.opacity(0.82))
                        .child(match cover {
                            Some(art) => gpui::img(gpui::ImageSource::Render(art))
                                .size_full()
                                .object_fit(gpui::ObjectFit::Cover)
                                .into_any_element(),
                            None => gpui::div().size_full().into_any_element(),
                        })
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(3.))
                                .when(is_playing, |el| {
                                    el.child(div().w(px(3.)).h(px(8.)).rounded(px(999.)).bg(p.accent))
                                        .child(div().w(px(3.)).h(px(16.)).rounded(px(999.)).bg(p.accent))
                                        .child(div().w(px(3.)).h(px(11.)).rounded(px(999.)).bg(p.accent))
                                }),
                        )
                        .on_click(move |_, _, _| open_album(album_id)),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("wave-play-{index}-{id}")))
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .cursor_pointer()
                        .on_click(move |_, _, _| play(id))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(15.))
                                        .line_height(px(19.))
                                        .font_weight(if is_playing {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::MEDIUM
                                        })
                                        .text_color(if is_playing { p.accent } else { p.ink })
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(track.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(p.ash)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(subtitle),
                                ),
                        )
                        .child(
                            div()
                                .max_w(px(340.))
                                .text_size(px(11.))
                                .text_color(p.mute)
                                .text_right()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(reason),
                        )
                        .child(
                            div()
                                .w(px(34.))
                                .flex_shrink_0()
                                .flex()
                                .justify_end()
                                .child(match key {
                                    Some(key) => div()
                                        .h(px(20.))
                                        .px(px(7.))
                                        .rounded(px(999.))
                                        .border_1()
                                        .border_color(p.hairline)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(p.mute)
                                                .child(key),
                                        )
                                        .into_any_element(),
                                    None => div().into_any_element(),
                                }),
                        ),
                ),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(18.))
        .pt(px(10.))
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(30.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.ink)
                                .child("волна"),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.ash)
                                .child("бесконечный поток · подстраивается под то, что играет"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.ash)
                        .text_right()
                        .child(format!(
                            "{count} треков{}",
                            if wave.loading { " · собираю" } else { "" }
                        )),
                ),
        )
        .when(count == 0, |el| {
            el.child(
                div()
                    .w_full()
                    .py(px(90.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(p.ash)
                            .child("волна собирается…"),
                    ),
            )
        })
        .when(count > 0, |el| el.child(rows))
}

/// The server's reason is a sentence, not a paragraph; keep the row on one line.
fn clamp_reason(reason: &str) -> String {
    match reason.char_indices().nth(60) {
        Some((cut, _)) => format!("{}…", &reason[..cut]),
        None => reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_reason;

    #[test]
    fn reason_is_capped_at_sixty_chars() {
        let long = "я".repeat(61);
        let cut = clamp_reason(&long);
        assert_eq!(cut.chars().count(), 61);
        assert!(cut.ends_with('…'));
        assert_eq!(clamp_reason("потому что ты слушал"), "потому что ты слушал");
    }
}
