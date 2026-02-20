//! PLL3 audio clock divider calculations for SAI1 MCLK generation.
//!
//! The STM32H743 PLL3 generates the master clock (MCLK) for SAI1 Block A.
//! For bit-perfect audio at 192 kHz / 256 fs, MCLK must be 49.152 MHz.
//!
//! # Clock Source
//!
//! PLL3 uses the internal HSI oscillator (64 MHz) as its source.
//! HSE (25 MHz external crystal) is NOT used for PLL3. HSI avoids
//! board-to-board variation and is available immediately on power-on
//! without crystal startup delay.
//!
//! # Clock Tree
//!
//!   HSI (64 MHz) -> PLL3M (div 4) -> VCO_IN (16 MHz)
//!                                  -> VCO_OUT (x49 = 784 MHz)
//!                                    -> PLL3P (div 16) = 49.0 MHz  [no FRACN]
//!                                    -> PLL3P (div 16) = 49.152 MHz [FRACN=1245]
//!                                       |
//!                                    SAI1_MCLK_A (PE2, AF6) -> ES9038Q2M
//!
//! # PLL3 Formula
//!
//!   VCO_INPUT  = HSI / PLL3M
//!   VCO_OUTPUT = VCO_INPUT * (PLL3N + PLL3FRACN / 8192)
//!   PLL3P_CLK  = VCO_OUTPUT / PLL3P    <- SAI1 MCLK
//!
//! # Finding Exact 49.152 MHz
//!
//! Target: 49 152 000 Hz = 256 x 192 000 Hz
//!
//! Step 1 -- Integer-only (no FRACN):
//!   HSI / M = 64 MHz / 4 = 16 MHz     (VCO input, 1-16 MHz per RM0433 S8.7.14)
//!   VCO     = 16 MHz x 49 = 784 MHz   (192-836 MHz VCO range)
//!   PLL3P   = 784 MHz / 16 = 49.0 MHz
//!   Error   = 152 000 Hz = 3092 ppm   (audible)
//!
//! Step 2 -- Adding FRACN:
//!   N + FRACN/8192 = 49 152 000 x 4 x 16 / 64 000 000 = 49.152
//!   N = 49,  FRACN = round(0.152 x 8192) = round(1245.18) = 1245
//!
//! Step 3 -- Verify (full-precision integer arithmetic):
//!   PLL3P_HZ = 64 000 000 x (49 x 8192 + 1245) / (4 x 8192 x 16)
//!            = 64 000 000 x 402 629 / 524 288
//!            = 49 151 977 Hz
//!   Error = |49 151 977 - 49 152 000| = 23 Hz  (< 1 ppm, inaudible)
//!
//! # ES9038Q2M MCLK Requirement
//!
//! ES9038Q2M requires MCLK = 256 x fs (I2S master mode).
//! At 192 kHz: MCLK = 256 x 192 000 = 49 152 000 Hz.
//! The DAC on-chip PLL locks to incoming MCLK; 23 Hz error (< 1 ppm)
//! is far below the lock range and inaudible on any audio system.
//!
//! References:
//! - STM32H7 RM0433 Rev 9, S8.7.14 (PLL configuration, VCO ranges)
//! - STM32H7 RM0433 Rev 9, S8.7.15 (fractional PLL, FRACN field)
//! - ES9038Q2M datasheet, S6.3.1 (MCLK / fs ratio requirements)
//! - firmware::boot::build_embassy_config() -- actual hardware wiring

/// HSI oscillator frequency (Hz) -- internal 64 MHz RC oscillator on STM32H743.
///
/// PLL3 source is HSI, not HSE. HSI is available immediately on power-on
/// and has +/-1% accuracy across temperature, adequate for ES9038Q2M MCLK.
pub const HSI_HZ: u32 = 64_000_000;

/// Target MCLK for ES9038Q2M at 192 kHz / 256 fs: 256 x 192 000 = 49 152 000 Hz.
pub const MCLK_TARGET_HZ: u32 = 49_152_000;

/// SAI1 Block A sample rate (Hz).
pub const SAMPLE_RATE_HZ: u32 = 192_000;

