//! Settings: the server, what it is willing to serve, and the room's mood.
//!
//! State and transport here; drawing is pure. The shell owns persistence — a
//! callback fires with the new value and the shell decides what to do with it.

use std::rc::Rc;

use anyhow::anyhow;
use gpui::{Div, FontWeight, SharedString, div, prelude::*, px};

use crate::api::Api;
use crate::theme::Palette;

const QUALITIES: [(&str, &str); 4] = [
    ("flac", "мастер, без потерь"),
    ("opus160", "~160 kbps"),
    ("opus128", "~128 kbps"),
    ("opus96", "~96 kbps"),
];

const CROSSFADES: [f64; 5] = [0.0, 2.0, 4.0, 8.0, 12.0];

const THEMES: [(Theme, &str); 3] = [
    (Theme::Auto, "как обложка"),
    (Theme::Light, "светлая"),
    (Theme::Dark, "тёмная"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Auto,
    Light,
    Dark,
}

impl Theme {
    pub fn tag(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub server: String,
    /// Server tag, not a human label: `flac` | `opus160` | `opus128` | `opus96`.
    pub quality: String,
    pub crossfade: f64,
    pub replay_gain: bool,
    pub theme: Theme,
}

impl Settings {
    pub fn new(server: impl Into<String>) -> Self {
        Self {
            server: server.into(),
            quality: "flac".into(),
            crossfade: 4.0,
            replay_gain: true,
            theme: Theme::Auto,
        }
    }

    /// Pull the server's current policy. Blocking; call it off the UI thread.
    pub fn refresh(&mut self, api: &Api) {
        self.server = api.base.clone();
        if let Ok(info) = api.info()
            && let Some(serve) = info.stats.get("serve").and_then(|v| v.as_str())
            && let Some(tag) = normalize_quality(serve)
        {
            self.quality = tag.to_string();
        }
    }

    /// Tell the server what to hand out from now on. Blocking.
    pub fn save_quality(&self, api: &Api) -> anyhow::Result<()> {
        let quality = normalize_quality(&self.quality)
            .ok_or_else(|| anyhow!("неизвестное качество: {}", self.quality))?;
        let _: serde_json::Value =
            api.post("/api/config/serve", &serde_json::json!({ "quality": quality }))?;
        Ok(())
    }
}

/// `Rc`, not `Box`: gpui click listeners must be `'static` and every row needs
/// its own copy, so the callbacks have to be cloneable.
#[derive(Clone)]
pub struct Handlers {
    pub on_server: Rc<dyn Fn(String)>,
    /// The shell opens its own URL dialog; this screen has no text input.
    pub on_edit_server: Rc<dyn Fn()>,
    pub on_quality: Rc<dyn Fn(String)>,
    pub on_crossfade: Rc<dyn Fn(f64)>,
    pub on_replay_gain: Rc<dyn Fn(bool)>,
    pub on_theme: Rc<dyn Fn(Theme)>,
}

/// Server aliases are accepted; only the canonical tag is stored or sent.
fn normalize_quality(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "flac" | "master" => Some("flac"),
        "opus160" | "opus_160" => Some("opus160"),
        "opus128" | "opus_128" | "opus" => Some("opus128"),
        "opus96" | "opus_96" => Some("opus96"),
        _ => None,
    }
}

pub fn render(s: &Settings, p: Palette, h: &Handlers) -> impl IntoElement {
    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(28.))
        .pt(px(18.))
        .max_w(px(760.))
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
                        .child("настройки"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.ash)
                        .child("сервер, качество и звук"),
                ),
        );

    // server ---------------------------------------------------------------
    let edit = h.on_edit_server.clone();
    column = column.child(group(
        p,
        "сервер",
        row()
            .id("settings-server")
            .child(label(p, "адрес сервера", "куда подключается клиент", false))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .min_w_0()
                    .child(
                        div()
                            .max_w(px(380.))
                            .text_size(px(12.5))
                            .text_color(p.mute)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(s.server.clone()),
                    )
                    .child(chip(
                        p,
                        "settings-server-edit",
                        "изменить",
                        false,
                        move |_, _, _| edit(),
                    )),
            ),
    ));

    // quality --------------------------------------------------------------
    let mut qualities = div().flex().flex_col();
    for (tag, hint) in QUALITIES {
        let active = normalize_quality(&s.quality) == Some(tag);
        let pick = h.on_quality.clone();
        let value = tag.to_string();
        qualities = qualities.child(
            row()
                .id(SharedString::from(format!("settings-quality-{tag}")))
                .hover(|el| el.bg(p.card_hover))
                .child(label(p, tag, hint, active))
                .child(dot(p, active))
                .on_click(move |_, _, _| pick(value.clone())),
        );
    }
    column = column.child(group(p, "качество", qualities));

    // playback -------------------------------------------------------------
    let mut chips = div().flex().items_center().gap(px(6.));
    for secs in CROSSFADES {
        let active = (s.crossfade - secs).abs() < 0.01;
        let pick = h.on_crossfade.clone();
        chips = chips.child(chip(
            p,
            SharedString::from(format!("settings-crossfade-{secs:.0}")),
            &format!("{secs:.0} с"),
            active,
            move |_, _, _| pick(secs),
        ));
    }

    let gain = h.on_replay_gain.clone();
    let next = !s.replay_gain;
    let playback = div()
        .flex()
        .flex_col()
        .child(
            row()
                .child(label(
                    p,
                    "кроссфейд",
                    "плавный переход между треками",
                    false,
                ))
                .child(chips),
        )
        .child(
            row()
                .child(label(p, "ReplayGain", "выравнивание громкости", false))
                .child(
                    div()
                        .id("settings-replay-gain")
                        .w(px(34.))
                        .h(px(18.))
                        .rounded(px(999.))
                        .border_1()
                        .border_color(if s.replay_gain { p.accent } else { p.hairline })
                        .bg(if s.replay_gain { p.accent } else { p.card_hover })
                        .flex()
                        .items_center()
                        .px(px(2.))
                        .when(s.replay_gain, |el| el.justify_end())
                        .hover(|el| el.opacity(0.85))
                        .child(
                            div()
                                .size(px(14.))
                                .rounded(px(999.))
                                // A light knob on both rooms: canvas in the light,
                                // ink (near-white) in the dark.
                                .bg(if p.light { p.canvas } else { p.ink }),
                        )
                        .on_click(move |_, _, _| gain(next)),
                ),
        );
    column = column.child(group(p, "воспроизведение", playback));

    // theme ----------------------------------------------------------------
    let mut themes = div().flex().items_center().gap(px(6.));
    for (theme, name) in THEMES {
        let pick = h.on_theme.clone();
        themes = themes.child(chip(
            p,
            SharedString::from(format!("settings-theme-{}", theme.tag())),
            name,
            s.theme == theme,
            move |_, _, _| pick(theme),
        ));
    }
    column = column.child(group(
        p,
        "тема",
        row()
            .child(label(p, "оформление", "палитра окна", false))
            .child(themes),
    ));

    column.child(
        div()
            .pt(px(4.))
            .text_size(px(10.5))
            .text_color(p.ash)
            .child(format!("версия 0.1 · сервер {}", s.server)),
    )
}

