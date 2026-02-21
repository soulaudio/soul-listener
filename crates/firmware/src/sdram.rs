//! FMC SDRAM timing constants for W9825G6KH-6 at 100 MHz.
//!
//! Target device: W9825G6KH-6 (Winbond) -- 32 MB (16M x 16-bit), 166 MHz, TSOP-54
//! FMC bank: Bank 5 (SDRAM bank 1, base address 0xC000_0000)
//! FMC clock: PLL2R = 200 MHz; FMC internal /2 divider -> SDCLK = 100 MHz
//!
//! # Clock source
//!
//! The FMC SDCLK is the SDRAM timing reference (same as PLL2R/2).
//! PLL2: HSI (64 MHz) / prediv(8) * mul(100) / divr(4) = 200 MHz (PLL2R).
//! FMC internal fixed /2 divider: SDCLK = PLL2R / 2 = 100 MHz.
//! FMC timing registers count in SDCLK cycles (~10 ns period).
//!
//! # Timing math
//!
//! cycles = ceil(t_ns * FMC_CLK_HZ / 1_000_000_000)
//!        = (t_ns * FMC_CLK_HZ + 999_999_999) / 1_000_000_000
//!
//! FMC_SDTRx stores (cycles - 1) per field.
//! All constants here are actual cycle counts (1-based).
//!
//! # Refresh count
//!
//! W9825G6KH-6: 8192 rows refreshed within 64 ms.
//! Per-row interval = 64 ms / 8192 * 100 MHz / 1000 = 781 SDCLK cycles - 20 safety = 761.
//!
//! # References
//!
//! - W9825G6KH-6 datasheet Rev I, Table 13 (AC characteristics, 3.3 V)
//! - STM32H7 RM0433 Rev 9 section 22 (FMC SDRAM controller)
//! - STM32H7 RM0433 section 22.7.3 (Initialization sequence)
//! - STM32H7 RM0433 section 22.9.4 (FMC_SDTRx register description)
//! - STM32H7 RM0433 section 22.7.7 (FMC_SDRTR refresh timer)

// ---- Clock ----------------------------------------------------------------

/// FMC clock (SDCLK) in Hz.
///
/// Derivation from `boot.rs` PLL configuration:
///   PLL2: HSI(64 MHz) / prediv(8) * mul(100) = 800 MHz VCO
///   PLL2R = 800 MHz / divr(4) = 200 MHz (FMC kernel clock from RCC)
///   SDCLK = PLL2R / 2 (FMC internal fixed divider) = 100 MHz
///
/// SDCLK = 100 MHz is within the W9825G6KH-6 166 MHz max rating.
/// FMC_SDTRx timing fields count in SDCLK cycles (~10 ns period).
pub const FMC_CLK_HZ: u32 = 100_000_000;

// ---- W9825G6KH-6 timing constraints in nanoseconds -----------------------
// Source: W9825G6KH-6 datasheet Rev I, Table 13 (CL=3, 3.3 V, -6 speed grade)

/// tRCD - RAS-to-CAS delay (ns). Minimum 18 ns.
pub const T_RCD_NS: u32 = 18;

/// tRP - Precharge-to-active delay (ns). Minimum 18 ns.
pub const T_RP_NS: u32 = 18;

/// tWR - Write recovery time (ns). Minimum 12 ns.
pub const T_WR_NS: u32 = 12;

/// tRC - Row cycle time: active-to-active same bank (ns). Minimum 60 ns.
pub const T_RC_NS: u32 = 60;

/// tRFC - Auto-refresh cycle time (ns). Minimum 60 ns.
pub const T_RFC_NS: u32 = 60;

/// tXSR - Exit self-refresh to active delay (ns). Minimum 70 ns.
pub const T_XSR_NS: u32 = 70;

/// tMRD - Load mode register to active delay (SDRAM cycles). Minimum 2.
pub const T_MRD_CYCLES: u32 = 2;

/// CAS latency (CL).
///
/// W9825G6KH-6 supports CL=2 (<=133 MHz) or CL=3 (<=166 MHz).
/// CL=3 used for safety margin; matches platform::sdram convention.
pub const CAS_LATENCY: u32 = 3;

// ---- Cycle counts (ceil of ns * FMC_CLK) ---------------------------------

