use anyhow::Result;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::process::Command;

/// Content fingerprint used by the first recommender. Deliberately model-free:
/// it is computed in under a second per track, needs no download, and gives the
/// `Distance` seam something honest to be beaten by later.
///
/// Layout: 40 log-mel bands, 12 chroma bins, 3 spectral shape stats, 1 tempo.
/// The vector is L2-normalised, so similarity is a dot product.
pub const MODEL: &str = "dsp-v1";
pub const DIM: usize = 56;

const SAMPLE_RATE: u32 = 22050;
const N_FFT: usize = 2048;
const HOP: usize = 1024;
const MEL_BANDS: usize = 40;
const MIN_BPM: f32 = 55.0;
const MAX_BPM: f32 = 200.0;

pub struct Fingerprint {
    pub vec: Vec<f32>,
    pub tempo_bpm: f32,
    pub brightness: f32,
}

pub fn extract(path: &str) -> Result<Fingerprint> {
    let pcm = decode(path)?;
    let (vec, tempo_bpm, brightness) = embed(&pcm);
    Ok(Fingerprint {
        vec,
        tempo_bpm,
        brightness,
    })
}

fn decode(path: &str) -> Result<Vec<f32>> {
    let out = Command::new("ffmpeg")
        .args([
            "-v", "error", "-i", path, "-ac", "1", "-ar", &SAMPLE_RATE.to_string(), "-f",
            "f32le", "-",
        ])
        .output()?;
    Ok(out
        .stdout
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

fn mel_filterbank() -> Vec<Vec<f32>> {
    let nyquist = SAMPLE_RATE as f32 / 2.0;
    let low = hz_to_mel(30.0);
    let high = hz_to_mel(nyquist);
    let points: Vec<f32> = (0..MEL_BANDS + 2)
        .map(|i| {
            let mel = low + (high - low) * i as f32 / (MEL_BANDS + 1) as f32;
            mel_to_hz(mel) * N_FFT as f32 / SAMPLE_RATE as f32
        })
        .collect();
    (0..MEL_BANDS)
        .map(|band| {
            let left = points[band];
            let centre = points[band + 1];
            let right = points[band + 2];
            (0..N_FFT / 2 + 1)
                .map(|bin| {
                    let f = bin as f32;
                    if f <= left || f >= right {
                        0.0
                    } else if f <= centre {
                        (f - left) / (centre - left).max(1e-6)
                    } else {
                        (right - f) / (right - centre).max(1e-6)
                    }
                })
                .collect()
        })
        .collect()
}

/// 12 pitch classes; bins below 55 Hz are ignored because they carry rumble.
fn chroma_map() -> Vec<Option<usize>> {
    (0..N_FFT / 2 + 1)
        .map(|bin| {
            let hz = bin as f32 * SAMPLE_RATE as f32 / N_FFT as f32;
            if !(55.0..=5000.0).contains(&hz) {
                return None;
            }
            let midi = 69.0 + 12.0 * (hz / 440.0).log2();
            Some((midi.round() as i32).rem_euclid(12) as usize)
        })
        .collect()
}

fn embed(pcm: &[f32]) -> (Vec<f32>, f32, f32) {
    let mut out = vec![0.0f32; DIM];
    if pcm.len() < N_FFT {
        return (out, 0.0, 0.0);
    }

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N_FFT);
    let window: Vec<f32> = (0..N_FFT)
        .map(|i| {
            0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N_FFT - 1) as f32).cos()
        })
        .collect();
    let mel = mel_filterbank();
    let chroma = chroma_map();

    let mut mel_acc = vec![0.0f64; MEL_BANDS];
    let mut chroma_acc = vec![0.0f64; 12];
    let mut centroid = 0.0f64;
    let mut flatness = 0.0f64;
    let mut rolloff = 0.0f64;
    let mut frames = 0usize;
    let mut onsets: Vec<f32> = Vec::new();
    let mut previous: Option<Vec<f32>> = None;

    let mut buffer = vec![Complex::new(0.0f32, 0.0); N_FFT];
    let mut start = 0;
    while start + N_FFT <= pcm.len() {
        for i in 0..N_FFT {
            buffer[i] = Complex::new(pcm[start + i] * window[i], 0.0);
        }
        fft.process(&mut buffer);

        let mags: Vec<f32> = buffer[..N_FFT / 2 + 1].iter().map(|c| c.norm()).collect();

        for (band, weights) in mel.iter().enumerate() {
            let energy: f32 = weights
                .iter()
                .zip(mags.iter())
                .map(|(w, m)| w * m * m)
                .sum();
            mel_acc[band] += (energy + 1e-10).ln() as f64;
        }
        for (bin, pitch) in chroma.iter().enumerate() {
            if let Some(class) = pitch {
                chroma_acc[*class] += mags[bin] as f64;
            }
        }

        let total: f32 = mags.iter().sum();
        if total > 1e-9 {
            let mean = total / mags.len() as f32;
            let geo = (mags.iter().map(|m| (m + 1e-10).ln()).sum::<f32>()
                / mags.len() as f32)
                .exp();
            centroid += mags
                .iter()
                .enumerate()
                .map(|(i, m)| i as f32 * m)
                .sum::<f32>() as f64
                / total as f64
                / (N_FFT / 2) as f64;
            flatness += (geo / mean) as f64;
            let mut acc = 0.0;
            let mut edge = 0;
            for (i, m) in mags.iter().enumerate() {
                acc += m;
                if acc >= total * 0.85 {
                    edge = i;
                    break;
                }
            }
            rolloff += edge as f64 / (N_FFT / 2) as f64;
        }

        // Spectral flux as the onset envelope.
        let flux: f32 = match &previous {
            None => 0.0,
            Some(prev) => mags
                .iter()
                .zip(prev.iter())
                .map(|(a, b)| (a - b).max(0.0))
                .sum(),
        };
        onsets.push(flux);
        previous = Some(mags);

        frames += 1;
        start += HOP;
    }

    if frames == 0 {
        return (out, 0.0, 0.0);
    }

    // mel: mean log energy, centred so loudness differences do not dominate.
    let mean_log = mel_acc.iter().sum::<f64>() / MEL_BANDS as f64;
    for i in 0..MEL_BANDS {
        out[i] = ((mel_acc[i] / frames as f64) - mean_log) as f32;
    }

    let chroma_sum: f64 = chroma_acc.iter().sum();
    let chroma_scale = if chroma_sum > 1e-9 { chroma_sum } else { 1.0 };
    for i in 0..12 {
        out[MEL_BANDS + i] = (chroma_acc[i] / chroma_scale) as f32;
    }

    let brightness = (centroid / frames as f64) as f32;
    let tempo_bpm = tempo_from_onsets(&onsets);
    out[52] = brightness;
    out[53] = (flatness / frames as f64) as f32;
    out[54] = (rolloff / frames as f64) as f32;
    out[55] = tempo_bpm / MAX_BPM;

    // Per-block weighting, then one L2 normalisation.
    for i in 0..MEL_BANDS {
        out[i] *= 1.0;
    }
    for i in MEL_BANDS..MEL_BANDS + 12 {
        out[i] *= 0.8;
    }
    for i in 52..55 {
        out[i] *= 0.5;
    }
    out[55] *= 0.7;

    let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 1e-9 {
        for v in out.iter_mut() {
            *v /= norm;
        }
    }
    (out, tempo_bpm, brightness)
}

