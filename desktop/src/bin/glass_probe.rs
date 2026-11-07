use gpui::{
    App, AppContext as _, Application, Bounds, Context, Hsla, IntoElement, Render, Window,
    WindowBackgroundAppearance, WindowBounds, WindowDecorations, WindowOptions, div, hsla,
    prelude::*, px, rgba, size,
};

struct GlassProbe {
    bg: Hsla,
    /// Stripes mode paints only high-frequency black/white 32px stripes: used as a
    /// backdrop window to detect compositor blur.
    stripes: bool,
}

impl Render for GlassProbe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.stripes {
            return div()
                .size_full()
                .flex()
                .flex_row()
                .children((0..40).map(|i| {
                    div().w(px(32.0)).h_full().bg(if i % 2 == 0 {
                        gpui::black()
                    } else {
                        gpui::white()
                    })
                }))
                .into_any_element();
        }
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(self.bg)
            .child(
                div()
                    .w(px(560.0))
                    .h(px(180.0))
                    .rounded(px(24.0))
                    .bg(rgba(0xffffff1f))
                    .border_1()
                    .border_color(rgba(0xffffff2e))
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_color(gpui::white())
                    .text_2xl()
                    .child("glass probe"),
            )
            .into_any_element()
    }
}

fn main() {
    let variant = match std::env::var("GLASS_BG").as_deref() {
        Ok("opaque") => WindowBackgroundAppearance::Opaque,
        Ok("transparent") => WindowBackgroundAppearance::Transparent,
        _ => WindowBackgroundAppearance::Blurred,
    };
    // h is clamped to 0..=1 in gpui's `hsla`, so 220 deg is 220/360.
    let bg = match std::env::var("GLASS_SOLID").as_deref() {
        Ok(rgb) => {
            let v: Vec<f32> = rgb.split(',').filter_map(|s| s.parse().ok()).collect();
            assert!(v.len() == 3, "GLASS_SOLID wants r,g,b");
            Hsla::from(rgba(
                ((v[0] as u32) << 24) | ((v[1] as u32) << 16) | ((v[2] as u32) << 8) | 0xff,
            ))
        }
        Err(_) => hsla(220.0 / 360.0, 0.3, 0.5, 0.35),
    };
    let stripes = std::env::var("GLASS_STRIPES").is_ok();
    let decorations = match std::env::var("GLASS_DECOR").as_deref() {
        Ok("server") => WindowDecorations::Server,
        _ => WindowDecorations::Client,
    };
    let bg = match std::env::var("GLASS_ALPHA") {
        Ok(a) => gpui::Hsla {
            a: a.parse().expect("GLASS_ALPHA wants f32"),
            ..bg
        },
        Err(_) => bg,
    };
    println!(
        "glass_probe: window_background = {variant:?}, decorations = {decorations:?}, stripes = {stripes}, bg = {bg:?} -> {:?}",
        gpui::Rgba::from(bg)
    );
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some("glass-probe".into()),
                window_decorations: Some(decorations),
                window_background: variant,
                ..Default::default()
            },
            |_, cx| cx.new(|_| GlassProbe { bg, stripes }),
        )
        .expect("glass_probe: open_window failed");
        println!("glass_probe: window opened");
        cx.activate(true);
    });
}
