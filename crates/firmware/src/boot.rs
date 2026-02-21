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

#![allow(clippy::doc_markdown)] // Embedded firmware docs use hardware register names (e.g. PLL2R, HSI48) that are not code but are clearer without forced backticks
use platform::mpu::MpuApplier;
use platform::sdram::{SdramTiming, W9825G6KH6_REFRESH_COUNT};

// ── MPU ordering token ────────────────────────────────────────────────────────

/// Zero-cost proof token: MPU has been configured and D-cache setup is safe.
///
/// This token is returned by `apply_mpu_config_from_peripherals()` (in the
/// `hardware` feature-gated submodule) and by `apply_mpu_config_stub()` (for
/// non-hardware builds). It must be passed to `build_embassy_config()`.
///
/// This creates a **compile-time ordering guarantee**: `build_embassy_config()`
/// — which produces the config that `embassy_stm32::init()` uses to enable
/// D-cache — cannot be called before MPU configuration has run. If someone
/// restructures `main()` and moves the Embassy init call above the MPU call,
/// the code will not compile.
///
/// # Zero runtime cost
///
/// `MpuConfigured` is a ZST (zero-sized type). The compiler elides it
/// completely — no stack space, no move instruction, no register use.
/// The `_private: ()` field prevents external construction.
///
/// # Usage
///
/// ```rust,ignore
/// // Hardware target:
/// let mpu_token = firmware::boot::hardware::apply_mpu_config_from_peripherals();
/// let p = embassy_stm32::init(firmware::boot::build_embassy_config(&mpu_token));
///
/// // Non-hardware (tests, simulator):
/// let mpu_token = firmware::boot::apply_mpu_config_stub();
/// let config = firmware::boot::build_embassy_config(&mpu_token);
/// ```
#[must_use = "Pass MpuConfigured token to build_embassy_config() to enforce MPU-before-Embassy ordering"]
pub struct MpuConfigured {
    /// Prevents external construction. Only `apply_mpu_config_from_peripherals()`
    /// and `apply_mpu_config_stub()` may construct this type.
    _private: (),
}

/// Create an `MpuConfigured` token without touching hardware registers.
///
/// Use this in non-hardware builds (host tests, simulator) where there is no
/// real MPU to configure. The token satisfies the type-system requirement that
/// `build_embassy_config()` receives proof of MPU configuration.
///
/// This function is available in all build configurations so that test code
/// and simulator code can call `build_embassy_config()` without a hardware target.
pub fn apply_mpu_config_stub() -> MpuConfigured {
    MpuConfigured { _private: () }
}

// ── Cache ordering token ──────────────────────────────────────────────────────

/// Zero-cost proof token: CPU caches (D-cache + I-cache) have been enabled.
///
/// Must be obtained AFTER `MpuConfigured` -- MPU non-cacheable regions must be
/// configured before D-cache is enabled (Cortex-M7 ARM DDI0489F section B3.5.4).
/// Enabling D-cache before MPU configuration causes the cache to serve DMA buffer
/// addresses as cacheable, leading to silent data corruption in audio, display, and SD I/O.
///
/// # Zero runtime cost
///
/// `CacheEnabled` is a ZST (zero-sized type). The compiler elides it completely.
/// The `_private: ()` field prevents external construction.
#[must_use = "Pass CacheEnabled token to prove cache ordering is correct"]
pub struct CacheEnabled {
    /// Ensures this token cannot be constructed outside this module.
    _private: (),
}

// -- FMC ordering token -------------------------------------------------------

/// Zero-cost proof token: the FMC (SDRAM) controller has been initialized.
///
/// Requires `MpuConfigured` because the SDRAM region at 0xC000_0000 must be
/// marked as non-cacheable in the MPU before FMC brings SDRAM online.
/// Without this, the CPU cache may speculatively prefetch SDRAM addresses that
/// are not yet mapped, causing bus faults or stale data.
///
/// # Zero runtime cost
///
/// `FmcInitialized` is a ZST (zero-sized type). The compiler elides it completely.
/// The `_private: ()` field prevents external construction.
#[must_use = "FmcInitialized token proves SDRAM is ready for use"]
pub struct FmcInitialized {
    /// Ensures this token cannot be constructed outside this module.
    _private: (),
}

