//! DAC (Digital-to-Analogue Converter) driver abstractions
//!
//! Concrete drivers are feature-gated by build target:
//! - `es9038q2m` — ESS ES9038Q2M hardware driver (`hardware` feature)
//! - `mock` — In-process mock for host tests (always available)

#![allow(async_fn_in_trait)]

pub mod es9038q2m;
pub mod mock;

use platform::{AudioCodec, AudioConfig};

/// DAP-specific DAC trait, extending [`platform::AudioCodec`].
///
/// Adds hardware initialisation and power-management methods that are
/// not part of the generic codec interface.
pub trait DacDriver: AudioCodec {
    /// Full hardware initialisation sequence.
    ///
    /// Performs chip reset, writes default register values, and configures
    /// the I²S input format to match the STM32 SAI output.
    async fn hardware_init(&mut self, config: AudioConfig) -> Result<(), Self::Error>;

    /// Enter soft power-down (mute + low-power state).
    async fn power_down(&mut self) -> Result<(), Self::Error>;

    /// Wake from soft power-down.
    async fn power_up(&mut self) -> Result<(), Self::Error>;
}
