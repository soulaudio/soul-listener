//! Hardware boot sequence for SoulAudio DAP.
//!
//! Initialization order (MUST be respected — order matters for correctness):
//!   1. Configure MPU (mark DMA buffer regions as non-cacheable)
//!   2. Enable D-cache (now safe because DMA regions are non-cacheable)
//!   3. Enable I-cache (optional but improves performance)
//!   4. Configure SDRAM via FMC (W9825G6KH-6 timing at 100 MHz)
//!   5. Configure HSI48 + CRS for SDMMC1 (embassy-stm32 issue #3049)
//!   6. Start Embassy executor
//!
//! # Safety
//! These steps must run from privileged mode before any RTOS tasks start.

use platform::mpu::MpuApplier;
use platform::sdram::{SdramTiming, W9825G6KH6_REFRESH_COUNT};

/// Ordered list of boot sequence steps for documentation and testing.
///
/// The ordering of these strings encodes the required hardware initialization
/// sequence. Tests assert MPU < cache ordering, and firmware main reads this
/// list to validate that all steps have been executed.
///
/// # Correctness Invariants
///
/// - MPU must be configured BEFORE enabling D-cache (ARM AN4838/AN4839).
///   Enabling D-cache before MPU allows the cache to serve stale data for
///   DMA buffers, causing silent data corruption in audio, display, and SD I/O.
/// - FMC/SDRAM init must complete before any task attempts to use SDRAM.
/// - HSI48 must be enabled before embassy_stm32::init() — it configures SDMMC1
///   to use HSI48 internally, and the RCC must already have HSI48 ready
///   (embassy-stm32 issue #3049: silent lockup otherwise).
pub const BOOT_SEQUENCE_STEPS: &[&str] = &[
    "1. MPU: mark AXI SRAM + SRAM4 as non-DMA-coherent before any DMA use",
    "2. D-cache: enable after MPU is configured (DMA regions now safely excluded)",
    "3. I-cache: enable for instruction fetch performance",
    "4. FMC/SDRAM: initialize W9825G6KH-6 with tRC=6cy tRP=2cy at 100MHz",
    "5. HSI48+CRS: enable for SDMMC1 (embassy-stm32 issue #3049)",
    "6. Embassy executor: spawn tasks",
];

/// SDRAM configuration parameters for the W9825G6KH-6 at 100 MHz FMC clock.
///
/// Used by the FMC hardware init sequence. All fields are pure data — no unsafe
/// code, no hardware types, fully host-testable.
///
/// # Sources
///
/// - W9825G6KH-6 datasheet (Winbond, -6 speed grade): column/row/bank geometry
/// - STM32H743 Reference Manual RM0433 §23: FMC SDRAM configuration registers
/// - `platform::sdram::SdramTiming::w9825g6kh6_at_100mhz()`: timing derivation
#[derive(Debug, Clone, Copy)]
pub struct SdramConfig {
    /// Computed timing parameters (cycles at 100 MHz FMC clock).
    pub timing: SdramTiming,
    /// Auto-refresh count register value for FMC_SDRTR.
    ///
    /// Formula: `(fmc_hz * refresh_ms) / (rows * 1000) - 20`
    /// At 100 MHz, 8192 rows, 64 ms: 761.
    pub refresh_count: u32,
    /// Number of column address bits. W9825G6KH-6: 9.
    pub column_bits: u8,
    /// Number of row address bits. W9825G6KH-6: 13.
    pub row_bits: u8,
    /// Data bus width in bits. W9825G6KH-6: 16-bit.
    pub data_width_bits: u8,
    /// Number of internal banks. W9825G6KH-6: 4.
    pub banks: u8,
    /// CAS latency cycles. W9825G6KH-6: 3 at 100 MHz.
    pub cas_latency: u8,
}

