//! Album art → three things the UI needs: a sharp cover, a heavily blurred
//! "room" made from the same cover, and the hue that lights the interface.
//!
//! The blur is done here rather than by the compositor on purpose: niri has no
//! backdrop blur, and a pre-blurred 32 px image upscaled looks the same on every
//! machine, costs nothing per frame and never flickers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage, imageops::FilterType};
use smallvec::SmallVec;

use crate::api::Api;

pub struct Art {
    pub cover: Arc<RenderImage>,
    pub backdrop: Arc<RenderImage>,
    pub hue: f32,
    pub sat: f32,
    pub lum: f32,
}

pub struct Covers {
    pub cache: HashMap<i64, Arc<Art>>,
    pending: HashSet<i64>,
}

impl Covers {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            pending: HashSet::new(),
        }
    }

    pub fn get(&self, album_id: i64) -> Option<Arc<Art>> {
        self.cache.get(&album_id).cloned()
    }

    pub fn is_pending(&self, album_id: i64) -> bool {
        self.pending.contains(&album_id)
    }

    pub fn begin(&mut self, album_id: i64) -> bool {
        if self.cache.contains_key(&album_id) || self.pending.contains(&album_id) {
            return false;
        }
        self.pending.insert(album_id);
        true
    }

    pub fn finish(&mut self, album_id: i64, art: Option<Art>) {
        self.pending.remove(&album_id);
        if let Some(art) = art {
            self.cache.insert(album_id, Arc::new(art));
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.pending.clear();
    }
}

impl Default for Covers {
    fn default() -> Self {
        Self::new()
    }
}

/// Fetches and prepares one cover. Runs on a background thread.
///
/// JPEG XL first: it is the only format here that keeps a 768 px cover under
/// 80 KB without looking like a JPEG, and the desktop can decode it. The server
/// falls back to its own encoder if it cannot make one, and webp is the safety
/// net.
pub fn load(api: &Api, album_id: i64, title: &str) -> Option<Art> {
    match api.get_bytes(&format!("/api/albums/{album_id}/cover?w=512&fmt=jxl")) {
        Ok(bytes) => from_bytes(&bytes).or_else(|| {
            api.get_bytes(&format!("/api/albums/{album_id}/cover?w=512"))
                .ok()
                .and_then(|webp| from_bytes(&webp))
        }),
        Err(e) => {
            if std::env::var("BULBA_COVER_LOG").is_ok() {
                eprintln!("[cover] {album_id}: {e}");
            }
            Some(placeholder(title))
        }
    }
}

fn is_jxl(bytes: &[u8]) -> bool {
    (bytes.len() > 2 && bytes[0] == 0xFF && bytes[1] == 0x0A)
        || (bytes.len() > 12 && &bytes[0..12] == b"\x00\x00\x00\x0CJXL \x0D\x0A\x87\x0A")
}

/// Decodes either a JXL (via jxl-oxide) or anything the `image` crate knows.
fn decode(bytes: &[u8]) -> Option<RgbaImage> {
    if is_jxl(bytes) {
        let image = jxl_oxide::JxlImage::builder()
            .read(std::io::Cursor::new(bytes))
            .ok()?;
        let render = image.render_frame(0).ok()?;
        let mut stream = render.stream();
        let (w, h, ch) = (stream.width(), stream.height(), stream.channels());
        let mut buf = vec![0.0f32; (w * h * ch) as usize];
        stream.write_to_buffer(&mut buf);
        let mut out = RgbaImage::new(w, h);
        let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        for (i, px) in out.pixels_mut().enumerate() {
            let s = &buf[i * ch as usize..];
            *px = match ch {
                1 => image::Rgba([to8(s[0]), to8(s[0]), to8(s[0]), 255]),
                2 => image::Rgba([to8(s[0]), to8(s[0]), to8(s[0]), to8(s[1])]),
                3 => image::Rgba([to8(s[0]), to8(s[1]), to8(s[2]), 255]),
                _ => image::Rgba([to8(s[0]), to8(s[1]), to8(s[2]), to8(s[3])]),
            };
        }
        return Some(out);
    }
    image::load_from_memory(bytes).ok().map(|i| i.into_rgba8())
}

