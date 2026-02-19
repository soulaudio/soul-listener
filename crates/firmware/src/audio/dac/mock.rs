//! Mock DAC for host-side testing
//!
//! Implements [`DacDriver`] and [`platform::AudioCodec`] without any hardware
//! dependency. Records all calls for assertion in tests.

use platform::{AudioCodec, AudioConfig, DsdMode, OversamplingFilter};

use super::DacDriver;

/// Mock DAC — records all calls for test assertions.
pub struct MockDac {
    /// Current volume setting (0–100)
    pub volume: u8,
    /// Total number of i32 samples written via [`AudioCodec::write_samples`]
    pub samples_written: usize,
    /// Last filter set via [`AudioCodec::set_filter`]
    pub filter: OversamplingFilter,
    /// DSD mode from last [`AudioCodec::init`]
    pub dsd_mode: DsdMode,
    /// Whether [`AudioCodec::start`] has been called (and not followed by `stop`)
    pub started: bool,
}

impl MockDac {
    /// Create a new mock DAC with sensible defaults.
    pub fn new() -> Self {
        Self {
            volume: 80,
            samples_written: 0,
            filter: OversamplingFilter::default(),
            dsd_mode: DsdMode::Disabled,
            started: false,
        }
    }
}

impl Default for MockDac {
    fn default() -> Self {
        Self::new()
    }
}

impl DacDriver for MockDac {
    async fn hardware_init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        self.dsd_mode = config.dsd_mode;
        Ok(())
    }

    async fn power_down(&mut self) -> Result<(), Self::Error> {
        self.started = false;
        Ok(())
    }

    async fn power_up(&mut self) -> Result<(), Self::Error> {
        self.started = true;
        Ok(())
    }
}

impl AudioCodec for MockDac {
    type Error = core::convert::Infallible;

    async fn init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        self.dsd_mode = config.dsd_mode;
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        self.started = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.started = false;
        Ok(())
    }

    async fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
        self.volume = volume.min(100);
        Ok(())
    }

    async fn write_samples(&mut self, samples: &[i32]) -> Result<(), Self::Error> {
        self.samples_written += samples.len();
        Ok(())
    }

    async fn set_filter(&mut self, filter: OversamplingFilter) -> Result<(), Self::Error> {
        self.filter = filter;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::AudioConfig;

    #[tokio::test]
    async fn test_mock_dac_init() {
        let mut dac = MockDac::new();
        dac.init(AudioConfig::default()).await.unwrap();
        assert_eq!(dac.dsd_mode, DsdMode::Disabled);
    }

    #[tokio::test]
    async fn test_mock_dac_volume() {
        let mut dac = MockDac::new();
        dac.set_volume(75).await.unwrap();
        assert_eq!(dac.volume, 75);
        // Clamped to 100
        dac.set_volume(200).await.unwrap();
        assert_eq!(dac.volume, 100);
    }

    #[tokio::test]
    async fn test_mock_dac_samples() {
        let mut dac = MockDac::new();
        let samples = [0i32; 512];
        dac.write_samples(&samples).await.unwrap();
        assert_eq!(dac.samples_written, 512);
    }

    #[tokio::test]
    async fn test_mock_dac_filter() {
        let mut dac = MockDac::new();
        dac.set_filter(OversamplingFilter::ApodizingFastRollOff)
            .await
            .unwrap();
        assert_eq!(dac.filter, OversamplingFilter::ApodizingFastRollOff);
    }

    #[tokio::test]
    async fn test_mock_dac_start_stop() {
        let mut dac = MockDac::new();
        dac.start().await.unwrap();
        assert!(dac.started);
        dac.stop().await.unwrap();
        assert!(!dac.started);
    }

    #[tokio::test]
    async fn test_mock_dac_power_cycle() {
        let mut dac = MockDac::new();
        dac.power_up().await.unwrap();
        assert!(dac.started);
        dac.power_down().await.unwrap();
        assert!(!dac.started);
    }
}
