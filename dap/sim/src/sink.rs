//! `SimSink` -- a host test double for `soul_audio_core::Sink`.
//!
//! Declares a fixed device stream and records every write: a running,
//! interleaved copy of the PCM plus the per-write block boundaries, so a
//! caller can assert continuity (gaplessness) and level directly off real
//! output. Never blocks (it is backed by a plain `Vec`).
//!
//! `Deck`/`Engine` never scale samples for volume themselves -- only for
//! ReplayGain (see `soul_audio_core::deck`'s module docs) -- `set_volume` is
//! forwarded straight to the sink, to be applied "at the output stage" the
//! same way a real device sink (cpal ring, SAI DMA) would. `SimSink` plays
//! that role here: it multiplies every written sample by the last
//! `set_volume` value before recording it, so a volume change is directly
//! observable in `recorded()`.

use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use soul_audio_core::{SampleFormat, Sink, SinkError, StreamInfo};

/// Declared device sample rate (Hz).
pub const DEVICE_RATE: u32 = 48_000;
/// Declared device channel count.
pub const DEVICE_CHANNELS: u16 = 2;

/// A non-blocking `Sink` that records every written block into memory
/// (optionally mirroring the raw interleaved `i32` LE stream to a `.pcm`
/// file for off-line inspection).
pub struct SimSink {
    info: StreamInfo,
    volume: f32,
    /// Every frame ever written, post-volume, interleaved per `info.channels`.
    recorded: Vec<i32>,
    /// Per-`write` call frame counts, in order -- lets a caller walk block
    /// boundaries if a plain sample scan isn't enough.
    block_frames: Vec<usize>,
    flush_count: usize,
    discard_count: usize,
    dump: Option<File>,
}

impl SimSink {
    /// A sink declaring [`DEVICE_RATE`] / [`DEVICE_CHANNELS`] / canonical
    /// 24-bit, with no `.pcm` dump.
    #[must_use]
    pub fn new() -> Self {
        Self {
            info: StreamInfo {
                sample_rate: DEVICE_RATE,
                channels: DEVICE_CHANNELS,
                format: SampleFormat::Int { bits: 24 },
            },
            volume: 1.0,
            recorded: Vec::new(),
            block_frames: Vec::new(),
            flush_count: 0,
            discard_count: 0,
            dump: None,
        }
    }

    /// Like [`new`](Self::new), also mirroring every written block (raw
    /// interleaved `i32` LE, post-volume) to `path` for off-line/by-ear
    /// inspection.
    ///
    /// # Errors
    /// An I/O error creating `path`.
    pub fn with_dump(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self {
            dump: Some(File::create(path)?),
            ..Self::new()
        })
    }

    /// Declared device sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.info.sample_rate
    }

    /// Declared device channel count.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.info.channels as usize
    }

    /// Every frame ever written, post-volume, interleaved per [`channels`](Self::channels).
    #[must_use]
    pub fn recorded(&self) -> &[i32] {
        &self.recorded
    }

    /// Per-`write` call frame counts, in call order.
    #[must_use]
    pub fn block_frames(&self) -> &[usize] {
        &self.block_frames
    }

    /// Number of [`flush`](Sink::flush) calls observed.
    #[must_use]
    pub fn flush_count(&self) -> usize {
        self.flush_count
    }

    /// Number of [`discard`](Sink::discard) calls observed.
    #[must_use]
    pub fn discard_count(&self) -> usize {
        self.discard_count
    }
}

impl Default for SimSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for SimSink {
    fn stream_info(&self) -> StreamInfo {
        self.info
    }

    fn write(&mut self, frames: &[i32]) -> Result<(), SinkError> {
        let ch = self.channels();
        debug_assert_eq!(
            frames.len() % ch,
            0,
            "write length must be a whole number of frames"
        );
        self.block_frames.push(frames.len() / ch);

        if self.volume == 1.0 {
            self.recorded.extend_from_slice(frames);
        } else {
            let vol = f64::from(self.volume);
            self.recorded
                .extend(frames.iter().map(|&s| (f64::from(s) * vol).round() as i32));
        }

        if let Some(dump) = &mut self.dump {
            let tail = &self.recorded[self.recorded.len() - frames.len()..];
            for sample in tail {
                // Never blocks: an in-memory Vec backs every write, and this
                // dump is a plain local file write (no device/backpressure).
                dump.write_all(&sample.to_le_bytes())
                    .map_err(|_| SinkError::Closed)?;
            }
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), SinkError> {
        self.flush_count += 1;
        Ok(())
    }

    fn discard(&mut self) -> Result<(), SinkError> {
        self.discard_count += 1;
        Ok(())
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }
}