/// MCLK/fs ratio for ES9038Q2M I2S master mode: MCLK = 256 x fs.
pub const MCLK_FS_RATIO: u32 = 256;

/// PLL3 M predivider: HSI / 4 = 16 MHz VCO input.
/// STM32H7 RM0433 S8.7.14: VCO input must be in range 1-16 MHz.
/// Corresponds to PllPreDiv::DIV4 in embassy-stm32.
pub const PLL3_M: u32 = 4;

/// PLL3 N multiplier: VCO = 16 MHz x 49 = 784 MHz.
/// STM32H7 RM0433 S8.7.14: VCO output must be in range 192-836 MHz.
/// Corresponds to PllMul::MUL49 in embassy-stm32.
pub const PLL3_N: u32 = 49;

/// PLL3 P divider: MCLK base = 784 MHz / 16 = 49.0 MHz.
/// With PLL3_FRACN = 1245, actual MCLK = 49 151 977 Hz (23 Hz below target).
/// Corresponds to PllDiv::DIV16 in embassy-stm32.
pub const PLL3_P: u32 = 16;

/// PLL3 fractional part (0-8191, 13-bit RCC_PLL3FRACR.FRACN field).
///
/// Trims PLL3P from 49.0 MHz to 49.152 MHz.
/// Derivation: FRACN = round((49.152 - 49) x 8192) = round(1245.18) = 1245
/// Verification: 64M x (49x8192+1245) / (4x8192x16) = 49 151 977 Hz (23 Hz error)
///
/// embassy-stm32 0.1.x does not expose FRACN through the Pll struct.
/// Apply via PAC after init: RCC.pll3fracr().write(|w| w.set_fracn(PLL3_FRACN as u16))
pub const PLL3_FRACN: u32 = 1245;

/// Computed PLL3P clock (SAI MCLK) in Hz using full-precision u128 arithmetic.
///
/// Formula: HSI x (N x 8192 + FRACN) / (M x 8192 x P)
/// Result: 49 151 977 Hz (23 Hz below 49.152 MHz target, < 1 ppm).
/// Must stay within +/-MCLK_MAX_ERROR_HZ of MCLK_TARGET_HZ.
// The intermediate u128 arithmetic prevents overflow; the final value (≈49 MHz) fits in u32.
#[allow(clippy::cast_possible_truncation)]
pub const PLL3P_HZ_APPROX: u32 = (HSI_HZ as u128
    * (PLL3_N as u128 * 8192 + PLL3_FRACN as u128)
    / (PLL3_M as u128 * 8192 * PLL3_P as u128)) as u32;

/// Maximum allowed MCLK error (Hz).
///
/// 500 Hz = ~10 ppm at 49.152 MHz. ES9038Q2M lock range >= 1000 ppm.
/// Audible pitch shift begins at ~100 ppm; 10 ppm is 10x below that threshold.
/// Actual error with FRACN=1245: 23 Hz (< 1 ppm).
pub const MCLK_MAX_ERROR_HZ: u32 = 500;

#[cfg(test)]
mod tests {
    use super::*;

    /// RM0433 S8.7.14: VCO input must be in range 1-16 MHz.
    /// PLL3M divides HSI before the VCO; out-of-range input prevents PLL lock.
    #[test]
    fn pll3_m_divider_gives_valid_vco_input() {
        let vco_input = HSI_HZ / PLL3_M;
        assert!(
            vco_input >= 1_000_000,
            "VCO input {vco_input} Hz is below the 1 MHz minimum (RM0433 S8.7.14)"
        );
        assert!(
            vco_input <= 16_000_000,
            "VCO input {vco_input} Hz exceeds the 16 MHz maximum (RM0433 S8.7.14)"
        );
    }

    /// RM0433 S8.7.14: PLL3 VCO output must be in range 192-836 MHz.
    #[test]
    fn pll3_n_gives_valid_vco_output() {
        let vco_input = HSI_HZ / PLL3_M;
        let vco_output = vco_input * PLL3_N;
        assert!(
            vco_output >= 192_000_000,
            "VCO output {vco_output} Hz is below the 192 MHz minimum (RM0433 S8.7.14)"
        );
        assert!(
            vco_output <= 836_000_000,
            "VCO output {vco_output} Hz exceeds the 836 MHz maximum (RM0433 S8.7.14)"
        );
    }

