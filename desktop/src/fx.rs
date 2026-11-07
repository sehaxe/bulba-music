//! GPU-free visual effects.
//!
//! GPUI 0.2.2 cannot run custom shaders, so each effect is a small raster
//! texture the caller stretches over its area: grain to hide gradient banding,
//! a radial glow behind the cover, a vignette for the window edges and a dotted
//! overlay for glass panels.

use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};
use smallvec::SmallVec;

/// RGBA -> BGRA, then into the one-frame image GPUI uploads.
fn to_render(img: RgbaImage) -> Arc<RenderImage> {
    let mut data = img;
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2); // GPUI wants BGRA
    }
    Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(data), 1)))
}

/// xorshift64: deterministic, fast, and one struct instead of a dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1) // never all-zero
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 56) as u8
    }
}

/// Monochrome noise with an alpha channel, tiled by the caller. `strength` 0..1.
pub fn grain(size: u32, strength: f32, seed: u64) -> Arc<RenderImage> {
    let size = size.max(1);
    let strength = strength.clamp(0.0, 1.0);
    let mut rng = Rng::new(seed);
    let mut img = RgbaImage::new(size, size);
    for px in img.pixels_mut() {
        let grey = rng.byte();
        // neutral grey (mean 127.5) so it sits on top of any colour
        let alpha = (strength * rng.byte() as f32).round() as u8;
        *px = image::Rgba([grey, grey, grey, alpha]);
    }
    to_render(img)
}

/// A radial falloff: opaque at the centre, transparent at the edge, in the
/// given hue/saturation. `size` is the texture edge in pixels (power of two
/// recommended, e.g. 512).
pub fn glow(size: u32, hue: f32, sat: f32, light: f32) -> Arc<RenderImage> {
    let size = size.max(1);
    let centre = size as f32 / 2.0;
    let (r, g, b) = hsl_to_rgb(hue, sat, light);
    let mut img = RgbaImage::new(size, size);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let dx = x as f32 + 0.5 - centre;
        let dy = y as f32 + 0.5 - centre;
        let radius = ((dx * dx + dy * dy).sqrt() / centre).min(1.0);
        let alpha = (1.0 - radius).powf(2.2);
        *px = image::Rgba([r, g, b, (alpha * 255.0).round() as u8]);
    }
    to_render(img)
}

/// A rectangular vignette: transparent centre, dark edges. `size` 512..
pub fn vignette(size: u32, darkness: f32) -> Arc<RenderImage> {
    let size = size.max(1);
    let half = size as f32 / 2.0;
    let max_radius = half * std::f32::consts::SQRT_2;
    let darkness = darkness.clamp(0.0, 1.0);
    let mut img = RgbaImage::new(size, size);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let dx = x as f32 + 0.5 - half;
        let dy = y as f32 + 0.5 - half;
        let radius = ((dx * dx + dy * dy).sqrt() / max_radius).min(1.0);
        let alpha = darkness * radius.powf(1.8);
        *px = image::Rgba([0, 0, 0, (alpha * 255.0).round() as u8]);
    }
    to_render(img)
}

/// Small repeating dot pattern used as a subtle overlay on glass panels, the
/// kind of texture a designer would call "noise". 64x64, alpha only.
pub fn dither(size: u32, strength: f32) -> Arc<RenderImage> {
    let size = size.max(1);
    // Split the mean between two alphas: floor/ceil average back to `strength`.
    let value = strength.clamp(0.0, 1.0) * 255.0;
    let lo = value.floor() as u8;
    let hi = value.ceil() as u8;
    let mut img = RgbaImage::new(size, size);
    for (x, y, px) in img.enumerate_pixels_mut() {
        let alpha = if (x + y) % 2 == 0 { hi } else { lo };
        *px = image::Rgba([255, 255, 255, alpha]);
    }
    to_render(img)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s <= 0.0 {
        let v = (l * 255.0) as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v.clamp(0.0, 1.0) * 255.0) as u8
    };
    (f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(img: &RenderImage, size: u32, x: u32, y: u32) -> u8 {
        img.as_bytes(0).unwrap()[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn dimensions_match() {
        for (img, size) in [
            (grain(32, 0.2, 1), 32),
            (glow(64, 210.0, 0.5, 0.5), 64),
            (vignette(64, 0.6), 64),
            (dither(64, 0.1), 64),
        ] {
            let got = img.size(0);
            assert_eq!((got.width.0, got.height.0), (size as i32, size as i32));
        }
    }

    #[test]
    fn alpha_within_bounds() {
        let grain_max = grain(64, 0.35, 7)
            .as_bytes(0)
            .unwrap()
            .chunks_exact(4)
            .map(|p| p[3])
            .max()
            .unwrap();
        assert!(grain_max <= (0.35 * 255.0) as u8 + 1);

        let vignette_max = vignette(64, 0.7)
            .as_bytes(0)
            .unwrap()
            .iter()
            .skip(3)
            .step_by(4)
            .copied()
            .max()
            .unwrap();
        assert!(vignette_max <= (0.7 * 255.0) as u8 + 1);

        let bytes = dither(64, 0.25).as_bytes(0).unwrap().to_vec();
        let mean = bytes.chunks_exact(4).map(|p| p[3] as f32).sum::<f32>() / (64.0 * 64.0);
        assert!((mean - 0.25 * 255.0).abs() <= 1.0);
    }

    #[test]
    fn glow_falls_off() {
        let img = glow(64, 40.0, 0.6, 0.5);
        assert!(alpha(&img, 64, 32, 32) as f32 / 255.0 > 0.9);
        assert!(alpha(&img, 64, 0, 0) as f32 / 255.0 < 0.05);
    }
}