fn group(p: Palette, title: &str, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .px(px(10.))
                .pb(px(8.))
                .border_b_1()
                .border_color(p.hairline)
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.ash)
                .child(title.to_uppercase()),
        )
        .child(body)
}

fn row() -> Div {
    div()
        .h(px(44.))
        .px(px(10.))
        .rounded(px(9.))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(20.))
}

fn label(p: Palette, name: &str, hint: &str, hot: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(
            div()
                .text_size(px(13.5))
                .text_color(if hot { p.accent } else { p.ink })
                .child(name.to_string()),
        )
        .child(
            div()
                .text_size(px(10.5))
                .text_color(p.ash)
                .child(hint.to_string()),
        )
}

fn dot(p: Palette, on: bool) -> impl IntoElement {
    div()
        .size(px(16.))
        .flex_shrink_0()
        .rounded(px(999.))
        .border_1()
        .border_color(if on { p.accent } else { p.hairline })
        .flex()
        .items_center()
        .justify_center()
        .when(on, |el| {
            el.child(div().size(px(8.)).rounded(px(999.)).bg(p.accent))
        })
}

fn chip(
    p: Palette,
    id: impl Into<gpui::ElementId>,
    text: &str,
    active: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(30.))
        .px(px(13.))
        .rounded(px(999.))
        .flex()
        .items_center()
        .when(active, |el| el.bg(p.ink))
        .when(!active, |el| {
            el.bg(p.card).border_1().border_color(p.hairline)
        })
        .hover(|el| el.opacity(0.88))
        .child(
            div()
                .text_size(px(11.5))
                .text_color(if active { p.canvas } else { p.mute })
                .child(text.to_string()),
        )
        .on_click(on_click)
}

#[cfg(test)]
mod tests {
    use super::normalize_quality;

    #[test]
    fn quality_aliases_fold_to_the_four_tags() {
        assert_eq!(normalize_quality("flac"), Some("flac"));
        assert_eq!(normalize_quality("master"), Some("flac"));
        assert_eq!(normalize_quality(" opus "), Some("opus128"));
        assert_eq!(normalize_quality("OPUS128"), Some("opus128"));
        assert_eq!(normalize_quality("opus160"), Some("opus160"));
        assert_eq!(normalize_quality("opus96"), Some("opus96"));
        assert_eq!(normalize_quality("mp3"), None);
    }
}
