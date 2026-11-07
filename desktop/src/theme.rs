//! The room: a palette that is lit by the record that is playing.
//!
//! Everything in the window is a shade of the current cover's dominant hue —
//! the canvas, the hairlines, the accent. When the music changes the room
//! changes with it, which is the whole visual idea of this client.

use gpui::{Background, Hsla, hsla, linear_color_stop, linear_gradient};

pub const FONT: &str = "Geist";

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub canvas: Hsla,
    pub ink: Hsla,
    pub body: Hsla,
    pub mute: Hsla,
    pub ash: Hsla,
    pub hairline: Hsla,
    pub hairline_soft: Hsla,
    pub card: Hsla,
    pub card_hover: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub shadow: Hsla,
    /// How much to brighten the faded backdrop.
    pub veil: f32,
}

impl Default for Palette {
    fn default() -> Self {
        Self::from_art(24.0, 0.35, 0.5)
    }
}

impl Palette {
    pub fn from_art(hue: f32, sat: f32, lum: f32) -> Self {
        let sat = sat.clamp(0.0, 1.0);
        // A grey cover must not produce a grey accent, and a neon one must not
        // produce an eye-searing panel. Both are clamped into a usable band.
        let chroma = sat.clamp(0.28, 1.0);
        let accent = hsla(hue, (chroma * 0.95).clamp(0.30, 0.92), 0.66, 1.0);
        Self {
            canvas: hsla(hue, (sat * 0.30).clamp(0.03, 0.16), 0.045, 1.0),
            ink: hsla(hue, 0.03, 0.975, 1.0),
            body: hsla(hue, 0.04, 0.86, 1.0),
            mute: hsla(hue, 0.04, 0.68, 1.0),
            ash: hsla(hue, 0.03, 0.50, 1.0),
            hairline: hsla(hue, 0.20, 1.0, 0.11),
            hairline_soft: hsla(hue, 0.20, 1.0, 0.06),
            card: hsla(hue, 0.22, 1.0, 0.045),
            card_hover: hsla(hue, 0.22, 1.0, 0.085),
            accent,
            accent_soft: accent.opacity(0.16),
            shadow: hsla(0.0, 0.0, 0.0, 0.55),
            veil: (lum - 0.35).clamp(-0.2, 0.3),
        }
    }

    /// The vertical wash that turns a blurred cover into a room: dark at the
    /// edges, a little lighter behind the record.
    pub fn scrim(&self) -> Background {
        linear_gradient(
            180.0,
            linear_color_stop(self.canvas.opacity(0.30), 0.0),
            linear_color_stop(self.canvas.opacity(0.97), 1.0),
        )
    }

    pub fn glow(&self) -> Background {
        linear_gradient(
            90.0,
            linear_color_stop(self.accent.opacity(0.0), 0.0),
            linear_color_stop(self.accent.opacity(0.10), 1.0),
        )
    }
}

/// A type scale small enough to remember.
pub mod type_ {
    pub const HERO: f32 = 68.0;
    pub const DISPLAY: f32 = 40.0;
    pub const TITLE: f32 = 26.0;
    pub const HEADING: f32 = 17.0;
    pub const BODY: f32 = 14.5;
    pub const SMALL: f32 = 12.5;
    pub const MICRO: f32 = 10.5;
}

/// Geometry. One rhythm, 4 px.
pub mod space {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
    pub const HUGE: f32 = 48.0;
}