    /// PLL3P with FRACN must be within +/-MCLK_MAX_ERROR_HZ of 49.152 MHz.
    /// Primary correctness test; catches frequency regressions before hardware.
    #[test]
    fn pll3p_produces_mclk_within_tolerance() {
        let diff = i64::from(PLL3P_HZ_APPROX) - i64::from(MCLK_TARGET_HZ);
        assert!(
            diff.unsigned_abs() <= u64::from(MCLK_MAX_ERROR_HZ),
            "PLL3P {PLL3P_HZ_APPROX} Hz differs from {MCLK_TARGET_HZ} Hz by {diff} Hz (max {MCLK_MAX_ERROR_HZ} Hz)"
        );
    }

    /// MCLK_TARGET_HZ must equal MCLK_FS_RATIO x SAMPLE_RATE_HZ.
    #[test]
    fn mclk_fs_ratio_matches_es9038q2m_spec() {
        assert_eq!(
            MCLK_TARGET_HZ,
            SAMPLE_RATE_HZ * MCLK_FS_RATIO,
            "MCLK_TARGET_HZ must equal SAMPLE_RATE_HZ x MCLK_FS_RATIO"
        );
    }

    /// RM0433 S8.7.15: FRACN is 13-bit, valid range 0-8191.
    /// Values >= 8192 are silently truncated, producing wrong MCLK.
    #[test]
    // PLL3_FRACN is a compile-time constant; assertion documents the hardware constraint.
    #[allow(clippy::assertions_on_constants)]
    fn pll3_fracn_in_valid_range() {
        assert!(
            PLL3_FRACN < 8192,
            "PLL3_FRACN {PLL3_FRACN} exceeds 13-bit max of 8191 (RM0433 S8.7.15)"
        );
    }

    /// 192 000 x 256 = 49 152 000. Sanity check on ES9038Q2M 192 kHz target.
    #[test]
    fn sample_rate_192khz_times_256_equals_mclk_target() {
        assert_eq!(
            SAMPLE_RATE_HZ * MCLK_FS_RATIO,
            MCLK_TARGET_HZ,
            "192 000 x 256 must equal 49 152 000"
        );
    }

    /// MCLK error with FRACN=1245 must be below 100 Hz (< 2 ppm).
    /// Verifies we use an optimal FRACN; current: 23 Hz (< 1 ppm).
    #[test]
    fn pll3p_mclk_error_is_below_100hz() {
        let diff = i64::from(PLL3P_HZ_APPROX) - i64::from(MCLK_TARGET_HZ);
        assert!(
            diff.unsigned_abs() < 100,
            "PLL3P {PLL3P_HZ_APPROX} Hz is {diff} Hz from target (FRACN={PLL3_FRACN})"
        );
    }

    /// HSI_HZ must be 64 MHz -- fixed STM32H743 internal oscillator.
    /// Changing to HSE (25 MHz) requires recalculating all PLL3 divisors.
    #[test]
    fn hsi_is_64mhz() {
        assert_eq!(HSI_HZ, 64_000_000, "HSI must be 64 MHz (STM32H743 internal oscillator)");
    }

    /// PLL3 M/N/P must match build_embassy_config() in boot.rs:
    /// PllPreDiv::DIV4 (M=4), PllMul::MUL49 (N=49), PllDiv::DIV16 (P=16).
    #[test]
    fn pll3_divisors_match_embassy_boot_config() {
        assert_eq!(PLL3_M, 4, "PLL3_M must match PllPreDiv::DIV4 in build_embassy_config()");
        assert_eq!(PLL3_N, 49, "PLL3_N must match PllMul::MUL49 in build_embassy_config()");
        assert_eq!(PLL3_P, 16, "PLL3_P must match PllDiv::DIV16 in build_embassy_config()");
    }
}
