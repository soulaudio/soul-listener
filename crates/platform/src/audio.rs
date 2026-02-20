//! Audio codec abstraction
//!
//! Designed around the ES9038Q2M DAC (ESS Technology):
//! - 32-bit PCM up to 768 kHz
//! - DSD64–DSD512 (native bitstream or `DoP` — DSD over PCM)
//! - 128 dB DNR, −120 dB THD+N
//! - I²C programmable: volume, oversampling filter, auto-mute, DSD mode

/// Audio codec trait
pub trait AudioCodec {
    /// Error type
    type Error: core::fmt::Debug;

    /// Initialize codec with configuration
    fn init(
        &mut self,
        config: AudioConfig,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Start playback
    fn start(&mut self) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Stop playback
    fn stop(&mut self) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Set volume (0–100, mapped to hardware attenuation register)
    fn set_volume(
        &mut self,
        volume: u8,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Write 32-bit PCM audio samples (interleaved L/R for stereo).
    ///
    /// For 16-bit and 24-bit content the samples should be left-justified
    /// in the 32-bit word (i.e. shifted to the MSBs).
    ///
    /// For DSD (`DoP` or native) this method is not used; DSD is streamed
    /// directly over I²S by the DMA peripheral.
    fn write_samples(
        &mut self,
        samples: &[i32],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Set oversampling filter (optional — codecs that do not support
    /// programmable filters may ignore this).
    fn set_filter(
        &mut self,
        filter: OversamplingFilter,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;
}

/// Audio configuration
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AudioConfig {
    /// Sample rate in Hz (PCM: 44100–768000; ignored for native DSD)
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u8,
    /// Bit depth for PCM: 16, 24, or 32
    pub bit_depth: u8,
    /// DSD playback mode
    pub dsd_mode: DsdMode,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 96_000,
            channels: 2,
            bit_depth: 32,
            dsd_mode: DsdMode::Disabled,
        }
    }
}

/// DSD (Direct Stream Digital) playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DsdMode {
    /// PCM only (default)
    #[default]
    Disabled,
    /// DSD over PCM (`DoP`) — works over standard I²S without hardware changes.
    /// Supported rates: DSD64, DSD128, DSD256.
    Dop,
    /// Native DSD bitstream — requires DAC hardware support (ES9038Q2M supports up to DSD512).
    Native,
}

impl AudioConfig {
    /// Validate that this configuration is acceptable for the ES9038Q2M DAC.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a human-readable message when:
    /// - `sample_rate` is zero or above the ES9038Q2M maximum of 768 000 Hz.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.sample_rate == 0 || self.sample_rate > 768_000 {
            return Err("sample_rate out of range [1, 768_000]");
        }
        Ok(())
    }
}

/// Oversampling filter selection for the ES9038Q2M
///
/// The ES9038Q2M provides seven programmable PCM oversampling filters and
/// two DSD filters via register 0x0B (Filter Shape / System Register).
/// All filters operate identically at frequencies below ~20 kHz; differences
/// appear in the stop-band and time-domain behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OversamplingFilter {
    /// Filter 1 — Fast roll-off, linear phase (ES9038Q2M default).
    /// Best measured stop-band rejection. Typical choice for measurement.
    #[default]
    FastRollOffLinearPhase,
    /// Filter 2 — Slow roll-off, linear phase.
    /// Gentler transition band; some listeners prefer its time-domain response.
    SlowRollOffLinearPhase,
    /// Filter 3 — Fast roll-off, minimum phase.
    /// No pre-ringing; post-ringing only.
    FastRollOffMinimumPhase,
    /// Filter 4 — Slow roll-off, minimum phase.
    SlowRollOffMinimumPhase,
    /// Filter 5 — Apodising fast roll-off, linear phase.
    /// Minimises pre-ringing from earlier recording/mastering filters.
    ApodizingFastRollOff,
    /// Filter 6 — Brick-wall (very sharp roll-off), linear phase.
    BrickWall,
    /// Filter 7 — Hybrid fast roll-off, minimum phase.
    HybridFastRollOff,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_config_rejects_zero_sample_rate() {
        let cfg = AudioConfig {
            sample_rate: 0,
            ..AudioConfig::default()
        };
        assert!(cfg.validate().is_err(), "sample rate 0 must be invalid");
    }

    #[test]
    fn test_audio_config_rejects_out_of_range_sample_rate() {
        // ES9038Q2M supports up to 768000 Hz
        let cfg = AudioConfig {
            sample_rate: 1_000_000,
            ..AudioConfig::default()
        };
        assert!(
            cfg.validate().is_err(),
            "sample rate >768000 must be invalid"
        );
    }

    #[test]
    fn test_audio_config_accepts_valid_sample_rates() {
        for &sr in &[
            44_100u32, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000, 768_000,
        ] {
            let cfg = AudioConfig {
                sample_rate: sr,
                ..AudioConfig::default()
            };
            assert!(cfg.validate().is_ok(), "sample rate {sr} must be valid");
        }
    }
}