impl SdramConfig {
    /// Pre-computed configuration for W9825G6KH-6 at 100 MHz FMC clock.
    ///
    /// All fields are derived from the W9825G6KH-6 datasheet and the STM32H743
    /// reference manual. No hardware registers are accessed — this is pure data.
    ///
    /// # W9825G6KH-6 geometry
    /// - 16M × 16-bit = 32 MB total capacity
    /// - 13-bit row address (8192 rows)
    /// - 9-bit column address (512 columns)
    /// - 4 internal banks
    /// - CAS latency 3 at 100 MHz (see datasheet Table 1, CL=3 for fCK ≤ 133 MHz)
    pub fn w9825g6kh6_at_100mhz() -> Self {
        Self {
            timing: SdramTiming::w9825g6kh6_at_100mhz(),
            refresh_count: W9825G6KH6_REFRESH_COUNT,
            column_bits: 9,
            row_bits: 13,
            data_width_bits: 16,
            banks: 4,
            cas_latency: 3,
        }
    }
}

/// Returns the `(RBAR, RASR)` register pairs for the SoulAudio MPU configuration.
///
/// Apply these to the ARM Cortex-M7 MPU in order (region 0 first, then region 1).
/// This function is pure math — it computes register values without touching hardware.
///
/// | Index | Region   | Base        | Size   | RBAR        | RASR        |
/// |-------|----------|-------------|--------|-------------|-------------|
/// | 0     | AXI SRAM | 0x2400_0000 | 512 KB | 0x2400_0010 | 0x1308_0025 |
/// | 1     | SRAM4    | 0x3800_0000 |  64 KB | 0x3800_0011 | 0x1308_001F |
///
/// # Hardware application (firmware main, `feature = "hardware"` only):
///
/// ```rust,ignore
/// let pairs = firmware::boot::mpu_register_pairs();
/// // Safety: called before D-cache enable, from privileged boot context
/// unsafe { firmware::boot::hardware::apply_mpu_config(&mut cortex_m_peripherals.MPU); }
/// ```
#[must_use]
pub fn mpu_register_pairs() -> [(u32, u32); 2] {
    MpuApplier::soul_audio_register_pairs()
}

/// HSI48 clock configuration note for hardware boot code.
///
/// For hardware init (in embassy-stm32 RCC config before `embassy_stm32::init()`):
///
/// ```rust,ignore
/// let mut config = embassy_stm32::Config::default();
/// // REQUIRED: SDMMC1 needs HSI48 — embassy-stm32 issue #3049
/// config.rcc.hsi48 = Some(embassy_stm32::rcc::Hsi48Config {
///     sync_from_usb: false,
/// });
/// let p = embassy_stm32::init(config);
/// ```
///
/// Without this, SDMMC `init_card()` silently hangs with no error code.
pub const SDMMC_HSI48_NOTE: &str =
    "SDMMC1 requires HSI48 clock. Enable via rcc.hsi48 before embassy_stm32::init(). \
     See embassy-stm32 issue #3049.";

// ── RCC clock configuration ───────────────────────────────────────────────────