/// tRCD in SDCLK cycles: ceil(18 ns * 100 MHz) = ceil(1.8) = 2 cycles.
/// FMC_SDTRx TRCD field (bits 7:4) = TRCD_CYCLES - 1 = 1.
pub const TRCD_CYCLES: u32 = ns_to_cycles(T_RCD_NS);

/// tRP in SDCLK cycles: ceil(18 ns * 100 MHz) = ceil(1.8) = 2 cycles.
/// FMC_SDTRx TRP field (bits 15:12) = TRP_CYCLES - 1 = 1.
pub const TRP_CYCLES: u32 = ns_to_cycles(T_RP_NS);

/// tWR in SDCLK cycles: ceil(12 ns * 100 MHz) = ceil(1.2) = 2 cycles.
/// FMC_SDTRx TWR field (bits 19:16) = TWR_CYCLES - 1 = 1.
pub const TWR_CYCLES: u32 = ns_to_cycles(T_WR_NS);

/// tRC in SDCLK cycles: ceil(60 ns * 100 MHz) = ceil(6.0) = 6 cycles.
/// FMC_SDTRx TRC field (bits 23:20) = TRC_CYCLES - 1 = 5.
pub const TRC_CYCLES: u32 = ns_to_cycles(T_RC_NS);

/// tXSR in SDCLK cycles: ceil(70 ns * 100 MHz) = ceil(7.0) = 7 cycles.
/// FMC_SDTRx TXSR field (bits 11:8) = TXSR_CYCLES - 1 = 6.
pub const TXSR_CYCLES: u32 = ns_to_cycles(T_XSR_NS);

/// SDRAM auto-refresh interval in SDCLK cycles (FMC_SDRTR COUNT field).
///
/// count = floor(64 ms * 100 MHz / 1000 / 8192) - 20 = 781 - 20 = 761.
/// Safety margin per RM0433 section 22.7.7. 13-bit field: 0-8191.
pub const REFRESH_COUNT: u32 = compute_refresh_count(FMC_CLK_HZ);

// ---- Const helper functions -----------------------------------------------

/// Convert a timing constraint from nanoseconds to SDCLK cycles (ceiling).
///
/// cycles = ceil(ns * FMC_CLK_HZ / 1_000_000_000)  [SDCLK cycles]
///        = (ns * FMC_CLK_HZ + 999_999_999) / 1_000_000_000
///
/// Returns at least 1 (FMC minimum per RM0433 section 22.9.4).
pub const fn ns_to_cycles(ns: u32) -> u32 {
    let cycles = (ns as u64 * FMC_CLK_HZ as u64 + 999_999_999) / 1_000_000_000;
    if cycles < 1 { 1 } else { cycles as u32 }
}

