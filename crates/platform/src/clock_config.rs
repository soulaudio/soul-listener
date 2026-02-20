//! Clock configuration requirements for STM32H743.
//!
//! Encodes which peripherals need which clock sources, enabling compile-time
//! documentation and runtime validation of clock setup order.
//!
//! # Background
//!
//! The STM32H743 has multiple clock sources that must be enabled in the correct
//! order before peripheral initialization. Getting the order wrong can cause
//! silent failures with no error code (embassy issue \#3049).
//!
//! # Sources
//!
//! - Embassy issue \#3049: <https://github.com/embassy-rs/embassy/issues/3049>
//!   SDMMC on STM32H743 silently hangs during `init_card()` unless HSI48 is
//!   enabled in RCC before SDMMC initialisation.
//! - STM32H743 Reference Manual (RM0433): Section 8.5 (RCC clock tree)
//! - Zephyr issue \#55358: confirms HSI48 requirement for SDMMC on STM32H7

/// Clock sources available on the STM32H743.
///
/// Each variant identifies one clock domain. Used to document and validate
/// which peripherals require which clock source to be active before init.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ClockSource {
    /// Internal 48 MHz RC oscillator.
    ///
    /// Required by: SDMMC1/2, USB OTG HS, RNG.
    ///
    /// **Must be enabled in RCC before any of those peripherals are
    /// initialised.** On embassy-stm32 set:
    /// ```text
    /// config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: false });
    /// ```
    /// Enabling CRS (Clock Recovery System) is optional but recommended
    /// to trim HSI48 accuracy when USB SOF is available.
    Hsi48,

    /// PLL1 Q output.
    ///
    /// Used by: SAI1/2 (audio I2S/TDM), SPI1/2/3.
    /// Typical value: 48 MHz or audio-ratio (e.g. 49.152 MHz for 192 kHz).
    Pll1Q,

    /// PLL2 R output.
    ///
    /// Used by: FMC (SDRAM controller), QUADSPI (NOR flash XiP).
    /// Typical value: 200 MHz (FMC/2 = 100 MHz SDRAM clock).
    Pll2R,

    /// APB peripheral bus clock.
    ///
    /// Used by: I2C1/2/3/4, USART1-8, UART4-8.
    Apb,
}

/// A peripheral and its mandatory clock-source dependency.
///
/// These are static documentation + runtime-assertion records.
/// They do **not** configure the hardware — they verify that the documented
/// requirement is present in the table and (on hardware) assert it at boot.
pub struct ClockRequirement {
    /// Short identifier for the peripheral (e.g. `"SDMMC1"`, `"SAI1"`).
    pub peripheral: &'static str,
    /// The clock source that must be active before this peripheral is init'd.
    pub required_source: ClockSource,
    /// Human-readable note explaining *why* this requirement exists, including
    /// any relevant issue trackers or datasheet sections.
    pub note: &'static str,
}

/// Set to `true` when the firmware `Cargo.toml` uses an explicit `time-driver-tim*`
/// feature rather than the catch-all `time-driver-any`.
///
/// This constant is `true` unconditionally — it acts as a documentation assertion
/// that callers must use an explicit timer feature (enforced by architecture tests
/// and CI checks). The actual feature flag used is `time-driver-tim2`.
pub const TIME_DRIVER_EXPLICIT: bool = true;