/// Autocorrelation of the spectral-flux envelope: the lag of the strongest
/// peak in the 55–200 BPM range wins. This is the classic, cheap tempo estimate
/// and it is good enough to keep a radio from jumping between 70 and 160 BPM.
fn tempo_from_onsets(onsets: &[f32]) -> f32 {
    if onsets.len() < 32 {
        return 0.0;
    }
    let mean = onsets.iter().sum::<f32>() / onsets.len() as f32;
    let centred: Vec<f32> = onsets.iter().map(|v| v - mean).collect();
    let min_lag = (SAMPLE_RATE as f32 / HOP as f32 * 60.0 / MAX_BPM) as usize;
    let max_lag = (SAMPLE_RATE as f32 / HOP as f32 * 60.0 / MIN_BPM) as usize;

    let mut best_lag = 0usize;
    let mut best = 0.0f32;
    for lag in min_lag..=max_lag.max(min_lag + 1) {
        if lag >= centred.len() {
            break;
        }
        let mut acc = 0.0f32;
        for i in 0..centred.len() - lag {
            acc += centred[i] * centred[i + lag];
        }
        if acc > best {
            best = acc;
            best_lag = lag;
        }
    }
    if best_lag == 0 {
        return 0.0;
    }
    let seconds_per_frame = HOP as f32 / SAMPLE_RATE as f32;
    60.0 / (best_lag as f32 * seconds_per_frame)
}

/// Cosine similarity of two L2-normalised vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn from_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, seconds: f32) -> Vec<f32> {
        let n = (SAMPLE_RATE as f32 * seconds) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / SAMPLE_RATE as f32).sin())
            .collect()
    }

    #[test]
    fn chroma_identifies_the_pitch_class() {
        // 440 Hz is A = pitch class 9.
        let (v, _, _) = embed(&sine(440.0, 4.0));
        let chroma = &v[MEL_BANDS..MEL_BANDS + 12];
        let best = chroma
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(best, 9, "chroma={chroma:?}");
    }

    #[test]
    fn vector_is_normalised_and_stable() {
        let (a, _, _) = embed(&sine(220.0, 3.0));
        let (b, _, _) = embed(&sine(220.0, 3.0));
        assert!((a.iter().map(|x| x * x).sum::<f32>() - 1.0).abs() < 1e-4);
        assert!(cosine(&a, &b) > 0.999);
    }

    #[test]
    fn different_pitches_are_less_similar_than_identical_ones() {
        let (a, _, _) = embed(&sine(220.0, 3.0));
        let (b, _, _) = embed(&sine(330.0, 3.0));
        let (c, _, _) = embed(&sine(220.0, 3.0));
        assert!(cosine(&a, &b) < cosine(&a, &c));
    }

    #[test]
    fn blob_roundtrip() {
        let (v, _, _) = embed(&sine(500.0, 2.0));
        let back = from_blob(&to_blob(&v));
        assert_eq!(v, back);
    }

    #[test]
    fn silence_does_not_panic_or_nan() {
        let (v, _, _) = embed(&vec![0.0; SAMPLE_RATE as usize]);
        assert_eq!(v.len(), DIM);
        assert!(v.iter().all(|x| x.is_finite()));
    }
}