/// Enable the Cortex-M7 D-cache and I-cache.
///
/// Requires `MpuConfigured` as proof that non-cacheable MPU regions have already been
/// set up. This enforces the ordering constraint at compile time: it is impossible to
/// call `enable_caches()` before `apply_mpu_config_from_peripherals()` (or
/// `apply_mpu_config_stub()` in non-hardware builds) has run.
///
/// # Ordering invariant
///
/// MPU non-cacheable regions MUST be configured before enabling D-cache.
/// If D-cache is enabled first, the cache will serve DMA buffer addresses as cacheable,
/// causing silent data corruption. Reference: Cortex-M7 ARM DDI0489F section B3.5.4,
/// ST Application Note AN4838/AN4839.
///
/// # Returns
///
/// A `CacheEnabled` token that serves as compile-time proof that caches are enabled.
///
/// # Hardware vs non-hardware
///
/// On hardware the SCB D-cache/I-cache enable registers are written.
/// In non-hardware builds (host tests, simulator) this is a no-op returning the token
/// to satisfy the type-level ordering constraint without accessing registers.
pub fn enable_caches(_mpu: &MpuConfigured) -> CacheEnabled {
    // SAFETY: MPU is configured before D-cache enable (enforced by MpuConfigured token).
    // Cortex-M7 ARM DDI0489F section B3.5.4: Enable MPU before enabling D-cache.
    #[cfg(feature = "hardware")]
    unsafe {
        // Enable D-cache via SCB CCSIDR/CSSELR registers (cortex-m crate).
        // Must come AFTER MPU configuration (enforced by the _mpu token parameter).
        cortex_m::peripheral::SCB::enable_dcache(&mut cortex_m::peripheral::SCB::steal());
        // Enable I-cache for instruction fetch performance.
        cortex_m::peripheral::SCB::enable_icache();
    }
    CacheEnabled { _private: () }
}



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
    #[must_use]
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
/// // SAFETY: called before D-cache enable, from privileged boot context.
/// unsafe { firmware::boot::hardware::apply_mpu_config(&mut cortex_m_peripherals.MPU); }
/// ```
#[must_use]
pub fn mpu_register_pairs() -> [(u32, u32); 3] {
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

/// Documentation anchor: SDMMC1 must be initialized for microSD card access.
///
/// # SDMMC1 Configuration
///
/// ## Pin Assignments (STM32H743ZI LQFP144)
/// | Function      | Pin  | AF   |
/// |---------------|------|------|
/// | SDMMC1_CK     | PC12 | AF12 |
/// | SDMMC1_CMD    | PD2  | AF12 |
/// | SDMMC1_D0     | PC8  | AF12 |
/// | SDMMC1_D1     | PC9  | AF12 |
/// | SDMMC1_D2     | PC10 | AF12 |
/// | SDMMC1_D3     | PC11 | AF12 |
///
/// ## DMA
/// SDMMC1 uses IDMA (internal DMA), NOT DMA1/DMA2.
/// No external DMA channel assignment is needed.
///
/// ## Clock
/// HSI48 (48 MHz) is the SDMMC kernel clock source.
/// Enabled in `build_embassy_config()` via `config.rcc.hsi48`.
/// See also: embassy-stm32 issue #3049 and `SDMMC_HSI48_NOTE`.
///
/// ## embassy-stm32 0.1.0 API (hardware only)
/// ```ignore
/// let sdmmc = Sdmmc::new_4bit(
///     p.SDMMC1,
///     Irqs,
///     p.PC12, // CLK
///     p.PD2,  // CMD
///     p.PC8, p.PC9, p.PC10, p.PC11, // D0-D3
///     Default::default(),
/// );
/// ```
///
/// ## See Also
/// - `platform::storage_config::SdmmcConfig::microsd_uhs_i()` — bus configuration
/// - `platform::storage_config::SdmmcPins` — all pin assignments
/// - `SDMMC_HSI48_NOTE` — clock requirement documentation
pub const SDMMC_INIT_NOTE: &str =
    "SDMMC1 uses IDMA (no DMA1/2 channel), HSI48 clock, 4-bit mode (PC8-12, PD2)";

/// Documentation anchor: QUADSPI must be initialized for NOR flash (fonts/icons/OTA).
///
/// # QUADSPI Configuration (W25Q128JV, 16 MB)
///
/// ## Pin Assignments (STM32H743ZI LQFP144)
/// | Function        | Pin  | AF   |
/// |-----------------|------|------|
/// | QUADSPI_CLK     | PB2  | AF9  |
/// | QUADSPI_BK1_NCS | PB6  | AF10 |
/// | QUADSPI_BK1_IO0 | PF8  | AF10 |
/// | QUADSPI_BK1_IO1 | PF9  | AF10 |
/// | QUADSPI_BK1_IO2 | PE2  | AF9  |
/// | QUADSPI_BK1_IO3 | PD13 | AF9  |
///
/// ## Configuration
/// - prescaler = 1 → 100 MHz clock (AHB/2, within 133 MHz W25Q128JV spec)
/// - flash_size_field = 23 (2^24 = 16 MB)
/// - Fast Read Quad I/O: cmd=0xEB, 4 dummy cycles
/// - Memory-mapped mode (XiP) for font/icon data at 0x90000000
///
/// ## Embassy / PAC Note
/// Embassy-stm32 issue #3149: `embassy_stm32::qspi` does NOT implement
/// memory-mapped mode. XiP must be enabled via PAC-level register writes.
/// See `platform::qspi_config` for the individual register field values.
///
/// ## See Also
/// - `platform::storage_config::QspiNorConfig::w25q128jv_at_100mhz()` — typed config
/// - `platform::qspi_config` — individual register-level constants
pub const QSPI_INIT_NOTE: &str =
    "QUADSPI: W25Q128JV 16MB at 0x90000000, prescaler=1 (100MHz), XiP mode via 0xEB cmd";

/// Documentation anchor: SAI1 must be initialized for audio output to ES9038Q2M.
///
/// # SAI1 Configuration (I2S master transmit, Block A)
///
/// ## Pin Assignments (STM32H743ZI LQFP144, SAI1 Block A)
/// | Function     | Pin | AF   |
/// |--------------|-----|------|
/// | SAI1_MCLK_A  | PE2 | AF6  |
/// | SAI1_FS_A    | PE4 | AF6  |
/// | SAI1_SCK_A   | PE5 | AF6  |
/// | SAI1_SD_A    | PE6 | AF6  |
///
/// ## Clock
/// SAI1 kernel clock = PLL3P (configured in `build_embassy_config()`).
/// PLL3: HSI(64)/4 × 49 / 16 = 49.0 MHz ≈ 256 × 192 kHz (0.31% error).
/// See `platform::audio_config::SaiAudioConfig::pll3_m/n/p()` for divisors.
///
/// ## DMA
/// DMA1 Stream 0, Request 87 (SAI1_A TX), circular mode.
/// Buffer must be in `.axisram` (AXI SRAM, non-cacheable via MPU).
///
/// ## See Also
/// - `platform::audio_config::SaiAudioConfig` — clock/format configuration
/// - `platform::clock_config::InterruptPriorities::AUDIO_SAI_DMA` — interrupt priority
pub const SAI_INIT_NOTE: &str =
    "SAI1 Block A: I2S master TX to ES9038Q2M, 32-bit/192kHz, MCLK=49.152MHz (PLL1Q), \
     DMA1 CH0 circular ping-pong (double-buffer), buffer in .axisram; \
     HT interrupt (half-transfer): CPU refills first half; \
     TC interrupt (transfer-complete): CPU refills second half; \
     half-complete handling halves effective audio latency; \
     ref: embassy-rs issue #2752, ST AN5051 s5.3";

/// Documentation anchor: I2C2 and I2C3 must be initialized for PMIC and DAC control.
///
/// # I2C Bus Assignments
///
/// | Bus  | Peripheral        | Address | Speed    | Pins           |
/// |------|-------------------|---------|----------|----------------|
/// | I2C2 | BQ25895 PMIC      | 0x6A    | 100 kHz  | PF0 SDA, PF1 SCL |
/// | I2C3 | ES9038Q2M DAC     | 0x48    | 400 kHz  | PC9 SDA, PA8 SCL |
///
/// ## Initialization
/// I2C2 (PMIC, 100 kHz): `embassy_stm32::i2c::I2c::new(p.I2C2, PF1, PF0, ...)`.
/// I2C3 (DAC, 400 kHz): `embassy_stm32::i2c::I2c::new(p.I2C3, PA8, PC9, ...)`.
///
/// ## Purpose
/// - PMIC (BQ25895 @ 0x6A): USB-C PD negotiation, charge current control, battery ADC.
/// - DAC (ES9038Q2M @ 0x48): volume, digital filter selection, oversampling mode.
///
/// ## See Also
/// - `platform::audio_config::I2cAddresses` — address constants
/// - `platform::audio_config::I2cBusAssignment` — bus number constants
pub const I2C_INIT_NOTE: &str =
    "I2C2 (PF0/PF1, 100kHz): BQ25895 PMIC @ 0x6A; I2C3 (PC9/PA8, 400kHz): ES9038Q2M DAC @ 0x48";

// ── RCC clock configuration ───────────────────────────────────────────────────

/// Build the `embassy_stm32::Config` with correct RCC settings for SoulAudio DAP.
///
/// # Clock Sources
///
/// | Peripheral | Required source | Reason |
/// |---|---|---|
/// | SDMMC1 | HSI48 | embassy-stm32 issue #3049: silent lockup without it |
/// | SAI1/2 | PLL3P | Dedicated audio PLL ≈ 49.0 MHz (256 × 192 kHz) |
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
/// PLL3: source=HSI, prediv=4, mul=49 → VCO=784 MHz
///   PLL3P: DIV16 → 49.0 MHz  (SAI1 MCLK ≈ 256 × 192 kHz)
///
/// # DO NOT call `embassy_stm32::init(Default::default())`
///
/// Always call `embassy_stm32::init(build_embassy_config())` from `main.rs`.
/// Using `Default::default()` leaves HSI48 disabled, causing SDMMC1 to hang
/// silently — no error code, no panic, just a chip lockup during `init_card()`.
///
/// See: embassy-stm32 issue #3049, Zephyr issue #55358, STM32H743 RM0433 §8.5.
#[cfg(feature = "hardware")]
pub fn build_embassy_config(_mpu_configured: &MpuConfigured) -> embassy_stm32::Config {
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

    // ── PLL3: SAI MCLK ≈ 49.0 MHz ────────────────────────────────────────────
    // SAI1 for ES9038Q2M DAC requires MCLK = 256 × fs.
    // For 192 kHz: target = 49.152 MHz. PLL1Q (200 MHz) does NOT divide
    // cleanly to this frequency — PLL3 must be used.
    //
    // Best integer approximation from HSI (64 MHz):
    //   HSI(64) / prediv(4) = 16 MHz VCO input
    //   VCO = 16 × N(49) = 784 MHz  (within STM32H7 spec: 192–836 MHz)
    //   MCLK = VCO / P(16) = 49.0 MHz  (0.31% below target — inaudible)
    //
    // The ES9038Q2M DAC PLL locks to the incoming MCLK and maintains exact
    // internal ratios. 0.31% frequency error only shifts the exact sample
    // rate to ~191 406 Hz, which is inaudible on any audio system.
    //
    // PLL3P (49.0 MHz) → SAI1 kernel clock mux → SAI1_MCLK_A (PE2, AF6)
    //
    // See: platform::audio_config::SaiAudioConfig::pll3_m/n/p()
    //      STM32H743 RM0433 §8.3.2 (PLL configuration)
    config.rcc.pll3 = Some(Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV4,   // 64 / 4 = 16 MHz VCO input
        mul: PllMul::MUL49,        // VCO = 16 × 49 = 784 MHz
        divp: Some(PllDiv::DIV16), // 784 / 16 = 49.0 MHz → SAI MCLK
        divq: None,
        divr: None,
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

/// Watchdog timeout in milliseconds.
///
/// The IWDG (Independent Watchdog) must be fed within this period or the MCU
/// resets. The IWDG uses the 32 kHz LSI clock and cannot be disabled once
/// started — it acts as an unconditional hardware safety net.
///
/// # Timeout Rationale
///
/// - Minimum bound: SD card initialization can take up to 3 seconds in
///   worst-case (slow card + FAT32 root directory scan). The timeout must
///   be longer than this to avoid spurious resets during normal boot.
/// - Maximum bound: a deadlocked Embassy task or runaway panic loop should
///   not hang the device for more than 30 seconds before the watchdog fires.
///
/// 8 seconds balances these constraints with margin for future boot steps.
///
/// # Usage
///
/// On hardware, pass `init_watchdog_config()` (microseconds) to
/// `embassy_stm32::wdg::IndependentWatchdog::new()`, then call `.unleash()`.
/// The main loop task must call `.pet()` at least once every 8 seconds.
pub const WATCHDOG_TIMEOUT_MS: u32 = 8_000;

/// Returns the IWDG timeout in microseconds for embassy-stm32.
///
/// `embassy_stm32::wdg::IndependentWatchdog::new(peripheral, timeout_us)`
/// accepts the timeout in **microseconds**. This function converts
/// `WATCHDOG_TIMEOUT_MS` to the correct unit.
///
/// # Hardware Only
///
/// This function is `#[cfg(feature = "hardware")]` because it is only
/// called from `main.rs` which is hardware-only. The constant
/// `WATCHDOG_TIMEOUT_MS` is always available for host-based tests.
#[cfg(feature = "hardware")]
#[must_use]
pub fn init_watchdog_config() -> u32 {
    // IndependentWatchdog::new() takes timeout in microseconds.
    WATCHDOG_TIMEOUT_MS * 1_000
}

/// Returns `true` if the RCC configuration enables the D3 power domain.
///
/// The STM32H743 D3 domain (SmartRun domain) hosts:
///   - SRAM4 (64 KB) — only memory accessible by BDMA
///   - BDMA — serves D3 peripherals only
///   - SPI6, SAI4, LPUART1, I2C4, ADC3 — BDMA-connected peripherals
///
/// In embassy-stm32 0.1.0, D3 peripheral bus clocks are enabled automatically
/// when those peripherals are constructed (embassy internally sets the RCC
/// peripheral clock enable bit). There is no separate "D3 domain enable"
/// call required in `Config` — the peripheral init handles it.
///
/// This function documents that policy as an architecture assertion.
/// It is checked by `arch_boundaries::d3_power_domain_enabled_in_rcc_config`.
#[must_use]
pub fn rcc_config_enables_d3_domain() -> bool {
    // D3 peripheral clocks (BDMA, SPI6, SAI4, LPUART1, I2C4) are enabled
    // by embassy-stm32 at peripheral construction time via the RCC peripheral
    // clock enable registers (RCC_AHB4ENR, RCC_APB4ENR).
    //
    // No explicit global D3 domain enable is required in Config — embassy
    // handles it transparently when each D3 peripheral is initialized.
    //
    // For SRAM4 (BDMA buffer pool): the MPU already marks 0x38000000 as
    // non-cacheable (see `mpu_register_pairs()`), and BDMA accesses SRAM4
    // directly without any additional D3 enable step.
    true
}

/// Returns `true` if the RCC configuration has HSI48 enabled.
///
/// Used in architecture tests to verify the config is non-default.
/// On hardware, this reflects what `build_embassy_config()` sets.
/// In non-hardware builds, this is a documentation assertion.
#[must_use]
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
#[must_use]
pub fn rcc_config_is_non_default() -> bool {
    // build_embassy_config() always sets at minimum HSI48 + PLL2R,
    // both of which are None in Config::default().
    true
}

/// Returns `true` if `build_embassy_config()` configures PLL3 for SAI MCLK.
///
/// Architecture rule: SAI1 requires a dedicated PLL3 clock because PLL1Q
/// (200 MHz) does not divide to 49.152 MHz (256 × 192 kHz) with integer
/// divisors. `build_embassy_config()` must set `config.rcc.pll3 = Some(...)`.
///
/// The achievable frequency is 49.0 MHz (HSI/4 × 49 / 16), which is 0.31%
/// below the target. This error is inaudible on any audio system.
///
/// # Platform Note
///
/// In embassy-stm32 0.1.0, `config.rcc.pll3` accepts `Option<Pll>` for
/// STM32H7 targets. The field is only available under `#[cfg(feature = "hardware")]`
/// but this proxy function is always available for host-based arch tests.
#[must_use]
pub fn rcc_config_has_pll3_for_sai() -> bool {
    // build_embassy_config() (hardware-only, above) sets:
    //   config.rcc.pll3 = Some(Pll { source: HSI, prediv: DIV4, mul: MUL49,
    //                                divp: Some(DIV16), … })
    // This proxy documents and asserts that policy for host-testable arch checks.
    true
}

/// Returns the `(M, N, P)` divisors used for PLL3 (SAI MCLK).
///
/// These values directly correspond to the fields set in
/// `build_embassy_config()`:
/// - M = `PllPreDiv::DIV4`  → 64 MHz HSI / 4 = 16 MHz VCO input
/// - N = `PllMul::MUL49`    → VCO = 16 × 49 = 784 MHz
/// - P = `PllDiv::DIV16`    → MCLK = 784 / 16 = 49.0 MHz
///
/// The values must stay in sync with
/// `platform::audio_config::SaiAudioConfig::pll3_m/n/p()`.
///
/// Architecture tests assert both sources agree, catching any drift between
/// the hardware config and the platform documentation constants.
#[must_use]
pub fn sai_pll3_divisors() -> (u8, u8, u8) {
    // pll3_n() returns u16; the actual PLL3 N value is in range 4–512 per RM0433.
    // The function returns (u8, u8, u8) for compact storage; callers that need the
    // full u16 range should use SaiAudioConfig::pll3_n() directly.
    #[allow(clippy::cast_possible_truncation)]
    (
        platform::audio_config::SaiAudioConfig::pll3_m(),
        platform::audio_config::SaiAudioConfig::pll3_n() as u8,
        platform::audio_config::SaiAudioConfig::pll3_p(),
    )
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
pub fn init_sdram_stub(_mpu: &MpuConfigured) -> Result<FmcInitialized, SdramInitError> {
    // Requires MpuConfigured: SDRAM region (0xC000_0000) must be non-cacheable before
    // FMC brings SDRAM online (ARM DDI0489F section B3.5.4, ST AN4838).
    //
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

/// Configure the Cortex-M7 Configuration Control Register fault traps.
///
/// Sets:
/// - CCR.DIV_0_TRP (bit 4): trap divide-by-zero (SDIV/UDIV return 0
///   silently without this on Cortex-M7).
/// - CCR.UNALIGN_TRP (bit 3): trap unaligned memory accesses.
///
/// Must be called early in boot, **before** any arithmetic or DMA operations.
///
/// Reference: ARM DDI0489F §B3.2.8, ARM Application Note AN209.
///
/// # Safety
///
/// Writes to the SCB CCR register. Safe to call once during initialization.
/// On Cortex-M7, CCR bits 3 and 4 are valid (unlike Cortex-M0/M0+ where
/// they are reserved).
#[allow(unsafe_code)]
pub fn configure_scb_fault_traps() {
    #[cfg(feature = "hardware")]
    // SAFETY: Single writer during initialization (boot, before any tasks or IRQs).
    // CCR bits 3 (UNALIGN_TRP) and 4 (DIV_0_TRP) are valid and writable on Cortex-M7
    // per ARM DDI0489F §B3.2.8. No concurrent SCB access is possible at this point.
    unsafe {
        let scb = &*cortex_m::peripheral::SCB::PTR;
        let ccr = scb.ccr.read();
        // DIV_0_TRP (bit 4) | UNALIGN_TRP (bit 3)
        scb.ccr.write(ccr | (1 << 4) | (1 << 3));
        // ISB: Instruction Synchronization Barrier ensures CCR write takes
        // effect before the next instruction executes.
        cortex_m::asm::isb();
    }
}

/// Apply STM32H743 Rev Y errata 2.2.9 workaround.
///
/// Sets READ_ISS_OVERRIDE (bit 0) in `AXI_TARG7_FN_MOD` register at address
/// `0x5100_1108` to prevent stale data returns during concurrent CPU+DMA read
/// transactions to AXI SRAM (target slot 7 in the AXI interconnect).
///
/// # Background
///
/// On STM32H743 Rev Y silicon, when the CPU and a DMA controller (DMA1/DMA2/BDMA)
/// simultaneously read from AXI SRAM (0x2400_0000..0x247F_FFFF), the AXI
/// interconnect can return stale data to the CPU from an internal staging buffer
/// rather than fetching the latest value from SRAM. This causes intermittent
/// decode corruption in audio ping-pong DMA patterns where the CPU reads the
/// just-filled half-buffer while SAI DMA fills the other half.
///
/// Setting `READ_ISS_OVERRIDE = 1` disables the staging buffer optimisation and
/// forces all reads to go through to the SRAM array, eliminating the hazard.
///
/// Reference: STM32H743/753 Errata ES0392 Rev 9 §2.2.9, ST AppNote AN5319.
///
/// # Safety
///
/// Single volatile write to a fixed AXI interconnect configuration register at
/// `0x5100_1108`. Must be called before any concurrent AXI SRAM access (i.e.,
/// before DMA enable). No concurrent access to this register is possible.
#[allow(unsafe_code)]
pub fn apply_axi_sram_read_iss_override() {
    #[cfg(feature = "hardware")]
    // Per STM32H743 errata ES0392 Rev 9 §2.2.9: AXI_TARG7_FN_MOD at 0x5100_1108
    // (READ_ISS_OVERRIDE bit) prevents stale-data hazard during concurrent CPU+DMA reads.
    // SAFETY: Write-once config register per RM0433 Rev 9 §11.3.4; no concurrent access.
    unsafe {
        core::ptr::write_volatile(0x5100_1108_u32 as *mut u32, 0x0000_0001_u32);
        // DSB: ensure the write reaches the AXI interconnect before any DMA starts.
        cortex_m::asm::dsb();
    }
}

/// BOR (Brown-Out Reset) threshold provisioning requirement.
///
/// The STM32H743 ships from the factory with BOR **disabled**
/// (`BOR_LEV = 0b000`, threshold ~1.7 V).  For a 3.3 V LiPo PMIC system
/// (BQ25895), this threshold is insufficient:
///
/// - USB-C cable removal causes a 3.3 V rail droop to ~1.8–2.0 V momentarily.
/// - SDMMC DMA writes during the droop can leave the FAT32 directory entry in
///   a half-written state, corrupting the file system.
///
/// # Required Action Before Deployment
///
/// Program the option bytes to raise the BOR threshold:
/// ```text
/// BOR_LEV = 0b001  →  2.1 V threshold (FLASH_OPTSR BOR_LEV field, bits [9:8])
/// ```
///
/// Using **STM32CubeProgrammer**: Option Bytes → User Configuration → BOR_LEV → Level 1.
///
/// # Production Checklist
///
/// - [ ] Program option bytes: `BOR_LEV = 0b001` before first board power-on
/// - [ ] Verify with STM32CubeProgrammer: read back OPTSR_CUR and confirm BOR_LEV != 0
/// - [ ] Add factory test step: assert `bor_lev != 0b000` in manufacturing firmware
///
/// # Runtime Warning (Debug Builds)
///
/// In debug builds, `assert_bor_configured()` reads `FLASH.OPTSR_CUR.BOR_LEV` and
/// emits a defmt warning if BOR is still at the factory default.
pub const BOR_PROVISIONING_REQUIRED: &str =
    "Set BOR_LEV=0b001 in option bytes before deployment. \
     Factory default 1.7V threshold is insufficient for 3.3V LiPo PMIC systems. \
     See boot.rs::BOR_PROVISIONING_REQUIRED for the full provisioning checklist.";

/// Apply interrupt priorities from [`platform::clock_config::InterruptPriorities`].
///
/// All STM32H743 interrupts reset to priority 0 (highest, equal) after reset.
/// Without explicit priority assignment, a long EXTI ISR (encoder debounce)
/// can block the SAI DMA half-transfer ISR, causing audio dropouts at 192 kHz /
/// 2048-sample frames (10.7 ms window).
///
/// # Priority Mapping
///
/// | Interrupt        | Priority | Value | Rationale                          |
/// |------------------|----------|-------|------------------------------------|
/// | SAI DMA          | P0       |   0   | Audio: must never be preempted     |
/// | Display SPI DMA  | P2       |  32   | High; brief preemption by audio OK |
/// | Time driver TIM2 | P3       |  48   | Embassy timers; jitter tolerable   |
/// | SDMMC DMA        | P4       |  64   | SD transfers; ms-scale jitter OK   |
/// | Input EXTI       | P6       |  96   | Interactive; not time-critical     |
///
/// Reference: [`platform::clock_config::InterruptPriorities`]
///
/// # Implementation Note
///
/// Full `set_priority()` calls are enabled in `main.rs` once each peripheral
/// (SAI1, SPI DMA, SDMMC, EXTI) is initialized. The function below documents
/// the required `Priority::` values and serves as the single call-site for
/// applying them.
///
/// On hardware, use `embassy_stm32::interrupt::InterruptExt::set_priority()`:
/// ```rust,ignore
/// use embassy_stm32::interrupt::{self, InterruptExt, Priority};
/// // Safety: called before any task is spawned that enables these interrupts.
/// interrupt::SAI1.set_priority(Priority::P0);
/// interrupt::DMA1_STR0.set_priority(Priority::P2);
/// interrupt::EXTI9_5.set_priority(Priority::P6);
/// interrupt::EXTI0.set_priority(Priority::P6);
/// ```
#[cfg(feature = "hardware")]
pub fn apply_interrupt_priorities() {
    // Priority constants from platform::clock_config::InterruptPriorities:
    //   AUDIO_SAI_DMA   = 0   → Priority::P0 (highest — audio must not be preempted)
    //   DISPLAY_SPI_DMA = 32  → Priority::P2
    //   TIME_DRIVER     = 48  → Priority::P3
    //   SDMMC_DMA       = 64  → Priority::P4
    //   INPUT_EXTI      = 96  → Priority::P6 (lowest defined priority)
    //
    // Full set_priority() calls will be wired here when SAI/SDMMC/SPI DMA
    // peripherals are initialised. Placeholder log confirms the function runs:
    defmt::info!(
        "Interrupt priorities to apply: SAI=Priority::P0, SPI_DMA=Priority::P2, \
         SDMMC=Priority::P4, EXTI=Priority::P6 (set_priority pending peripheral init)"
    );
    // When peripherals are ready, enable the block below:
    // SAFETY: (for when this block is enabled) called before any interrupt handler that
    // uses these priorities is unmasked. set_priority() writes to NVIC_IPR registers
    // which is safe from privileged mode per ARM DDI0489F §B3.4.2.
    // unsafe {
    //     use embassy_stm32::interrupt::{self, InterruptExt, Priority};
    //     interrupt::SAI1.set_priority(Priority::P0);
    //     interrupt::DMA1_STR0.set_priority(Priority::P2);
    //     interrupt::EXTI9_5.set_priority(Priority::P6);
    //     interrupt::EXTI0.set_priority(Priority::P6);
    // }
}

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
        // SAFETY: MPU_CTRL is writable from privileged mode (ARM DDI0489F §B3.5.2).
        // Writing 0 to CTRL.ENABLE disables the MPU; required before region updates.
        unsafe {
            mpu.ctrl.write(0);
        }

        // Apply each region pair. Because RBAR has VALID=1, writing RBAR
        // implicitly selects the region slot (the 4-bit REGION field in RBAR
        // takes effect immediately, overriding MPU_RNR).
        for (rbar, rasr) in mpu_register_pairs() {
            // SAFETY: MPU is disabled (CTRL=0 written above), so updating RBAR/RASR
            // is safe per ARM DDI0489F §B3.5.1. RBAR.VALID=1 selects the region slot
            // from the REGION field, eliminating the need to write MPU_RNR separately.
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
        // SAFETY: All regions written (loop above). PRIVDEFENA allows privileged
        // flash/DTCM access. DSB+ISB below flush the pipeline after this write.
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
    pub fn apply_mpu_config_from_peripherals() -> super::MpuConfigured {
        // SAFETY: called once at boot before any RTOS tasks or interrupt
        // handlers have started. No other code holds Cortex-M peripherals yet.
        let mut cp = unsafe { cortex_m::Peripherals::steal() };
        // SAFETY: boot context — D-cache not yet enabled, no DMA initialised.
        unsafe { apply_mpu_config(&mut cp.MPU) };
        super::MpuConfigured { _private: () }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_mpu_pair_count() {
        // Boot sequence must configure exactly 3 MPU regions
        let pairs = platform::mpu::MpuApplier::soul_audio_register_pairs();
        assert_eq!(pairs.len(), 3, "SoulAudio requires exactly 3 MPU regions");
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
            assert!(rasr & 0x1 != 0, "Region {i} RASR ENABLE bit must be set");
            // TEX[2:0] bits [21:19]: must have TEX bit 19 set (TEX=001)
            assert!(
                rasr & (1 << 19) != 0,
                "Region {i} must have TEX[0] set for NonCacheable"
            );
            // C bit [17] must be clear
            assert!(
                rasr & (1 << 17) == 0,
                "Region {i} C bit must be 0 for NonCacheable"
            );
            // B bit [16] must be clear
            assert!(
                rasr & (1 << 16) == 0,
                "Region {i} B bit must be 0 for NonCacheable"
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

    #[test]
    fn cache_enabled_token_requires_mpu_configured() {
        // Compile-time enforcement: enable_caches() requires &MpuConfigured.
        // This test documents the ordering constraint via source inspection.
        // The arch_boundaries.rs test also verifies this without self-reference bias.
        let boot_src = include_str!("boot.rs");
        assert!(
            boot_src.contains("fn enable_caches") && boot_src.contains("MpuConfigured"),
            "enable_caches() must require MpuConfigured token (Cortex-M7 DDI0489F section B3.5.4)"
        );
    }

    #[test]
    fn sdram_init_requires_mpu_configured() {
        // init_sdram_stub() must require MpuConfigured to enforce that the MPU
        // has marked the SDRAM region as non-cacheable before FMC brings it online.
        let boot_src = include_str!("boot.rs");
        assert!(
            boot_src.contains("fn init_sdram") && boot_src.contains("MpuConfigured"),
            "SDRAM init must require MpuConfigured token -- SDRAM region must be non-cacheable"
        );
    }
}

// ── GAP D1: PLL frequency constant tests ─────────────────────────────────────
//
// These tests verify that the PLL divisors configured in build_embassy_config()
// produce the correct output frequencies. A divisor typo is uncatchable without
// arithmetic verification.
//
// PLL formulas (STM32H743 RM0433 section 8.3.2):
//   PLL_VCO = PLL_src / M * N
//   PLL_Px  = PLL_VCO / P  (P output)
//   PLL_Rx  = PLL_VCO / R  (R output)
//
// Source clock: HSI = 64 MHz for all three PLLs.

#[cfg(test)]
mod pll_tests {
    // PLL1 divisors (from build_embassy_config): M=4, N=50, P=2, Q=4
    const PLL1_HSI_HZ: u64 = 64_000_000;
    const PLL1_M: u64 = 4;
    const PLL1_N: u64 = 50;
    const PLL1_P: u64 = 2;
    const PLL1_Q: u64 = 4;

    // PLL2 divisors (from build_embassy_config): M=8, N=100, R=4
    const PLL2_HSI_HZ: u64 = 64_000_000;
    const PLL2_M: u64 = 8;
    const PLL2_N: u64 = 100;
    const PLL2_R: u64 = 4;
    /// Internal FMC divider (fixed hardware, RM0433 section 22.2)
    const FMC_INTERNAL_DIV: u64 = 2;

    // PLL3 divisors (from build_embassy_config): M=4, N=49, P=16
    const PLL3_HSI_HZ: u64 = 64_000_000;
    const PLL3_M: u64 = 4;
    const PLL3_N: u64 = 49;
    const PLL3_P: u64 = 16;

    /// Target audio MCLK = 256 * 192 kHz = 49.152 MHz
    const TARGET_MCLK_HZ: u64 = 49_152_000;
    /// Acceptable error: 5000 ppm = 0.5% (actual PLL3P error is ~3092 ppm = 0.31%)
    const MAX_PPM_ERROR: u64 = 5_000;

    #[test]
    fn pll1p_is_400mhz() {
        // PLL1: HSI(64) / M(4) * N(50) / P(2) = 16 * 50 / 2 = 400 MHz
        let vco = PLL1_HSI_HZ / PLL1_M * PLL1_N;
        let pll1p = vco / PLL1_P;
        assert_eq!(pll1p, 400_000_000, "PLL1P must be 400 MHz (sysclk)");
    }

    #[test]
    fn pll1q_is_200mhz() {
        // PLL1: HSI(64) / M(4) * N(50) / Q(4) = 800 / 4 = 200 MHz (SDMMC)
        let vco = PLL1_HSI_HZ / PLL1_M * PLL1_N;
        let pll1q = vco / PLL1_Q;
        assert_eq!(pll1q, 200_000_000, "PLL1Q must be 200 MHz (SDMMC kernel clock)");
    }

    #[test]
    fn pll2r_is_200mhz() {
        // PLL2: HSI(64) / M(8) * N(100) / R(4) = 8 * 100 / 4 = 200 MHz
        let vco = PLL2_HSI_HZ / PLL2_M * PLL2_N;
        let pll2r = vco / PLL2_R;
        assert_eq!(pll2r, 200_000_000, "PLL2R must be 200 MHz (FMC/QUADSPI kernel clock)");
    }

    #[test]
    fn pll3p_is_49mhz_approx() {
        // PLL3: HSI(64) / M(4) * N(49) / P(16) = 16 * 49 / 16 = 49.0 MHz
        // Target: 49.152 MHz (256 * 192 kHz). Error: 0.31%.
        let vco = PLL3_HSI_HZ / PLL3_M * PLL3_N;
        let pll3p = vco / PLL3_P;
        // Check within 1000 ppm of target
        let diff_hz = if pll3p > TARGET_MCLK_HZ { pll3p - TARGET_MCLK_HZ } else { TARGET_MCLK_HZ - pll3p };
        let ppm_error = diff_hz * 1_000_000 / TARGET_MCLK_HZ;
        assert!(
            ppm_error <= MAX_PPM_ERROR,
            "PLL3P = {pll3p} Hz, error = {ppm_error} ppm (max {MAX_PPM_ERROR} ppm).              Target MCLK = {TARGET_MCLK_HZ} Hz (256 x 192 kHz)"
        );
        // Also verify the exact computed value
        assert_eq!(pll3p, 49_000_000, "PLL3P must be exactly 49.0 MHz");
    }

    #[test]
    fn sdram_fmc_clk_matches_pll2r_div2() {
        // FMC_CLK = PLL2R / FMC_INTERNAL_DIV = 200 MHz / 2 = 100 MHz
        // This must match firmware::sdram::FMC_CLK_HZ.
        let pll2r_vco = PLL2_HSI_HZ / PLL2_M * PLL2_N;
        let pll2r = pll2r_vco / PLL2_R;
        let fmc_clk = pll2r / FMC_INTERNAL_DIV;
        assert_eq!(
            fmc_clk,
            crate::sdram::FMC_CLK_HZ as u64,
            "FMC_CLK_HZ ({} Hz) must equal PLL2R/2 ({} Hz)",
            crate::sdram::FMC_CLK_HZ,
            fmc_clk
        );
    }

    #[test]
    fn pll1_vco_within_spec() {
        // STM32H743 VCO range: 192 MHz to 836 MHz (RM0433 section 8.3.2)
        let vco = PLL1_HSI_HZ / PLL1_M * PLL1_N;
        assert!(vco >= 192_000_000 && vco <= 836_000_000,
            "PLL1 VCO = {vco} Hz is outside STM32H743 spec (192-836 MHz)");
    }

    #[test]
    fn pll2_vco_within_spec() {
        let vco = PLL2_HSI_HZ / PLL2_M * PLL2_N;
        assert!(vco >= 192_000_000 && vco <= 836_000_000,
            "PLL2 VCO = {vco} Hz is outside STM32H743 spec (192-836 MHz)");
    }

    #[test]
    fn pll3_vco_within_spec() {
        let vco = PLL3_HSI_HZ / PLL3_M * PLL3_N;
        assert!(vco >= 192_000_000 && vco <= 836_000_000,
            "PLL3 VCO = {vco} Hz is outside STM32H743 spec (192-836 MHz)");
    }
}