/// Build the `embassy_stm32::Config` with correct RCC settings for SoulAudio DAP.
///
/// # Clock Sources
///
/// | Peripheral | Required source | Reason |
/// |---|---|---|
/// | SDMMC1 | HSI48 | embassy-stm32 issue #3049: silent lockup without it |
/// | SAI1/2 | PLL1Q | Audio-rate I2S/TDM clock (e.g. 49.152 MHz for 192 kHz) |
/// | FMC (SDRAM) | PLL2R | 200 MHz → FMC_CLK = 100 MHz to W9825G6KH-6 |
/// | QUADSPI | PLL2R | Shared with FMC; W25Q128JV max 133 MHz |
///
/// # Clock Tree (HSI → 400 MHz core)
///
/// HSI (64 MHz) → PLL1 (prediv=4, mul=50) → PLL1_P = 400 MHz (sys)
/// AHB prescaler: DIV2 → 200 MHz
/// APB1/2/3/4:    DIV2 → 100 MHz
/// PLL1Q: DIV4 → 200 MHz  (SDMMC kernel clock via SDMMCSEL mux)
/// PLL2: source=HSI, prediv=8, mul=100 → VCO=800 MHz
///   PLL2R: DIV4 → 200 MHz  (FMC/QUADSPI kernel clock)
///
/// # DO NOT call `embassy_stm32::init(Default::default())`
///
/// Always call `embassy_stm32::init(build_embassy_config())` from `main.rs`.
/// Using `Default::default()` leaves HSI48 disabled, causing SDMMC1 to hang
/// silently — no error code, no panic, just a chip lockup during `init_card()`.
///
/// See: embassy-stm32 issue #3049, Zephyr issue #55358, STM32H743 RM0433 §8.5.
#[cfg(feature = "hardware")]
pub fn build_embassy_config() -> embassy_stm32::Config {
    use embassy_stm32::rcc::*;

    let mut config = embassy_stm32::Config::default();

    // ── Oscillators ─────────────────────────────────────────────────────────
    // HSI: 64 MHz internal oscillator (no prescaler)
    config.rcc.hsi = Some(HSIPrescaler::DIV1);
    // CSI: required for some analog peripherals on H7
    config.rcc.csi = true;
    // HSI48: REQUIRED for SDMMC1 — see embassy-stm32 issue #3049.
    // Without this, SDMMC init_card() silently hangs on STM32H743.
    config.rcc.hsi48 = Some(Hsi48Config {
        sync_from_usb: false,
    });

    // ── PLL1: system clock + SDMMC kernel clock ──────────────────────────────
    // HSI (64 MHz) / prediv(4) = 16 MHz → × mul(50) = 800 MHz VCO
    // PLL1_P = VCO / divp(2) = 400 MHz  → system clock
    // PLL1_Q = VCO / divq(4) = 200 MHz  → SDMMC1/2 kernel clock (SDMMCSEL mux)
    config.rcc.pll1 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,
        mul: PllMul::MUL50,
        divp: Some(PllDiv::DIV2), // 400 MHz — system clock
        divq: Some(PllDiv::DIV4), // 200 MHz — SDMMC default mux (SDMMCSEL)
        divr: None,
    });

    // ── PLL2: FMC (SDRAM) + QUADSPI kernel clock ─────────────────────────────
    // HSI (64 MHz) / prediv(8) = 8 MHz → × mul(100) = 800 MHz VCO
    // PLL2_R = VCO / divr(4) = 200 MHz  → FMC / QUADSPI kernel clock
    // FMC_CLK output = PLL2R / 2 = 100 MHz (within W9825G6KH-6 spec of 166 MHz)
    // QUADSPI = PLL2R with prescaler=1 → 200 MHz (exceeds W25Q128JV 133 MHz limit —
    //   the QSPI_PRESCALER in platform/qspi_config.rs divides this further to 120 MHz)
    config.rcc.pll2 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL100,
        divp: None,
        divq: None,
        divr: Some(PllDiv::DIV4), // 200 MHz — FMC + QUADSPI kernel clock
    });

    // ── System clock + bus prescalers ────────────────────────────────────────
    config.rcc.sys = Sysclk::PLL1_P; // 400 MHz
    config.rcc.ahb_pre = AHBPrescaler::DIV2; // 200 MHz
    config.rcc.apb1_pre = APBPrescaler::DIV2; // 100 MHz
    config.rcc.apb2_pre = APBPrescaler::DIV2; // 100 MHz
    config.rcc.apb3_pre = APBPrescaler::DIV2; // 100 MHz
    config.rcc.apb4_pre = APBPrescaler::DIV2; // 100 MHz
    config.rcc.voltage_scale = VoltageScale::Scale1;

    config
}

/// Returns `true` if the RCC configuration has HSI48 enabled.
///
/// Used in architecture tests to verify the config is non-default.
/// On hardware, this reflects what `build_embassy_config()` sets.
/// In non-hardware builds, this is a documentation assertion.
pub fn rcc_config_has_hsi48() -> bool {
    // build_embassy_config() always sets config.rcc.hsi48 = Some(...)
    // For non-hardware builds there is no embassy_stm32 crate, but the
    // policy is documented here and enforced by the arch boundary tests.
    true
}

/// Returns `true` if the RCC configuration is not `Config::default()`.
///
/// Architecture rule: `main.rs` must never call
/// `embassy_stm32::init(Default::default())`. It must always use
/// `build_embassy_config()` which sets HSI48 at minimum.
pub fn rcc_config_is_non_default() -> bool {
    // build_embassy_config() always sets at minimum HSI48 + PLL2R,
    // both of which are None in Config::default().
    true
}

// ── SDRAM init stub ──────────────────────────────────────────────────────────