pub fn from_bytes(bytes: &[u8]) -> Option<Art> {
    let img = decode(bytes)?;
    let cover = to_render(&img);
    let (hue, sat, lum) = palette(&img);
    let backdrop = backdrop(&img, hue, sat);
    Some(Art {
        cover,
        backdrop,
        hue,
        sat,
        lum,
    })
}

/// A cover that does not exist still deserves a colour: the album's name picks
/// the hue, so the record looks deliberate instead of broken.
pub fn placeholder(title: &str) -> Art {
    let mut hash: u64 = 1469598103934665603;
    for b in title.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    let hue = (hash % 360) as f32;
    let (w, h) = (64usize, 64usize);
    let mut img = RgbaImage::new(w as u32, h as u32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let t = (x as f32 / w as f32) * 0.6 + (y as f32 / h as f32) * 0.4;
        let (r, g, b) = hsl_to_rgb(hue, 0.32, 0.22 + t * 0.16);
        *p = image::Rgba([r, g, b, 255]);
    }
    Art {
        cover: to_render(&img),
        backdrop: backdrop(&img, hue, 0.32),
        hue,
        sat: 0.32,
        lum: 0.3,
    }
}

fn to_render(img: &RgbaImage) -> Arc<RenderImage> {
    let mut data = img.clone();
    for px in data.chunks_exact_mut(4) {
        px.swap(0, 2); // GPUI wants BGRA
    }
    Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(data), 1)))
}

/// 32 px of colour, blown back up to 1024: smooth organic blobs with no banding
/// and no per-frame cost.
fn backdrop(img: &RgbaImage, hue: f32, sat: f32) -> Arc<RenderImage> {
    let small = image::imageops::resize(img, 32, 32, FilterType::Triangle);
    // 256 is enough: the GPU stretches it to the window and the scrim covers the
    // rest. A 1024² texture per album cost ~4 MB each and bought nothing.
    let mut big = image::imageops::resize(&small, 256, 256, FilterType::Lanczos3);
    // Push the colour harder than the cover has it: a blurred image reads as
    // grey unless the saturation is lifted, and the scrim over it eats the rest.
    let boost = 0.18 + sat * 0.35;
    for px in big.chunks_exact_mut(4) {
        let (h, s, l) = rgb_to_hsl(px[0], px[1], px[2]);
        let h = if s < 0.05 { hue } else { h };
        let (r, g, b) = hsl_to_rgb(h, (s * (1.0 + boost)).min(0.95), l * 0.82);
        px[0] = r;
        px[1] = g;
        px[2] = b;
        px[3] = 255;
    }
    to_render(&big)
}

fn palette(img: &RgbaImage) -> (f32, f32, f32) {
    let small = image::imageops::resize(img, 24, 24, FilterType::Triangle);
    let mut sx = 0.0f64;
    let mut sy = 0.0f64;
    let mut sw = 0.0f64;
    let mut sat = 0.0f64;
    let mut lum = 0.0f64;
    for px in small.chunks_exact(4) {
        let (h, s, l) = rgb_to_hsl(px[0], px[1], px[2]);
        // HSL lightness calls pure red "half bright", which would make every
        // saturated cover a light room. Perceptual luma is what the eye means.
        let y = (0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32) / 255.0;
        let weight = 0.15 + s as f64 * (1.0 - (l as f64 - 0.5).abs());
        let angle = (h as f64).to_radians();
        sx += angle.cos() * weight;
        sy += angle.sin() * weight;
        sw += weight;
        sat += s as f64 * weight;
        lum += y as f64;
    }
    let n = (24 * 24) as f64;
    if sw <= f64::EPSILON {
        return (210.0, 0.25, 0.5);
    }
    let hue = (sy.atan2(sx).to_degrees() + 360.0) % 360.0;
    (hue as f32, (sat / sw) as f32, (lum / n) as f32)
}

pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) * 60.0
    } else if max == g {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h, s, l)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
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
