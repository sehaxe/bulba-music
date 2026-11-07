mod api;
mod app;
mod audio;
mod covers;
pub mod fx;
mod mpris;
pub mod search;
mod theme;
mod track_source;
pub mod wave;

use std::borrow::Cow;

use gpui::{
    App, Application, AppContext as _, WindowBackgroundAppearance, AssetSource, Bounds, SharedString, TitlebarOptions, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};

/// Icons ship next to the binary in a release build and inside the crate in a
/// dev build; both are the same relative path.
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let mut roots = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))];
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            roots.push(dir.to_path_buf());
            roots.push(dir.join("../share/bulba-desktop"));
        }
        for root in roots {
            if let Ok(bytes) = std::fs::read(root.join(path)) {
                return Ok(Some(bytes.into()));
            }
        }
        Ok(None)
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        let base = std::env::var("BULBA_SERVER").unwrap_or_else(|_| api::DEFAULT_SERVER.to_string());
        let api = api::Api::new(base);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(100.), px(70.)),
                size(px(1380.), px(900.)),
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("bulba music".into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            // Server decorations: niri draws its focus ring as a solid slab
            // behind a client-decorated window, which would hide the desktop we
            // are trying to show. It paints no titlebar, so this looks frameless.
            window_decorations: Some(WindowDecorations::Server),
            // The compositor blurs what is behind the window; everything drawn
            // on top is translucent glass.
            window_background: WindowBackgroundAppearance::Blurred,
            app_id: Some("bulba-music".into()),
            window_min_size: Some(size(px(980.), px(620.))),
            ..Default::default()
        };
        cx.open_window(options, |_window, cx| {
            cx.new(|cx| app::Root::new(api.clone(), cx))
        })
        .expect("не открылось окно");
        cx.activate(true);
    });
}
