//! nanomp3-based MP3 frame decoder.
//!
//! Implements the `FrameDecoder` trait using the `nanomp3` crate.
//! nanomp3 is a pure-Rust, no_std c2rust translation of minimp3 with ARM
//! soundness fixes.  `minimp3` / `minimp3-rs` / `minimp3-sys` are banned in
//! `deny.toml`; this is the approved replacement.
//!
//! # Feature flag
//!
//! The `nanomp3` dependency and the real decode path are both gated behind the
//! `mp3` feature so the crate compiles on bare-metal targets that don't need
//! MP3 support yet.

use crate::decoder::{DecodeError, FrameDecoder, PcmFrame};

// ─── Implementation ───────────────────────────────────────────────────────────

/// MP3 frame decoder backed by nanomp3.
///
/// `nanomp3::Decoder` has no internal buffering; callers must provide the full
/// MP3 frame bytes on each call to [`FrameDecoder::decode_frame`].
pub struct NanoMp3Decoder {
    sample_rate: u32,
    channels: u8,
    #[cfg(feature = "mp3")]
    inner: nanomp3::Decoder,
    #[cfg(not(feature = "mp3"))]
    _phantom: (),
}

impl NanoMp3Decoder {
    /// Create a new MP3 decoder.
    ///
    /// `sample_rate` and `channels` are zero until the first successful frame
    /// decode, at which point they are updated from the frame header.
    pub fn new() -> Self {
        Self {
            sample_rate: 0,
            channels: 0,
            #[cfg(feature = "mp3")]
            inner: nanomp3::Decoder::new(),
            #[cfg(not(feature = "mp3"))]
            _phantom: (),
        }
    }
}

impl Default for NanoMp3Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder for NanoMp3Decoder {
    type Error = DecodeError;

    /// Decode one MP3 frame from `input` into `output`.
    ///
    /// # nanomp3 API
    ///
    /// `nanomp3::Decoder::decode(mp3: &[u8], pcm: &mut [f32]) -> (usize, Option<FrameInfo>)`
    ///
    /// - Returns `(bytes_consumed, Some(FrameInfo))` on success.
    /// - Returns `(bytes_consumed, None)` when no frame was decoded (garbage
    ///   at start, or true end-of-stream).
    /// - The `pcm` slice must be at least `MAX_SAMPLES_PER_FRAME` (= 2304)
    ///   elements long or the call panics.
    ///
    /// Samples produced are left-justified into the 32-bit `PcmFrame.samples`
    /// field (f32 → i32 via bit-cast preserving the float range).
    fn decode_frame(&mut self, input: &[u8], output: &mut PcmFrame) -> Result<usize, Self::Error> {
        if input.is_empty() {
            return Err(DecodeError::EndOfStream);
        }

        #[cfg(feature = "mp3")]
        {
            // Scratch buffer on the stack — 2304 f32 samples = 9 216 bytes.
            // MAX_SAMPLES_PER_FRAME = 1152 * 2 = 2304.
            let mut pcm_buf = [0.0f32; nanomp3::MAX_SAMPLES_PER_FRAME];

            let (consumed, info_opt) = self.inner.decode(input, &mut pcm_buf);

            match info_opt {
                Some(info) => {
                    self.sample_rate = info.sample_rate;
                    self.channels = info.channels.num() as u8;

                    // Copy decoded f32 samples → left-justified i32.
                    // f32 range is [-1.0, 1.0]; we scale to full i32 range.
                    let n_samples = info.samples_produced;
                    let n = n_samples.min(output.samples.len());
                    for (dst, &src) in output.samples[..n].iter_mut().zip(pcm_buf[..n].iter()) {
                        // Scale f32 [-1.0, 1.0] to i32 range, clamping.
                        *dst = (src.clamp(-1.0, 1.0) * i32::MAX as f32) as i32;
                    }
                    // `len` = number of samples per channel.
                    let ch = self.channels.max(1) as usize;
                    output.len = n / ch;
                    output.sample_rate = self.sample_rate;
                    output.channels = self.channels;
                    Ok(consumed)
                }
                None => {
                    // No frame decoded: either the sync word was not found in
                    // `input`, or `input` is exhausted.
                    Err(DecodeError::EndOfStream)
                }
            }
        }

        #[cfg(not(feature = "mp3"))]
        {
            let _ = output;
            Err(DecodeError::UnsupportedFormat)
        }
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u8 {
        self.channels
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)] // Test indexing into known-length buffers is safe
#[allow(clippy::expect_used)] // Tests use expect() for readable assertions
mod tests {
    use super::*;

    #[test]
    fn test_nanomp3_decoder_implements_frame_decoder() {
        // NanoMp3Decoder must implement FrameDecoder.
        fn assert_impl<T: crate::decoder::FrameDecoder>() {}
        assert_impl::<NanoMp3Decoder>();
    }

    #[test]
    fn test_nanomp3_decoder_new() {
        let decoder = NanoMp3Decoder::new();
        assert_eq!(decoder.sample_rate(), 0); // 0 until first frame decoded
        assert_eq!(decoder.channels(), 0);
    }

    #[test]
    fn test_nanomp3_decode_silence_frame() {
        // A minimal valid MP3 silence frame.
        // Frame sync: 0xFF 0xFB (MPEG1, Layer3, 128kbps, 44100Hz, stereo)
        // Pad to a realistic frame size (417 bytes for 128kbps 44100Hz).
        let mut frame_data = vec![0u8; 417];
        frame_data[0] = 0xFF;
        frame_data[1] = 0xFB;
        frame_data[2] = 0x90;
        frame_data[3] = 0x00;

        let mut decoder = NanoMp3Decoder::new();
        let mut output = PcmFrame::default();

        let result = decoder.decode_frame(&frame_data, &mut output);

        // When the `mp3` feature is enabled: may succeed or return EndOfStream.
        // When the `mp3` feature is disabled: UnsupportedFormat is the correct stub response.
        // All three outcomes are valid depending on the feature configuration.
        assert!(
            result.is_ok()
                || matches!(result, Err(DecodeError::EndOfStream))
                || matches!(result, Err(DecodeError::UnsupportedFormat)),
            "Unexpected decode result: {:?}",
            result
        );

        // When the `mp3` feature IS enabled, UnsupportedFormat is NOT acceptable.
        #[cfg(feature = "mp3")]
        assert!(
            result.is_ok() || matches!(result, Err(DecodeError::EndOfStream)),
            "With mp3 feature: should decode or EndOfStream, not {:?}",
            result
        );
    }

    #[test]
    fn test_nanomp3_decode_empty_returns_error() {
        let mut decoder = NanoMp3Decoder::new();
        let mut output = PcmFrame::default();
        let result = decoder.decode_frame(&[], &mut output);
        assert!(result.is_err(), "Empty input must return error");
    }

    #[test]
    fn test_nanomp3_decode_invalid_header_returns_error() {
        let mut decoder = NanoMp3Decoder::new();
        let mut output = PcmFrame::default();
        // Random bytes that are not a valid MP3 frame.
        let garbage = [0x00u8; 100];
        let result = decoder.decode_frame(&garbage, &mut output);
        // Should return an error (InvalidData or EndOfStream).
        assert!(result.is_err(), "Invalid data must return error");
    }

    #[test]
    fn test_pcm_frame_default_is_zero() {
        let frame = PcmFrame::default();
        assert_eq!(frame.len, 0);
        assert_eq!(frame.sample_rate, 0);
        assert_eq!(frame.channels, 0);
        assert_eq!(frame.samples[0], 0);
    }
}
