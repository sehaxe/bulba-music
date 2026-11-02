use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Derived files: an Opus copy for phones on a metered connection and a small
/// cover for every list. Both are generated on demand, keyed by the master's
/// path and mtime, and thrown away when the source changes.
#[derive(Clone, Debug)]
pub struct Cache {
    root: Arc<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Opus96,
    Opus128,
    Opus160,
}

impl Quality {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "opus96" | "opus_96" => Some(Self::Opus96),
            "opus128" | "opus_128" | "opus" => Some(Self::Opus128),
            "opus160" | "opus_160" => Some(Self::Opus160),
            _ => None,
        }
    }

    pub fn bitrate(self) -> u32 {
        match self {
            Self::Opus96 => 96,
            Self::Opus128 => 128,
            Self::Opus160 => 160,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::Opus96 => "opus96",
            Self::Opus128 => "opus128",
            Self::Opus160 => "opus160",
        }
    }
}

fn fingerprint(path: &Path, mtime: i64) -> String {
    // Not a content hash: the point is to notice that this file changed, and
    // path+mtime does that for free on every filesystem we care about.
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in path.to_string_lossy().as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= mtime as u64;
    format!("{h:016x}")
}

impl Cache {
    pub fn new(root: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&root);
        let _ = std::fs::create_dir_all(root.join("audio"));
        let _ = std::fs::create_dir_all(root.join("covers"));
        Self { root: Arc::new(root) }
    }

    /// The Opus copy of a track, encoding it on first use.
    pub fn opus(&self, source: &Path, mtime: i64, quality: Quality) -> Result<PathBuf> {
        let key = fingerprint(source, mtime);
        let out = self
            .root
            .join("audio")
            .join(format!("{key}-{}.opus", quality.tag()));
        if out.exists() && std::fs::metadata(&out)?.len() > 0 {
            return Ok(out);
        }
        // Encode to a temporary name and rename: a half-written file would
        // otherwise be served as a corrupt track.
        let tmp = out.with_extension("part");
        let status = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(source)
            .args([
                "-vn",
                "-c:a",
                "libopus",
                "-b:a",
                &format!("{}k", quality.bitrate()),
                "-vbr",
                "on",
                "-compression_level",
                "10",
                "-f",
                "opus",
            ])
            .arg(&tmp)
            .status()
            .with_context(|| "ffmpeg not found — install it to stream Opus")?;
        if !status.success() {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("ffmpeg failed on {}", source.display());
        }
        std::fs::rename(&tmp, &out)?;
        Ok(out)
    }

    /// A cover scaled to `width`. WebP by default: every client that can show
    /// this app can decode it, and it is ~40 % smaller than JPEG at the same
    /// visible quality.
    pub fn cover(&self, source: &Path, width: u32, format: &str) -> Result<PathBuf> {
        let mtime = std::fs::metadata(source)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let key = fingerprint(source, mtime);
        let ext = match format {
            "jpeg" | "jpg" => "jpg",
            "png" => "png",
            "jxl" => "jxl",
            _ => "webp",
        };
        let out = self
            .root
            .join("covers")
            .join(format!("{key}-{width}.{ext}"));
        if out.exists() && std::fs::metadata(&out)?.len() > 0 {
            return Ok(out);
        }
        let tmp = out.with_extension("part");
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(source)
            .args(["-vf", &format!("scale={width}:-2:flags=lanczos"), "-frames:v", "1"]);
        // The temporary file ends in .part, so ffmpeg cannot infer the muxer
        // from the name and has to be told explicitly.
        match ext {
            "webp" => {
                cmd.args(["-c:v", "libwebp", "-quality", "82", "-f", "webp"]);
            }
            "jpg" => {
                cmd.args(["-c:v", "mjpeg", "-q:v", "4", "-f", "image2"]);
            }
            // ~11% smaller than webp q82 on a real cover and visibly cleaner,
            // but no Flutter client can decode it without shipping libjxl:
            // this is for web clients and for the record, not the phone.
            "jxl" => {
                cmd.args(["-c:v", "libjxl", "-quality", "90", "-f", "image2"]);
            }
            _ => {
                cmd.args(["-c:v", "png", "-f", "image2"]);
            }
        }
        let status = cmd.arg(&tmp).status()?;
        if !status.success() {
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("cover conversion failed for {}", source.display());
        }
        std::fs::rename(&tmp, &out)?;
        Ok(out)
    }

    /// Bytes on disk, so the settings page can show what the cache costs.
    pub fn size(&self) -> u64 {
        fn walk(dir: &Path) -> u64 {
            let mut total = 0;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    let path = e.path();
                    if path.is_dir() {
                        total += walk(&path);
                    } else if let Ok(md) = e.metadata() {
                        total += md.len();
                    }
                }
            }
            total
        }
        walk(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_parsing_is_forgiving_about_names() {
        assert_eq!(Quality::parse("opus"), Some(Quality::Opus128));
        assert_eq!(Quality::parse("opus_96"), Some(Quality::Opus96));
        assert_eq!(Quality::parse("opus160"), Some(Quality::Opus160));
        assert_eq!(Quality::parse("flac"), None);
    }

    #[test]
    fn the_key_changes_when_the_file_changes() {
        let a = fingerprint(Path::new("/music/a.flac"), 1);
        let b = fingerprint(Path::new("/music/a.flac"), 2);
        let c = fingerprint(Path::new("/music/b.flac"), 1);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }
}
