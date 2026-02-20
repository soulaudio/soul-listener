//! Audio peripheral configuration for SoulAudio DAP.
//!
//! Defines SAI1 configuration for the ES9038Q2M DAC and
//! I2C addresses/registers for PMIC and DAC control.
//!
//! # SAI1 Clock Chain
//!
//! The ES9038Q2M requires a precise MCLK derived from the audio sample rate:
//!
//! ```text
//! PLL1Q (configured in firmware::boot::build_embassy_config)
//!   → SAI1 kernel clock
//!   → MCLK_A (PE2, AF6) = 256 × fs
//! ```
//!
//! For 192 kHz: MCLK = 256 × 192 000 = 49.152 MHz
//! For 96 kHz:  MCLK = 256 × 96 000  = 24.576 MHz
//!
//! # I2C Bus Assignments
//!
//! | Bus  | Peripheral        | Address | Speed    |
//! |------|-------------------|---------|----------|
//! | I2C2 | BQ25895 PMIC      | 0x6A    | 100 kHz  |
//! | I2C3 | ES9038Q2M DAC     | 0x48    | 400 kHz  |

/// SAI1 clock and format configuration for audio output.
///
/// Target: 32-bit, 192 kHz, 2 channels (stereo)
/// MCLK = 256 × fs = 256 × 192 000 = 49.152 MHz (from PLL1Q)
///
/// # Pin Assignments (STM32H743ZI LQFP144, SAI1 Block A)
///
/// | Function     | Pin | AF   |
/// |--------------|-----|------|
/// | SAI1_MCLK_A  | PE2 | AF6  |
/// | SAI1_FS_A    | PE4 | AF6  |
/// | SAI1_SCK_A   | PE5 | AF6  |
/// | SAI1_SD_A    | PE6 | AF6  |
///
/// # DMA
///
/// DMA1 Stream 0, Request 87 (SAI1_A TX) in circular mode.
/// Buffer must be in `.axisram` (AXI SRAM — DMA1-accessible, non-cacheable via MPU).
/// DTCM (`0x2000_0000`) is NOT accessible by DMA1 — do not place the buffer there.
#[derive(Debug, Clone, Copy)]
pub struct SaiAudioConfig {
    /// Sample rate in Hz (e.g. 192_000, 96_000, 48_000).
    pub sample_rate_hz: u32,
    /// Bit depth per sample (16, 24, or 32 for PCM; 32 for DoP).
    pub bit_depth: u8,
    /// Number of channels (1 = mono, 2 = stereo).
    pub channels: u8,
    /// MCLK multiplier: MCLK = `mclk_div` × `sample_rate_hz`.
    ///
    /// ES9038Q2M requires MCLK = 256 × fs in I2S master mode.
    pub mclk_div: u16,
}

impl SaiAudioConfig {
    /// ES9038Q2M reference configuration: 32-bit / 192 kHz stereo via SAI1.
    ///
    /// Clock derivation:
    /// - MCLK = 256 × 192 000 = 49.152 MHz (from PLL1Q)
    /// - BCLK = 32 × 2 × 192 000 = 12.288 MHz (= MCLK / 4)
    /// - FS (LRCK) = 192 000 Hz
    ///
    /// STM32H743 SAI1 in master transmit mode, I2S format, 32-bit slot.
    pub fn es9038q2m_192khz() -> Self {
        Self {
            sample_rate_hz: 192_000,
            bit_depth: 32,
            channels: 2,
            mclk_div: 256,
        }
    }

    /// Calculate the master clock (MCLK) frequency in Hz.
    ///
    /// MCLK = `mclk_div` × `sample_rate_hz`.
    /// For 192 kHz / 256 fs: 49 152 000 Hz (49.152 MHz).
    pub fn mclk_hz(&self) -> u32 {
        self.sample_rate_hz * u32::from(self.mclk_div)
    }

    /// Calculate the bit clock (BCLK) frequency in Hz.
    ///
    /// BCLK = `bit_depth` × `channels` × `sample_rate_hz`.
    /// For 32-bit / 2ch / 192 kHz: 12 288 000 Hz (12.288 MHz).
    pub fn bclk_hz(&self) -> u32 {
        u32::from(self.bit_depth) * u32::from(self.channels) * self.sample_rate_hz
    }
}

/// I2C addresses for SoulAudio DAP peripherals.
///
/// All addresses are 7-bit (the embedded-hal standard convention).
/// Shift left by 1 and OR with the R/W bit to get the 8-bit wire address.
pub struct I2cAddresses;