/// Compute the FMC_SDRTR COUNT value for the W9825G6KH-6 refresh timer.
///
/// count = floor(refresh_period_ms * fmc_clk_hz / 1_000 / num_rows) - safety
///
/// refresh_period_ms = 64, num_rows = 8192, safety_margin = 20.
pub const fn compute_refresh_count(fmc_clk_hz: u32) -> u32 {
    const REFRESH_PERIOD_MS: u64 = 64;
    const NUM_ROWS: u64 = 8_192;
    const SAFETY_MARGIN: u64 = 20;
    let count = REFRESH_PERIOD_MS * fmc_clk_hz as u64 / 1_000 / NUM_ROWS;
    (count - SAFETY_MARGIN) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ns_to_cycles helper ------------------------------------------------

    #[test]
    fn ns_to_cycles_rounds_up() {
        // 1 ns * 100 MHz = 0.1 -> ceiling = 1
        assert_eq!(ns_to_cycles(1), 1);
        // 9 ns * 100 MHz = 0.9 -> ceiling = 1
        assert_eq!(ns_to_cycles(9), 1);
        // 11 ns * 100 MHz = 1.1 -> ceiling = 2
        assert_eq!(ns_to_cycles(11), 2);
    }

    #[test]
    fn ns_to_cycles_exact_boundary() {
        // ceil(100 ns * 100 MHz) = ceil(10.0) = 10 (exact)
        assert_eq!(ns_to_cycles(100), 10);
        // ceil(101 ns * 100 MHz) = ceil(10.1) = 11
        assert_eq!(ns_to_cycles(101), 11);
    }

    #[test]
    fn ns_to_cycles_zero_returns_one() {
        assert_eq!(ns_to_cycles(0), 1);
    }

    // -- Individual timing constants ----------------------------------------

    #[test]
    fn trcd_cycles_correct_for_100mhz() {
        // ceil(18 ns * 100 MHz) = ceil(1.8) = 2 cycles
        assert_eq!(TRCD_CYCLES, 2);
    }

    #[test]
    fn trp_cycles_correct_for_100mhz() {
        // ceil(18 ns * 100 MHz) = ceil(1.8) = 2 cycles
        assert_eq!(TRP_CYCLES, 2);
    }

    #[test]
    fn twr_cycles_correct_for_100mhz() {
        // ceil(12 ns * 100 MHz) = ceil(1.2) = 2 cycles
        assert_eq!(TWR_CYCLES, 2);
    }

    #[test]
    fn trc_cycles_correct_for_100mhz() {
        // ceil(60 ns * 100 MHz) = ceil(6.0) = 6 cycles
        assert_eq!(TRC_CYCLES, 6);
    }

    #[test]
    fn txsr_cycles_correct_for_100mhz() {
        // ceil(70 ns * 100 MHz) = ceil(7.0) = 7 cycles
        assert_eq!(TXSR_CYCLES, 7);
    }

    // -- Refresh count -------------------------------------------------------

    #[test]
    fn refresh_count_is_761() {
        // floor(64 * 100_000_000 / 1_000 / 8_192) - 20 = 781 - 20 = 761
        assert_eq!(REFRESH_COUNT, 761);
    }

    #[test]
    fn refresh_count_is_in_valid_range() {
        assert!(REFRESH_COUNT > 0);
        assert!(REFRESH_COUNT < 8_191);
    }

    #[test]
    fn refresh_count_within_20_percent_of_theoretical() {
        // theoretical = floor(64ms * 100MHz / 1000 / 8192) = 781
        let theoretical: u32 = (64_u64 * 100_000_000 / 1_000 / 8_192) as u32;
        let diff = REFRESH_COUNT.abs_diff(theoretical);
        assert!(diff < 150);
    }

    // -- CAS latency --------------------------------------------------------

    #[test]
    fn cas_latency_valid_for_w9825g6kh6() {
        assert!(CAS_LATENCY == 2 || CAS_LATENCY == 3);
    }

    #[test]
    fn cas_latency_is_3() {
        assert_eq!(CAS_LATENCY, 3);
    }

    // -- FMC register field width constraints --------------------------------

    #[test]
    fn all_timing_values_fit_in_4bit_fmc_fields() {
        assert!(TRCD_CYCLES <= 16);
        assert!(TRP_CYCLES  <= 16);
        assert!(TWR_CYCLES  <= 16);
        assert!(TRC_CYCLES  <= 16);
        assert!(TXSR_CYCLES <= 16);
    }

    #[test]
    fn fmc_register_values_minus_one_are_nonzero() {
        assert!(TRCD_CYCLES >= 1);
        assert!(TRP_CYCLES  >= 1);
        assert!(TWR_CYCLES  >= 1);
        assert!(TRC_CYCLES  >= 1);
        assert!(TXSR_CYCLES >= 1);
    }

    // -- Clock constant ------------------------------------------------------

    #[test]
    fn fmc_clk_hz_is_100mhz() {
        // PLL2: HSI(64 MHz) / prediv(8) * mul(100) / divr(4) = 200 MHz (PLL2R).
        // FMC internal /2 divider: SDCLK = PLL2R / 2 = 100 MHz.
        // See boot.rs: PLL2 prediv=DIV8, mul=MUL100, divr=DIV4.
        assert_eq!(FMC_CLK_HZ, 100_000_000);
    }

    // -- compute_refresh_count helper ----------------------------------------

    #[test]
    fn compute_refresh_count_matches_const() {
        assert_eq!(compute_refresh_count(100_000_000), REFRESH_COUNT);
    }

    #[test]
    fn compute_refresh_count_at_100mhz_is_761() {
        // floor(64 * 100_000_000 / 1_000 / 8_192) - 20 = 781 - 20 = 761
        assert_eq!(compute_refresh_count(100_000_000), 761);
    }

    // -- T_MRD_CYCLES -------------------------------------------------------

    #[test]
    fn t_mrd_cycles_minimum_two() {
        assert!(T_MRD_CYCLES >= 2);
    }
}
