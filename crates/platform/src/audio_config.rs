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
    #[allow(clippy::arithmetic_side_effects)] // Audio config: multiplication fits u32 (max 192k*512=98M < u32::MAX)
    pub fn mclk_hz(&self) -> u32 {
        self.sample_rate_hz * u32::from(self.mclk_div)
    }

    /// Calculate the bit clock (BCLK) frequency in Hz.
    ///
    /// BCLK = `bit_depth` × `channels` × `sample_rate_hz`.
    /// For 32-bit / 2ch / 192 kHz: 12 288 000 Hz (12.288 MHz).
    #[allow(clippy::arithmetic_side_effects)] // Audio config: 32*2*768000=49M < u32::MAX
    pub fn bclk_hz(&self) -> u32 {
        u32::from(self.bit_depth) * u32::from(self.channels) * self.sample_rate_hz
    }

    /// Best-achievable MCLK from HSI (64 MHz) with integer PLL divisors.
    ///
    /// Target: 49.152 MHz (256 × 192 kHz)
    /// Achieved: HSI(64) / M(4) × N(49) / P(16) = 49.0 MHz (0.31% error)
    ///
    /// This is acceptable for ES9038Q2M — the DAC PLL locks to the incoming
    /// MCLK and maintains exact ratios internally. 0.31% frequency error
    /// does not affect audio quality; it only shifts the exact sample rate
    /// from 192 000 Hz to ~191 406 Hz. For a DAP this is inaudible.
    ///
    /// Note: `mclk_hz()` returns the specification target (49.152 MHz);
    /// this method returns the hardware-achievable value (49.0 MHz).
    pub fn actual_mclk_hz() -> u32 {
        49_000_000 // 49.0 MHz achievable with integer PLL from HSI
    }

    /// PLL3 M divider for SAI MCLK.
    ///
    /// Divides HSI (64 MHz) input to the VCO reference: 64 / 4 = 16 MHz.
    /// Corresponds to `PllPreDiv::DIV4` in embassy-stm32.
    pub fn pll3_m() -> u8 {
        4
    }

    /// PLL3 N multiplier for SAI MCLK.
    ///
    /// VCO = 16 MHz × 49 = 784 MHz (within STM32H7 VCO range 192–836 MHz).
    /// Corresponds to `PllMul::MUL49` in embassy-stm32.
    pub fn pll3_n() -> u16 {
        49
    }

    /// PLL3 P divider for SAI MCLK output.
    ///
    /// MCLK = 784 MHz / 16 = 49.0 MHz → SAI1_MCLK_A pin (PE2, AF6).
    /// Corresponds to `PllDiv::DIV16` in embassy-stm32.
    pub fn pll3_p() -> u8 {
        16
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
    fn bq25895_address_is_0x6a() {
        // BQ25895 datasheet (SLUUBA2B, Table 6): 7-bit address = 0x6A.
        // The value 0x6B appearing in some documents is a confirmed datasheet
        // errata. TI E2E forum confirms 0x6A as the functional address:
        // https://e2e.ti.com/support/power-management-group/power-management/
        // f/power-management-forum/507682
        // There is NO address pin — the address is hardware-fixed.
        assert_eq!(
            I2cAddresses::BQ25895_PMIC,
            0x6A,
            "BQ25895 I2C address is hardware-fixed at 0x6A (not 0x6B)"
        );
    }

    #[test]
    // PLL divisor variables are conventionally named m/n/p in datasheet formulas.
    #[allow(clippy::many_single_char_names)]
    fn pll3_divisors_produce_correct_mclk() {
        // Verify PLL3 M/N/P produce the correct achievable MCLK frequency.
        // MCLK = HSI / M × N / P
        let hsi_hz: u64 = 64_000_000;
        let m = u64::from(SaiAudioConfig::pll3_m());
        let n = u64::from(SaiAudioConfig::pll3_n());
        let p = u64::from(SaiAudioConfig::pll3_p());
        let mclk = hsi_hz / m * n / p;
        assert_eq!(mclk, 49_000_000, "PLL3 must produce 49.0 MHz for SAI MCLK");
    }

    #[test]
    // PLL divisor variables are conventionally named m/n/p in datasheet formulas.
    #[allow(clippy::many_single_char_names)]
    fn pll3_vco_within_stm32h7_spec() {
        // STM32H7 PLL VCO must be 192–836 MHz (RM0433 §8.3.2)
        let hsi_hz: u64 = 64_000_000;
        let m = u64::from(SaiAudioConfig::pll3_m());
        let n = u64::from(SaiAudioConfig::pll3_n());
        let vco = hsi_hz / m * n;
        assert!(vco >= 192_000_000, "PLL3 VCO ({vco} Hz) must be >= 192 MHz");
        assert!(vco <= 836_000_000, "PLL3 VCO ({vco} Hz) must be <= 836 MHz");
    }

    #[test]
    fn pll3_mclk_error_within_1_percent() {
        let target = 49_152_000u64; // ideal 256 × 192000
        let actual = u64::from(SaiAudioConfig::actual_mclk_hz());
        let error_ppm = (target.abs_diff(actual) * 1_000_000) / target;
        assert!(
            error_ppm < 10_000, // < 1% = < 10000 ppm
            "MCLK frequency error must be < 1%, got {error_ppm} ppm"
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