impl I2cAddresses {
    /// BQ25895 USB-C PMIC I2C address.
    ///
    /// The BQ25895 has a fixed 7-bit address of **0x6A**. There is no
    /// configurable ADDR pin — the 0x6B value that appeared in some early
    /// datasheet revisions was a typographic error (confirmed by TI E2E forum,
    /// SLUUBA2B errata).
    ///
    /// Wire address: 0xD4 (write) / 0xD5 (read).
    pub const BQ25895_PMIC: u8 = 0x6A;

    /// ES9038Q2M DAC I2C control address (hardware-fixed).
    ///
    /// The ES9038Q2M has a fixed 7-bit address of **0x48**. This is
    /// hard-wired on-chip and cannot be changed.
    ///
    /// Wire address: 0x90 (write) / 0x91 (read).
    pub const ES9038Q2M_DAC: u8 = 0x48;
}

/// I2C bus assignments for SoulAudio DAP peripherals.
///
/// The STM32H743ZI has four I2C peripherals (I2C1–I2C4). This table
/// documents which bus each SoulAudio peripheral is connected to.
pub struct I2cBusAssignment;

impl I2cBusAssignment {
    /// I2C bus number for the BQ25895 PMIC. Bus 2 (I2C2).
    ///
    /// Pins: PF0 (SDA, AF4), PF1 (SCL, AF4) — 100 kHz standard mode.
    /// I2C2 is in the D2 domain; DMA-capable via DMA1_CH4/CH5.
    pub const PMIC_BUS: u8 = 2;

    /// I2C bus number for ES9038Q2M DAC control. Bus 3 (I2C3).
    ///
    /// Pins: PC9 (SDA, AF4), PA8 (SCL, AF4) — 400 kHz fast mode.
    /// I2C3 is in the D2 domain; DMA-capable via DMA1_CH6/CH7.
    pub const DAC_BUS: u8 = 3;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn sai_config_sample_rate_192khz() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        assert_eq!(cfg.sample_rate_hz, 192_000);
    }

    #[test]
    fn sai_config_mclk_is_49_152_mhz() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        assert_eq!(
            cfg.mclk_hz(),
            49_152_000,
            "MCLK must be 49.152 MHz for 192kHz/256fs"
        );
    }

    #[test]
    fn sai_config_bclk_is_correct() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        // BCLK = 32 bits × 2 channels × 192 000 = 12.288 MHz
        assert_eq!(cfg.bclk_hz(), 12_288_000);
    }

    #[test]
    fn sai_config_bit_depth_is_32() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        assert_eq!(cfg.bit_depth, 32, "ES9038Q2M supports 32-bit PCM");
    }

    #[test]
    fn sai_config_channels_is_2() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        assert_eq!(cfg.channels, 2, "stereo output");
    }

    #[test]
    fn sai_config_mclk_div_is_256() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        assert_eq!(cfg.mclk_div, 256, "ES9038Q2M I2S master: MCLK = 256 × fs");
    }

    #[test]
    fn pmic_i2c_address_is_0x6a() {
        // BQ25895 address is fixed at 0x6A.
        // 0x6B appeared in early datasheets but was a typo (TI E2E errata).
        let addr = I2cAddresses::BQ25895_PMIC;
        assert_eq!(
            addr, 0x6A,
            "BQ25895 I2C address must be 0x6A (fixed, no ADDR pin)"
        );
    }

    #[test]
    fn pmic_i2c_address_is_valid_7bit() {
        let addr = I2cAddresses::BQ25895_PMIC;
        assert!(
            addr == 0x6A || addr == 0x6B,
            "BQ25895 I2C address must be 0x6A or 0x6B, got 0x{addr:02X}"
        );
    }

    #[test]
    fn dac_i2c_address_is_0x48() {
        assert_eq!(
            I2cAddresses::ES9038Q2M_DAC,
            0x48,
            "ES9038Q2M I2C address is hardware-fixed at 0x48"
        );
    }

    #[test]
    fn pmic_is_on_i2c2() {
        assert_eq!(I2cBusAssignment::PMIC_BUS, 2);
    }

    #[test]
    fn dac_is_on_i2c3() {
        assert_eq!(I2cBusAssignment::DAC_BUS, 3);
    }

    #[test]
    fn bclk_mclk_ratio_is_power_of_two() {
        let cfg = SaiAudioConfig::es9038q2m_192khz();
        let ratio = cfg.mclk_hz() / cfg.bclk_hz();
        // MCLK / BCLK must be a power of 2 for SAI internal divider
        // 49.152 MHz / 12.288 MHz = 4 = 2^2
        assert!(
            ratio.is_power_of_two(),
            "MCLK/BCLK ratio must be a power of 2, got {ratio}"
        );
    }
}