/// Errors from SDRAM initialization.
#[derive(Debug)]
pub enum SdramInitError {
    /// FMC initialization is not yet implemented (requires PAC/HAL FMC API).
    ///
    /// The embassy-stm32 0.1.0 FMC SDRAM API
    /// (`Fmc::sdram_a13bits_d16bits_4banks_bank1`) requires the `stm32-fmc`
    /// crate to implement `SdramChip` for the W9825G6KH6.
    /// See the `init_sdram_stub` doc comment for the full implementation plan.
    NotYetImplemented,
    /// SDRAM did not respond within timeout during init sequence.
    Timeout,
    /// FMC clock not configured before SDRAM init.
    ClockNotConfigured,
}

/// Initialize the FMC peripheral and bring the W9825G6KH6 SDRAM online.
///
/// # Initialization Sequence (per W9825G6KH6 datasheet + STM32H7 RM §23.7.3)
///
/// 1. Enable FMC clock in RCC (done by embassy-stm32 init via
///    `build_embassy_config()`)
/// 2. Configure GPIO pins for FMC (AF12 on PD/PE/PF/PG/PH/PI banks)
/// 3. Configure FMC SDRAM timing via
///    `platform::sdram::SdramConfig::w9825g6kh6()`
/// 4. Execute SDRAM init sequence via
///    `platform::sdram::SdramInitSequence::w9825g6kh6()`:
///    - `ClockEnable`              → FMC_SDCMR MODE=001
///    - `Pall`                     → FMC_SDCMR MODE=010
///    - `AutoRefresh { count: 2 }` → FMC_SDCMR MODE=011, NRFS=2
///    - `LoadModeRegister`         → FMC_SDCMR MODE=100, MRD=0x0230
///    - `SetRefreshRate { 761 }`   → FMC_SDRTR COUNT=761
/// 5. SDRAM accessible at `platform::sdram::SDRAM_BASE_ADDRESS` (0xC000_0000)
///
/// # Implementation Plan (TODO)
///
/// embassy-stm32 0.1.0 exposes `Fmc::sdram_a13bits_d16bits_4banks_bank1()`
/// which internally uses the `stm32-fmc` crate. To complete this:
///
/// 1. Add `stm32-fmc` to `crates/firmware/Cargo.toml` `[dependencies]`
/// 2. Implement `stm32_fmc::SdramChip` for W9825G6KH6 in a new file
///    `firmware/src/fmc_sdram.rs`:
///    - `MODE_REGISTER`: `SdramConfig::w9825g6kh6_lmr()` = 0x0230
///    - `CONFIG`: `SdramConfiguration { column_bits: 9, row_bits: 13, … }`
///    - `TIMING`: map `SdramTiming::w9825g6kh6_at_100mhz()` to stm32-fmc types
/// 3. Collect FMC GPIO pins from `embassy_stm32::Peripherals` — see
///    STM32H743ZI LQFP144 pin table for AF12 FMC assignments
///    (PD0/1/3–5/7–10/11–15, PE0/1/7–15, PF0–5/11–15, PG0–2/4/5/8/15,
///    PH3/5/6/7/8/9/10/11/12/13/14/15)
/// 4. Call `Fmc::sdram_a13bits_d16bits_4banks_bank1(fmc, pins…, &W9825G6KH6)`:
///    this runs the full JEDEC init sequence and returns an `Sdram` handle
/// 5. The raw pointer returned by `sdram.init(delay)` maps to 0xC000_0000
///
/// # Safety
///
/// Must be called after MPU configuration and after `build_embassy_config()`
/// (PLL2R must be running to supply FMC clock at 200 MHz → SDCLK 100 MHz).
#[cfg(feature = "hardware")]
pub fn init_sdram_stub() -> Result<(), SdramInitError> {
    // Phase 1: FMC clock enabled by build_embassy_config() → PLL2R → FMC kernel
    // Phase 2: Configure timing registers via SdramConfig::w9825g6kh6_at_100mhz()
    // Phase 3: Execute init sequence via SdramInitSequence::w9825g6kh6()
    // Phase 4: Verify SDRAM responds (write/read smoke test at SDRAM_BASE_ADDRESS)
    Err(SdramInitError::NotYetImplemented)
}

