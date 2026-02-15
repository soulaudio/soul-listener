//! Audio codec abstraction

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

    /// Set volume (0-100)
    fn set_volume(
        &mut self,
        volume: u8,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Write audio samples
    fn write_samples(
        &mut self,
        samples: &[i16],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;
}

/// Audio configuration
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AudioConfig {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo)
    pub channels: u8,
    /// Bit depth (16 or 24)
    pub bit_depth: u8,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            bit_depth: 16,
        }
    }
}
