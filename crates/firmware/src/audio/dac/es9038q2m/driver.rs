//! ES9038Q2M hardware driver for STM32H7
//!
//! Communicates with the chip via I²C. Uses the `embedded_hal_async::i2c::I2c`
//! trait so it is HAL-agnostic while remaining async.
//!
//! The audio stream itself is delivered over I²S by the STM32 SAI + DMA
//! peripheral — that path does not go through this driver.
//!
//! # I²C Address
//!
//! | ADDR pin | Address |
//! |----------|---------|
//! | GND      | `0x48`  |
//! | VDD      | `0x49`  |

use embedded_hal_async::i2c::I2c;
use platform::{AudioCodec, AudioConfig, DsdMode, OversamplingFilter};

use super::registers::*;
use crate::audio::dac::DacDriver;

/// Default I²C address (ADDR pin = GND)
const I2C_ADDR: u8 = 0x48;

/// ES9038Q2M DAC driver
pub struct Es9038q2mDriver<I> {
    i2c: I,
    volume: u8,
}

impl<I: I2c> Es9038q2mDriver<I> {
    /// Create a new ES9038Q2M driver.
    ///
    /// `i2c` must be a configured async I²C peripheral pointing at the chip.
    pub fn new(i2c: I) -> Self {
        Self { i2c, volume: 80 }
    }

    /// Write a single register over I²C.
    async fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), I::Error> {
        self.i2c.write(I2C_ADDR, &[reg, value]).await
    }

    /// Perform a soft reset (register 0x00 bit 0, self-clearing).
    async fn soft_reset(&mut self) -> Result<(), I::Error> {
        self.write_reg(REG_SYSTEM, SYSTEM_SOFT_RESET).await
    }

    /// Map volume 0–100 to ES9038Q2M attenuation register value.
    ///
    /// ES9038Q2M: `0x00` = 0 dB (loudest), `0xFF` = max attenuation (quietest).
    fn volume_to_att(volume: u8) -> u8 {
        ((100 - volume.min(100)) as u16 * 255 / 100) as u8
    }
}

impl<I: I2c> DacDriver for Es9038q2mDriver<I> {
    async fn hardware_init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        defmt::info!("Initialising ES9038Q2M DAC");

        // Soft reset
        self.soft_reset().await?;

        // I²S slave, 32-bit input
        self.write_reg(REG_INPUT_CONFIG, INPUT_I2S_32BIT).await?;

        // Master mode: slave (STM32 SAI drives all clocks)
        self.write_reg(REG_MASTER_MODE, MASTER_MODE_SLAVE).await?;

        // Initial volume
        let att = Self::volume_to_att(self.volume);
        self.write_reg(REG_VOLUME_LEFT, att).await?;
        self.write_reg(REG_VOLUME_RIGHT, att).await?;

        // DSD mode
        let dsd_reg = match config.dsd_mode {
            DsdMode::Disabled => 0x00,
            DsdMode::Dop => DSD_DOP_ENABLE,
            DsdMode::Native => DSD_NATIVE_ENABLE,
        };
        self.write_reg(REG_DSD_CONFIG, dsd_reg).await?;

        defmt::info!("ES9038Q2M initialisation complete");
        Ok(())
    }

    async fn power_down(&mut self) -> Result<(), Self::Error> {
        self.write_reg(REG_VOLUME_LEFT, VOLUME_MUTE).await?;
        self.write_reg(REG_VOLUME_RIGHT, VOLUME_MUTE).await
    }

    async fn power_up(&mut self) -> Result<(), Self::Error> {
        let att = Self::volume_to_att(self.volume);
        self.write_reg(REG_VOLUME_LEFT, att).await?;
        self.write_reg(REG_VOLUME_RIGHT, att).await
    }
}

impl<I: I2c> AudioCodec for Es9038q2mDriver<I> {
    type Error = I::Error;

    async fn init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        self.hardware_init(config).await
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        self.power_up().await
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.power_down().await
    }

    async fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
        self.volume = volume.min(100);
        let att = Self::volume_to_att(self.volume);
        self.write_reg(REG_VOLUME_LEFT, att).await?;
        self.write_reg(REG_VOLUME_RIGHT, att).await
    }

    async fn write_samples(&mut self, _samples: &[i32]) -> Result<(), Self::Error> {
        // The audio stream is delivered directly to the SAI/DMA peripheral.
        // This method is intentionally a no-op for hardware; the DMA path
        // bypasses the I²C driver entirely.
        Ok(())
    }

    async fn set_filter(&mut self, filter: OversamplingFilter) -> Result<(), Self::Error> {
        let bits: u8 = match filter {
            OversamplingFilter::FastRollOffLinearPhase => 0b000,
            OversamplingFilter::SlowRollOffLinearPhase => 0b001,
            OversamplingFilter::FastRollOffMinimumPhase => 0b010,
            OversamplingFilter::SlowRollOffMinimumPhase => 0b011,
            OversamplingFilter::ApodizingFastRollOff => 0b100,
            OversamplingFilter::BrickWall => 0b101,
            OversamplingFilter::HybridFastRollOff => 0b110,
        };
        self.write_reg(REG_OSF_FILTER, bits).await
    }
}