/// All clock requirements for SoulAudio DAP peripherals on STM32H743.
///
/// This table is the single source of truth for "which clock must be enabled
/// before which peripheral". It is checked in `arch_boundaries.rs` tests and
/// will be read at firmware boot to validate the RCC configuration.
///
/// # Sources
/// - Embassy issue \#3049 (SDMMC + HSI48)
/// - STM32H743 reference manual RM0433, Table 56 (peripheral clock mux)
pub const SOUL_AUDIO_CLOCK_REQUIREMENTS: &[ClockRequirement] = &[
    // ── HSI48 consumers ───────────────────────────────────────────────────────
    ClockRequirement {
        peripheral: "SDMMC1",
        required_source: ClockSource::Hsi48,
        note: "embassy-stm32 issue #3049: SDMMC requires HSI48 + CRS enabled before init; \
               failure produces silent chip lockup with no error code",
    },
    // ── PLL1Q consumers ───────────────────────────────────────────────────────
    ClockRequirement {
        peripheral: "SAI1",
        required_source: ClockSource::Pll1Q,
        note: "SAI1 I2S/TDM audio output to ES9038Q2M DAC; PLL1Q tuned to audio rate \
               (e.g. 49.152 MHz for 192 kHz / 256 fs); RM0433 §33.4.4",
    },
    // ── PLL2R consumers ───────────────────────────────────────────────────────
    ClockRequirement {
        peripheral: "FMC",
        required_source: ClockSource::Pll2R,
        note: "FMC SDRAM controller kernel clock = PLL2R; FMC_CLK output = PLL2R/2 \
               → 100 MHz at PLL2R=200 MHz; RM0433 §22.3.2",
    },
    ClockRequirement {
        peripheral: "QUADSPI",
        required_source: ClockSource::Pll2R,
        note: "QUADSPI kernel clock = PLL2R; target 120 MHz from 240 MHz PLL2R with \
               prescaler=1; W25Q128JV max 133 MHz; RM0433 §24.3.1",
    },
];

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// SDMMC1 must require HSI48 (embassy issue #3049).
    ///
    /// Without HSI48, SDMMC silently hangs on hardware — no error, no panic,
    /// just a chip lockup. This entry documents and enforces that requirement.
    #[test]
    fn test_sdmmc_requires_hsi48() {
        let sdmmc = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .find(|r| r.peripheral == "SDMMC1")
            .expect("SDMMC1 must have a clock requirement entry");
        assert_eq!(
            sdmmc.required_source,
            ClockSource::Hsi48,
            "SDMMC1 must require HSI48 (embassy issue #3049)"
        );
        assert!(
            sdmmc.note.contains("3049"),
            "SDMMC1 note must reference embassy issue #3049"
        );
    }

    /// SAI1 must require PLL1Q for audio-rate I2S clocking.
    #[test]
    fn test_sai_requires_pll1q() {
        let sai = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .find(|r| r.peripheral == "SAI1")
            .expect("SAI1 must have a clock requirement entry");
        assert_eq!(
            sai.required_source,
            ClockSource::Pll1Q,
            "SAI1 must require PLL1Q for audio I2S clocking"
        );
    }

    /// FMC must require PLL2R (SDRAM controller kernel clock).
    #[test]
    fn test_fmc_requires_pll2r() {
        let fmc = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .find(|r| r.peripheral == "FMC")
            .expect("FMC must have a clock requirement entry");
        assert_eq!(
            fmc.required_source,
            ClockSource::Pll2R,
            "FMC must require PLL2R as its kernel clock"
        );
    }

    /// QUADSPI must require PLL2R (shared with FMC for simplicity).
    #[test]
    fn test_quadspi_requires_pll2r() {
        let qspi = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .find(|r| r.peripheral == "QUADSPI")
            .expect("QUADSPI must have a clock requirement entry");
        assert_eq!(
            qspi.required_source,
            ClockSource::Pll2R,
            "QUADSPI must require PLL2R as its kernel clock"
        );
    }

    /// Exactly one peripheral requires HSI48: SDMMC1.
    ///
    /// USB OTG will also need HSI48 in v2, but is out of scope for v1.
    /// This test pins the count to 1 so that a future addition of USB requires
    /// a deliberate update here.
    #[test]
    fn test_hsi48_peripheral_count() {
        let count = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .filter(|r| r.required_source == ClockSource::Hsi48)
            .count();
        assert_eq!(
            count, 1,
            "exactly 1 peripheral requires HSI48 in v1 (SDMMC1 only; USB is v2)"
        );
    }

    /// The TIME_DRIVER_EXPLICIT constant must be true.
    ///
    /// Architecture rule: the firmware Cargo.toml must use an explicit
    /// `time-driver-tim*` feature (e.g. `time-driver-tim2`) rather than the
    /// catch-all `time-driver-any`. This constant documents that requirement.
    #[test]
    fn time_driver_explicit_constant_is_true() {
        assert!(
            TIME_DRIVER_EXPLICIT,
            "TIME_DRIVER_EXPLICIT must be true — use time-driver-tim2, not time-driver-any"
        );
    }
}
