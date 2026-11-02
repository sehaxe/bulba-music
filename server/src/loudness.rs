//! Integrated loudness, the number ReplayGain needs.
//!
//! ffmpeg already ships the exact filter EBU R128 defines (`ebur128`), so the
//! honest implementation is to ask it instead of writing K-weighting by hand and
//! hoping the gating matches. One decode pass per track at import.

use anyhow::Result;
use std::process::Command;

/// Integrated loudness in LUFS. `None` when ffmpeg has nothing to say — a
/// silent file or a broken decoder is not a reason to fail an import.
pub fn lufs(path: &str) -> Result<Option<f64>> {
    let out = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostats",
            "-i",
            path,
            "-af",
            "ebur128=peak=true",
            "-f",
            "null",
            "-",
        ])
        .output()?;
    let text = String::from_utf8_lossy(&out.stderr);
    // The summary block prints "    I:         -14.2 LUFS" once, at the end.
    let mut value = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("I:") {
            if let Some(num) = rest.split_whitespace().next() {
                if let Ok(v) = num.parse::<f64>() {
                    value = Some(v);
                }
            }
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_file_has_no_integrated_loudness() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("silence.wav");
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=mono",
                "-t",
                "2",
            ])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        let v = lufs(path.to_str().unwrap()).unwrap();
        assert!(v.is_none() || v.unwrap() < -60.0, "silence read as {v:?}");
    }

    #[test]
    fn twenty_db_quieter_reads_twenty_lu_lower() {
        let dir = tempfile::tempdir().unwrap();
        let tone = |gain: &str, name: &str| -> f64 {
            let path = dir.path().join(name);
            let ok = Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "sine=frequency=1000:sample_rate=48000:duration=3",
                    "-af",
                    &format!("volume={gain}"),
                ])
                .arg(&path)
                .status()
                .unwrap()
                .success();
            assert!(ok);
            lufs(path.to_str().unwrap()).unwrap().expect("a tone has a level")
        };
        // The absolute level of ffmpeg's sine is an implementation detail; the
        // scale is not. This is the property ReplayGain relies on.
        let loud = tone("0dB", "loud.wav");
        let quiet = tone("-20dB", "quiet.wav");
        assert!(
            (loud - quiet - 20.0).abs() < 0.5,
            "expected 20 LU between the two, got {:.1}",
            loud - quiet
        );
    }
}