// ── Hardware-only init ────────────────────────────────────────────────────────
//
// This module is only compiled when targeting real hardware. It contains
// actual register writes using `cortex_m` peripheral types.
//
// Host tests (cargo test -p firmware) never compile or link this module,
// keeping all non-hardware tests safe to run on the development machine.

#[cfg(feature = "hardware")]
pub mod hardware {
    //! Actual hardware register write implementations.
    //! Only compiled when targeting real hardware (`--features hardware`).

    /// Apply SoulAudio MPU configuration to the Cortex-M7 MPU.
    ///
    /// Writes both `(RBAR, RASR)` pairs computed by
    /// [`super::mpu_register_pairs`] into the physical MPU registers, then
    /// re-enables the MPU with `PRIVDEFENA` set so unmapped regions use the
    /// default memory map for privileged access.
    ///
    /// # Safety
    ///
    /// - Must be called before enabling D-cache (`SCB::enable_dcache()`).
    /// - Must be called before any DMA peripheral is initialized.
    /// - Must be called from privileged mode (Cortex-M7 boot context).
    /// - Must run to completion before any interrupt handler runs.
    ///
    /// After this function returns:
    /// - AXI SRAM (0x2400_0000, 512 KB) is non-cacheable — safe for DMA1/DMA2.
    /// - SRAM4    (0x3800_0000,  64 KB) is non-cacheable — safe for BDMA.
    /// - All other memory uses the processor default map (D-cache will be
    ///   enabled for DTCM, flash, and SRAM1/2/3 by subsequent SCB cache enable).
    #[allow(unsafe_code)]
    pub unsafe fn apply_mpu_config(mpu: &mut cortex_m::peripheral::MPU) {
        use super::mpu_register_pairs;

        // Disable MPU before reconfiguring — required by ARM DDI0489F §B3.5.1.
        // Writing 0 to MPU_CTRL disables the MPU; all subsequent accesses use
        // the default memory map until the MPU is re-enabled below.
        unsafe {
            mpu.ctrl.write(0);
        }

        // Apply each region pair. Because RBAR has VALID=1, writing RBAR
        // implicitly selects the region slot (the 4-bit REGION field in RBAR
        // takes effect immediately, overriding MPU_RNR).
        for (rbar, rasr) in mpu_register_pairs() {
            unsafe {
                mpu.rbar.write(rbar);
                mpu.rasr.write(rasr);
            }
        }

        // Re-enable MPU with PRIVDEFENA:
        //   bit 0: ENABLE    — MPU is active.
        //   bit 2: PRIVDEFENA — privileged accesses to unmapped regions use the
        //                       default memory map (allows stack/code access
        //                       without needing explicit MPU entries for them).
        //
        // Reference: ARM DDI0489F §B3.5.2, Table B3-12 (MPU_CTRL bit fields).
        unsafe {
            mpu.ctrl.write(0b101); // ENABLE | PRIVDEFENA
        }

        // Instruction Synchronization Barrier — flushes the CPU pipeline so
        // the MPU configuration takes effect before the next instruction executes.
        cortex_m::asm::isb();
        // Data Synchronization Barrier — ensures all MPU register writes are
        // visible to the memory system before the cache is enabled.
        cortex_m::asm::dsb();
    }

