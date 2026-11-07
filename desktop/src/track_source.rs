//! What the server sends is not always what the file says it is: the owner set
//! the box to Opus, so every stream is an Ogg Opus file no matter what the
//! master is. `rodio` has no Opus decoder, so this module supplies one — and a
//! source that plays either kind from the same path.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use opus_pure::{OggOpusReader, OpusDecoder};
use rodio::{Decoder, Source};

const OPUS_RATE: u32 = 48_000;
const MAX_FRAME: usize = 5_760; // 120 ms, the longest Opus frame

pub enum TrackSource {
    Decoded(Decoder<BufReader<File>>),
    Opus(OpusSource),
}

/// Opens any file the server can hand out: flac, mp3, wav or Ogg Opus.
pub fn open(path: &Path, total: Duration) -> anyhow::Result<TrackSource> {
    let mut probe = File::open(path)?;
    let mut magic = [0u8; 4];
    let n = probe.read(&mut magic)?;
    if n == 4 && &magic == b"OggS" {
        return Ok(TrackSource::Opus(OpusSource::new(path, total)?));
    }
    let file = File::open(path)?;
    Ok(TrackSource::Decoded(Decoder::try_from(file)?))
}

pub struct OpusSource {
    path: std::path::PathBuf,
    reader: OggOpusReader<BufReader<File>>,
    decoder: OpusDecoder,
    channels: u16,
    pre_skip: usize,
    buf: Vec<f32>,
    cursor: usize,
    total: Duration,
    done: bool,
}

impl OpusSource {
    fn new(path: &Path, total: Duration) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = OggOpusReader::new(BufReader::new(file))?;
        let channels = reader.head().channel_count.max(1) as u16;
        let pre_skip = reader.head().pre_skip as usize;
        let decoder = OpusDecoder::new(OPUS_RATE as i32, channels as usize)?;
        Ok(Self {
            path: path.to_path_buf(),
            reader,
            decoder,
            channels,
            pre_skip,
            buf: Vec::new(),
            cursor: 0,
            total,
            done: false,
        })
    }

    fn fill(&mut self) -> bool {
        while !self.done {
            let packet = match self.reader.read_packet() {
                Ok(Some(packet)) => packet,
                _ => {
                    self.done = true;
                    return false;
                }
            };
            let mut out = vec![0.0f32; MAX_FRAME * self.channels as usize];
            let per_channel = match self.decoder.decode(&packet.data, MAX_FRAME, &mut out) {
                Ok(n) => n,
                Err(_) => continue,
            };
            out.truncate(per_channel * self.channels as usize);
            let mut start = 0;
            if self.pre_skip > 0 {
                let samples = (self.pre_skip * self.channels as usize).min(out.len());
                start = samples;
                self.pre_skip -= samples / self.channels as usize;
            }
            self.buf.clear();
            self.buf.extend_from_slice(&out[start..]);
            self.cursor = 0;
            if !self.buf.is_empty() {
                return true;
            }
        }
        false
    }

    /// Opus has no cheap random access here: the container is read from the
    /// top and packets are decoded to be discarded. A seek costs about a second
    /// per four minutes skipped, on the audio thread. ponytail: page-granule
    /// bisection if seeking ever feels slow.
    fn seek_to(&mut self, to: Duration) -> anyhow::Result<()> {
        let file = File::open(&self.path)?;
        let mut file = file;
        file.seek(SeekFrom::Start(0))?;
        self.reader = OggOpusReader::new(BufReader::new(file))?;
        self.decoder.reset_state()?;
        self.pre_skip = self.reader.head().pre_skip as usize;
        self.buf.clear();
        self.cursor = 0;
        self.done = false;

        let mut left = (to.as_secs_f64() * OPUS_RATE as f64) as usize;
        let mut scratch = vec![0.0f32; MAX_FRAME * self.channels as usize];
        while left > 0 {
            let packet = match self.reader.read_packet() {
                Ok(Some(packet)) => packet,
                _ => break,
            };
            let per_channel = match self.decoder.decode(&packet.data, MAX_FRAME, &mut scratch) {
                Ok(n) => n,
                Err(_) => continue,
            };
            let skip = self.pre_skip.min(per_channel);
            self.pre_skip -= skip;
            let usable = per_channel - skip;
            if usable >= left {
                let offset = (left + skip) * self.channels as usize;
                let len = (usable - left) * self.channels as usize;
                self.buf.clear();
                self.buf
                    .extend_from_slice(&scratch[offset..offset + len]);
                self.cursor = 0;
                return Ok(());
            }
            left -= usable;
        }
        Ok(())
    }
}

impl Iterator for TrackSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        match self {
            TrackSource::Decoded(d) => d.next(),
            TrackSource::Opus(o) => {
                if o.cursor >= o.buf.len() && !o.fill() {
                    return None;
                }
                let sample = o.buf.get(o.cursor).copied();
                o.cursor += 1;
                sample
            }
        }
    }
}

impl Source for TrackSource {
    fn current_span_len(&self) -> Option<usize> {
        match self {
            TrackSource::Decoded(d) => d.current_span_len(),
            TrackSource::Opus(o) => Some(o.buf.len().saturating_sub(o.cursor)),
        }
    }

    fn channels(&self) -> rodio::ChannelCount {
        match self {
            TrackSource::Decoded(d) => d.channels(),
            TrackSource::Opus(o) => std::num::NonZeroU16::new(o.channels).unwrap(),
        }
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        match self {
            TrackSource::Decoded(d) => d.sample_rate(),
            TrackSource::Opus(_) => std::num::NonZeroU32::new(OPUS_RATE).expect("48 kHz"),
        }
    }

    fn total_duration(&self) -> Option<Duration> {
        match self {
            TrackSource::Decoded(d) => d.total_duration(),
            TrackSource::Opus(o) => Some(o.total),
        }
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        match self {
            TrackSource::Decoded(d) => d.try_seek(pos),
            TrackSource::Opus(o) => o
                .seek_to(pos)
                .map_err(|_| rodio::source::SeekError::NotSupported {
                    underlying_source: std::any::type_name::<OpusSource>(),
                }),
        }
    }
}
