//! Audio decoder abstractions — format detection, PCM frame types, codec traits.
//!
//! No concrete decoder crates are linked here yet; this module defines the
//! data types and traits that future FLAC/MP3/WAV decoder integrations must
//! implement.  The constraint of `no_std` + fixed-size stack arrays is
//! intentional: DMA buffers on the STM32H743 live in AXI SRAM and must never
//! touch the heap.
//!
//! # Decoder crate selection rationale (research 2025-02)
//!
//! * **MP3**: `nanomp3` (pure-Rust, `no_std`, c2rust translation of minimp3 with
//!   soundness fixes).  `minimp3` / `minimp3-rs` are banned in `deny.toml` due
//!   to multiple ARM-specific UB issues.  `symphonia` requires `std` and is too
//!   large for internal flash.
//!
//! * **FLAC**: `libfoxenflac` via C FFI (tiny, heap-free, state-machine based,
//!   GPL-2.0 C99 — build.rs integration pending) or `claxon` (pure-Rust but
//!   requires `std`).  `libfoxenflac` wins for embedded: 8.8 KB WASM, no alloc.
//!
//! * **WAV**: Parse PCM chunks directly — no third-party crate needed.

/// A decoded PCM frame — up to 4 096 samples per channel on the stack.
///
/// MP3 decodes at most 1 152 samples/channel; FLAC block size ≤ 4 096.
/// The array is always fully allocated; `len` indicates the valid suffix.
/// Samples are left-justified 32-bit signed integers (MSBs carry the audio
/// data regardless of the original bit depth).
#[derive(Clone)]
pub struct PcmFrame {
    /// Raw sample storage, left-justified 32-bit signed integers.
    pub samples: [i32; 4096],
    /// Number of valid samples in `samples` (per channel).
    pub len: usize,
    /// Sample rate in Hz (e.g. 44 100, 48 000, 96 000).
    pub sample_rate: u32,
    /// Channel count (1 = mono, 2 = stereo).
    pub channels: u8,
}

impl PcmFrame {
    /// Create a zeroed `PcmFrame` suitable for use as an output buffer.
    pub const fn zeroed() -> Self {
        Self {
            samples: [0i32; 4096],
            len: 0,
            sample_rate: 44_100,
            channels: 2,
        }
    }
}

/// Errors that a [`FrameDecoder`] may return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The input bitstream contains invalid or corrupt data.
    InvalidData,
    /// The input buffer is exhausted; no more frames can be decoded.
    EndOfStream,
    /// The codec does not support this file's parameters (e.g. DSD in a WAV decoder).
    UnsupportedFormat,
    /// The provided output buffer is too small for one decoded frame.
    BufferTooSmall,
}

/// Audio container / codec format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// Free Lossless Audio Codec
    Flac,
    /// MPEG Layer 3
    Mp3,
    /// Waveform Audio File Format (PCM or IEEE-float payload)
    Wav,
}

impl AudioFormat {
    /// Detect the audio format from a lowercase file extension.
    ///
    /// Returns `None` when the extension is not recognised.
    ///
    /// The match is case-sensitive; callers should lower-case the extension
    /// before calling this function.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "flac" => Some(Self::Flac),
            "mp3" => Some(Self::Mp3),
            "wav" => Some(Self::Wav),
            _ => None,
        }
    }
}

/// Trait for stateful, frame-by-frame audio decoders.
///
/// Each call to [`decode_frame`] consumes some bytes from `input` and writes
/// one decoded PCM frame to `output`, returning the number of input bytes
/// consumed.  Implementations must be `no_std`-safe and must not allocate.
///
/// [`decode_frame`]: FrameDecoder::decode_frame
pub trait FrameDecoder {
    /// Error type produced by this decoder.
    type Error: core::fmt::Debug;

    /// Decode one frame from `input` into `output`.
    ///
    /// # Returns
    ///
    /// `Ok(bytes_consumed)` on success, where `bytes_consumed ≤ input.len()`.
    ///
    /// # Errors
    ///
    /// Returns `Err(Self::Error)` on bitstream errors, format mismatches, or
    /// insufficient output buffer space.
    fn decode_frame(&mut self, input: &[u8], output: &mut PcmFrame) -> Result<usize, Self::Error>;

    /// Sample rate of the stream being decoded, in Hz.
    fn sample_rate(&self) -> u32;

    /// Number of audio channels in the stream.
    fn channels(&self) -> u8;
}