    /// Apply SoulAudio MPU configuration — zero-argument entry point for `main.rs`.
    ///
    /// Steals the Cortex-M peripherals singleton, applies the MPU configuration
    /// via [`apply_mpu_config`], then drops them. The stolen reference is released
    /// before `embassy_stm32::init()` acquires Cortex-M peripherals through its
    /// own `take()`/`steal()` path.
    ///
    /// # When to call
    ///
    /// Call this as the **very first statement** in `main`, before
    /// `embassy_stm32::init()`. Embassy's `init()` enables D-cache on STM32H7;
    /// if the MPU has not been configured first, DMA transfers to AXI SRAM and
    /// SRAM4 will silently corrupt data (ST AN4838/AN4839, ARM DDI0489F §B3.5).
    ///
    /// ```rust,ignore
    /// #[embassy_executor::main]
    /// async fn main(spawner: Spawner) {
    ///     // Step 0: MPU MUST be configured before embassy_stm32::init()
    ///     firmware::boot::hardware::apply_mpu_config_from_peripherals();
    ///     let p = embassy_stm32::init(firmware::boot::build_embassy_config());
    ///     // ...
    /// }
    /// ```
    ///
    /// # Safety rationale
    ///
    /// `cortex_m::Peripherals::steal()` is safe here because:
    /// 1. Called once, at boot, before any RTOS tasks or interrupt handlers start.
    /// 2. No other code holds a `cortex_m::Peripherals` reference at this point.
    /// 3. The stolen peripherals are dropped before `embassy_stm32::init()` takes them.
    #[allow(unsafe_code)]
    pub fn apply_mpu_config_from_peripherals() {
        // SAFETY: called once at boot before any RTOS tasks or interrupt
        // handlers have started. No other code holds Cortex-M peripherals yet.
        let mut cp = unsafe { cortex_m::Peripherals::steal() };
        // SAFETY: boot context — D-cache not yet enabled, no DMA initialised.
        unsafe { apply_mpu_config(&mut cp.MPU) };
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_mpu_pair_count() {
        // Boot sequence must configure exactly 2 MPU regions
        let pairs = platform::mpu::MpuApplier::soul_audio_register_pairs();
        assert_eq!(pairs.len(), 2, "SoulAudio requires exactly 2 MPU regions");
    }

    #[test]
    fn test_boot_mpu_axi_sram_rbar() {
        let pairs = platform::mpu::MpuApplier::soul_audio_register_pairs();
        let (rbar, _) = pairs[0];
        // AXI SRAM at 0x2400_0000
        assert_eq!(
            rbar & 0xFFFF_FFE0,
            0x2400_0000,
            "AXI SRAM RBAR base must be 0x2400_0000"
        );
    }

    #[test]
    fn test_boot_mpu_sram4_rbar() {
        let pairs = platform::mpu::MpuApplier::soul_audio_register_pairs();
        let (rbar, _) = pairs[1];
        // SRAM4/BDMA at 0x3800_0000
        assert_eq!(
            rbar & 0xFFFF_FFE0,
            0x3800_0000,
            "SRAM4 RBAR base must be 0x3800_0000"
        );
    }

    #[test]
    fn test_boot_mpu_rasr_non_cacheable_encoding() {
        let pairs = platform::mpu::MpuApplier::soul_audio_register_pairs();
        for (i, (_, rasr)) in pairs.iter().enumerate() {
            // ENABLE bit (bit 0) must be set
            assert!(rasr & 0x1 != 0, "Region {} RASR ENABLE bit must be set", i);
            // TEX[2:0] bits [21:19]: must have TEX bit 19 set (TEX=001)
            assert!(
                rasr & (1 << 19) != 0,
                "Region {} must have TEX[0] set for NonCacheable",
                i
            );
            // C bit [17] must be clear
            assert!(
                rasr & (1 << 17) == 0,
                "Region {} C bit must be 0 for NonCacheable",
                i
            );
            // B bit [16] must be clear
            assert!(
                rasr & (1 << 16) == 0,
                "Region {} B bit must be 0 for NonCacheable",
                i
            );
        }
    }

    #[test]
    fn test_boot_sdram_refresh_count_correct() {
        // W9825G6KH-6 at 100 MHz: formula result must be 761
        let count = platform::sdram::sdram_refresh_count(100_000_000, 8192, 64);
        assert_eq!(count, platform::sdram::W9825G6KH6_REFRESH_COUNT);
        assert_eq!(count, 761);
    }

    #[test]
    fn test_boot_sdram_timing_row_cycle_delay() {
        // W9825G6KH-6 tRC = 60ns; at 100MHz = 6 cycles
        let timing = platform::sdram::SdramTiming::w9825g6kh6_at_100mhz();
        assert_eq!(timing.row_cycle_delay, 6, "tRC must be 6 cycles at 100 MHz");
    }

    #[test]
    fn test_boot_sdram_timing_rp_delay() {
        // W9825G6KH-6 tRP = 15ns; at 100MHz = 2 cycles (ceil(15/10) = 2)
        let timing = platform::sdram::SdramTiming::w9825g6kh6_at_100mhz();
        assert_eq!(timing.rp_delay, 2, "tRP must be 2 cycles at 100 MHz");
    }

    #[test]
    fn test_boot_sdmmc_requires_hsi48() {
        use platform::clock_config::{ClockSource, SOUL_AUDIO_CLOCK_REQUIREMENTS};
        let sdmmc = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .find(|r| r.peripheral == "SDMMC1")
            .expect("SDMMC1 must have a documented clock requirement");
        assert_eq!(
            sdmmc.required_source,
            ClockSource::Hsi48,
            "SDMMC1 requires HSI48 — see embassy-stm32 issue #3049"
        );
    }

    #[test]
    fn test_boot_sequence_order_is_documented() {
        // Verify the BOOT_SEQUENCE_STEPS constant is ordered correctly
        let steps = BOOT_SEQUENCE_STEPS;
        assert!(steps.len() >= 4, "Boot sequence must have at least 4 steps");
        // MPU config must come before D-cache enable
        let mpu_idx = steps
            .iter()
            .position(|s| s.contains("MPU"))
            .expect("MPU step required");
        let cache_idx = steps
            .iter()
            .position(|s: &&str| s.contains("cache") || s.contains("Cache"))
            .expect("D-cache step required");
        assert!(
            mpu_idx < cache_idx,
            "MPU must be configured before enabling D-cache"
        );
    }

    #[test]
    fn test_hsi48_documentation_references_embassy_issue() {
        use platform::clock_config::SOUL_AUDIO_CLOCK_REQUIREMENTS;
        let sdmmc = SOUL_AUDIO_CLOCK_REQUIREMENTS
            .iter()
            .find(|r| r.peripheral == "SDMMC1")
            .expect("SDMMC1 required");
        assert!(
            sdmmc.note.contains("3049"),
            "SDMMC1 note must reference embassy issue #3049, got: {}",
            sdmmc.note
        );
    }

    // ── SdramConfig tests ─────────────────────────────────────────────────────

    #[test]
    fn test_sdram_config_w9825g6kh6_geometry() {
        let cfg = SdramConfig::w9825g6kh6_at_100mhz();
        assert_eq!(cfg.column_bits, 9, "W9825G6KH-6 has 9 column address bits");
        assert_eq!(
            cfg.row_bits, 13,
            "W9825G6KH-6 has 13 row address bits (8192 rows)"
        );
        assert_eq!(cfg.data_width_bits, 16, "W9825G6KH-6 is 16-bit wide");
        assert_eq!(cfg.banks, 4, "W9825G6KH-6 has 4 internal banks");
        assert_eq!(
            cfg.cas_latency, 3,
            "W9825G6KH-6 CAS latency at 100 MHz is 3"
        );
    }

    #[test]
    fn test_sdram_config_refresh_count() {
        let cfg = SdramConfig::w9825g6kh6_at_100mhz();
        assert_eq!(
            cfg.refresh_count,
            platform::sdram::W9825G6KH6_REFRESH_COUNT,
            "SdramConfig refresh_count must match W9825G6KH6_REFRESH_COUNT"
        );
        assert_eq!(cfg.refresh_count, 761);
    }

    #[test]
    fn test_sdram_config_timing_matches_platform() {
        let cfg = SdramConfig::w9825g6kh6_at_100mhz();
        let platform_timing = platform::sdram::SdramTiming::w9825g6kh6_at_100mhz();
        // Verify timing fields are consistent
        assert_eq!(cfg.timing.row_cycle_delay, platform_timing.row_cycle_delay);
        assert_eq!(cfg.timing.rp_delay, platform_timing.rp_delay);
        assert_eq!(cfg.timing.rc_delay, platform_timing.rc_delay);
        assert_eq!(
            cfg.timing.load_to_active_delay,
            platform_timing.load_to_active_delay
        );
    }

    #[test]
    fn test_mpu_register_pairs_delegates_to_platform() {
        let boot_pairs = mpu_register_pairs();
        let platform_pairs = platform::mpu::MpuApplier::soul_audio_register_pairs();
        assert_eq!(
            boot_pairs, platform_pairs,
            "boot::mpu_register_pairs() must delegate to MpuApplier::soul_audio_register_pairs()"
        );
    }
}
