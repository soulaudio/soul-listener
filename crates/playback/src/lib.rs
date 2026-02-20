//! Audio playback engine — FLAC/MP3/WAV decoding, DMA streaming to SAI I²S
#![cfg_attr(not(test), no_std)]
#![deny(clippy::unwrap_used)]

pub mod decoder;
pub mod engine;
pub mod ring_buffer;
pub mod volume;

// Tests come first — implementations below will make them pass
#[cfg(test)]
mod tests {
    /// Decoder abstraction tests
    mod decoder_tests {
        use crate::decoder::{AudioFormat, DecodeError, PcmFrame};

        #[test]
        fn test_pcm_frame_holds_sample_count() {
            let frame = PcmFrame {
                samples: [0i32; 4096],
                len: 576,
                sample_rate: 44100,
                channels: 2,
            };
            assert_eq!(frame.len, 576);
            assert_eq!(frame.sample_rate, 44100);
            assert_eq!(frame.channels, 2);
        }

        #[test]
        fn test_decode_error_is_debug() {
            let e = DecodeError::InvalidData;
            let s = format!("{e:?}");
            assert!(!s.is_empty());
        }

        #[test]
        fn test_audio_format_detection_flac() {
            assert_eq!(AudioFormat::from_extension("flac"), Some(AudioFormat::Flac));
        }

        #[test]
        fn test_audio_format_detection_mp3() {
            assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
        }

        #[test]
        fn test_audio_format_detection_wav() {
            assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
        }

        #[test]
        fn test_audio_format_unknown_returns_none() {
            assert_eq!(AudioFormat::from_extension("txt"), None);
        }
    }

    /// Playback state machine tests
    mod engine_tests {
        use crate::engine::{PlaybackEngine, PlaybackError, PlaybackState};

        #[test]
        fn test_engine_starts_stopped() {
            let engine = PlaybackEngine::new();
            assert_eq!(engine.state(), PlaybackState::Stopped);
        }

        #[test]
        fn test_play_transitions_to_playing() {
            let mut engine = PlaybackEngine::new();
            engine.play().expect("play from stopped should succeed");
            assert_eq!(engine.state(), PlaybackState::Playing);
        }

        #[test]
        fn test_pause_from_playing() {
            let mut engine = PlaybackEngine::new();
            engine.play().expect("play should succeed");
            engine.pause().expect("pause from playing should succeed");
            assert_eq!(engine.state(), PlaybackState::Paused);
        }

        #[test]
        fn test_stop_from_playing() {
            let mut engine = PlaybackEngine::new();
            engine.play().expect("play should succeed");
            engine.stop().expect("stop from playing should succeed");
            assert_eq!(engine.state(), PlaybackState::Stopped);
        }

        #[test]
        fn test_stop_from_paused() {
            let mut engine = PlaybackEngine::new();
            engine.play().expect("play should succeed");
            engine.pause().expect("pause should succeed");
            engine.stop().expect("stop from paused should succeed");
            assert_eq!(engine.state(), PlaybackState::Stopped);
        }

        #[test]
        fn test_cannot_pause_when_stopped() {
            let mut engine = PlaybackEngine::new();
            let result = engine.pause();
            assert_eq!(result, Err(PlaybackError::NotPlaying));
        }

        #[test]
        fn test_seek_updates_position() {
            let mut engine = PlaybackEngine::with_duration(60_000);
            engine.seek_ms(5000);
            assert_eq!(engine.position_ms(), 5000);
        }

        #[test]
        fn test_seek_clamped_to_duration() {
            let mut engine = PlaybackEngine::with_duration(10_000);
            engine.seek_ms(99_999);
            assert_eq!(engine.position_ms(), 10_000);
        }
    }

    /// Ring buffer tests
    mod ring_buffer_tests {
        use crate::ring_buffer::RingBuffer;

        #[test]
        fn test_ring_buffer_write_then_read() {
            let mut rb: RingBuffer<64> = RingBuffer::new();
            let data: [i32; 16] = core::array::from_fn(|i| i as i32);
            rb.write_slice(&data).expect("write should succeed");
            let mut out = [0i32; 16];
            let n = rb.read_slice(&mut out);
            assert_eq!(n, 16);
            assert_eq!(out, data);
        }

        #[test]
        fn test_ring_buffer_available_after_write() {
            let mut rb: RingBuffer<64> = RingBuffer::new();
            let data = [1i32; 20];
            rb.write_slice(&data).expect("write should succeed");
            assert_eq!(rb.available(), 20);
        }

        #[test]
        fn test_ring_buffer_full_returns_err() {
            let mut rb: RingBuffer<8> = RingBuffer::new();
            let data = [0i32; 8];
            rb.write_slice(&data).expect("filling to capacity should succeed");
            // One more should fail
            let result = rb.write_slice(&[42i32]);
            assert!(result.is_err(), "writing past capacity must fail");
        }

        #[test]
        fn test_ring_buffer_wraps_around() {
            let mut rb: RingBuffer<8> = RingBuffer::new();
            // Write CAPACITY samples
            let first = [1i32; 8];
            rb.write_slice(&first).expect("initial fill");
            // Read half
            let mut half = [0i32; 4];
            let n = rb.read_slice(&mut half);
            assert_eq!(n, 4);
            assert_eq!(half, [1i32; 4]);
            // Write 4 more (wrapping)
            let second = [2i32; 4];
            rb.write_slice(&second).expect("wrap-around write");
            // Read remaining 4 (original) + 4 (new)
            let mut rest = [0i32; 8];
            let n2 = rb.read_slice(&mut rest);
            assert_eq!(n2, 8);
            assert_eq!(&rest[..4], &[1i32; 4]);
            assert_eq!(&rest[4..], &[2i32; 4]);
        }
    }

    /// Volume/DSP tests
    mod volume_tests {
        use crate::volume::volume_to_attenuation;

        #[test]
        fn test_volume_linear_to_attenuation_zero() {
            // volume 0 -> max attenuation (255 = muted on ES9038Q2M)
            assert_eq!(volume_to_attenuation(0), 255);
        }

        #[test]
        fn test_volume_linear_to_attenuation_100() {
            // volume 100 -> 0 attenuation (0 dB, loudest)
            assert_eq!(volume_to_attenuation(100), 0);
        }

        #[test]
        fn test_volume_clamp_above_100() {
            // volume 150 maps same as 100
            assert_eq!(volume_to_attenuation(150), volume_to_attenuation(100));
        }

        #[test]
        fn test_volume_50_percent_is_midpoint() {
            // volume 50 -> ~127 attenuation
            // 255 - (50 * 255 / 100) = 255 - 127 = 128
            let att = volume_to_attenuation(50);
            assert_eq!(att, 128);
        }
    }
}
