//! External SDRAM abstraction via FMC
//!
//! Provides a trait for accessing external SDRAM connected through the STM32H7
//! Flexible Memory Controller (FMC). Used for large allocations that exceed
//! internal SRAM capacity: library index cache, album art thumbnails, and
//! large audio decode scratch buffers.
//!
//! # Hardware
//!
//! - **Option A:** IS42S16320G-7TL (ISSI) — 64 MB (32M × 16-bit), TSOP-54
//! - **Option B:** W9825G6KH-6 (Winbond) — 32 MB (16M × 16-bit), TSOP-54
//!
//! Mapped at `0xC0000000` via FMC bank 5/6 after initialization by the
//! Embassy STM32 FMC driver. Accesses use the CPU cache and are subject to
//! MPU region configuration.
//!
//! # Memory Region Layout (32 MB target)
//!
//! ```text
//! 0xC000_0000  ┌─────────────────────┐
//!              │  Library index cache │  4 MB  (~13k tracks @ 300 B)
//! 0xC040_0000  ├─────────────────────┤
//!              │  Album art cache     │  8 MB  (~500 thumbs @ 16 KB)
//! 0xC0C0_0000  ├─────────────────────┤
//!              │  Audio decode scratch│  4 MB  (FLAC + DSD512 ring buf)
//! 0xC100_0000  ├─────────────────────┤
//!              │  UI overflow / spare │ 16 MB  (future expansion)
//! 0xC200_0000  └─────────────────────┘
//! ```
//!
//! # DMA note
//!
//! SDRAM is accessible by DMA — audio DMA buffers for real-time paths should
//! still prefer internal AXI SRAM (`0x2400_0000`) to avoid FMC arbitration
//! latency. Use SDRAM for large, non-latency-critical buffers only.

/// External SDRAM interface.
///
/// Implementations provide safe, bounds-checked access to a region of
/// SDRAM. On hardware this wraps a `*mut u8` slice over the FMC window;
/// in tests it wraps a heap-allocated `Vec<u8>`.
pub trait ExternalRam {
    /// Error type
    type Error: core::fmt::Debug;

    /// Read bytes from external RAM at `offset`.
    ///
    /// Returns `Err` if `offset + buf.len() > capacity()`.
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), Self::Error>;

    /// Write bytes to external RAM at `offset`.
    ///
    /// Returns `Err` if `offset + data.len() > capacity()`.
    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), Self::Error>;

    /// Zero-fill `len` bytes starting at `offset`.
    fn zero(&mut self, offset: usize, len: usize) -> Result<(), Self::Error>;

    /// Total SDRAM capacity in bytes.
    fn capacity(&self) -> usize;
}

/// Well-known SDRAM region descriptors.
///
/// Defines the canonical layout for a 32 MB SDRAM device.
/// All offsets are relative to the FMC base address (`0xC000_0000`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamRegion {
    /// Byte offset from SDRAM base
    pub offset: usize,
    /// Region length in bytes
    pub len: usize,
}

impl RamRegion {
    /// Music library index cache — 4 MB
    ///
    /// Holds serialised track records for browse/search without SD I/O.
    /// Supports ~13,000 tracks at ~300 B per record.
    pub const LIBRARY_INDEX: Self = Self {
        offset: 0,
        len: 4 * 1024 * 1024,
    };

    /// Album art thumbnail cache — 8 MB
    ///
    /// Pre-scaled thumbnails (96×96 px, 2bpp = 2.3 KB each).
    /// Fits ~500 album thumbnails resident in memory.
    pub const ALBUM_ART: Self = Self {
        offset: 4 * 1024 * 1024,
        len: 8 * 1024 * 1024,
    };

    /// Audio decoder scratch — 4 MB
    ///
    /// Working memory for FLAC frame decode (~128 KB) and DSD512
    /// streaming ring buffer (~1.1 MB for 200 ms at stereo DSD512).
    pub const AUDIO_SCRATCH: Self = Self {
        offset: 12 * 1024 * 1024,
        len: 4 * 1024 * 1024,
    };

    /// UI overflow / reserved — 16 MB
    ///
    /// Available for future features: waveform display buffers,
    /// additional font glyph caches, OTA download staging.
    pub const UI_OVERFLOW: Self = Self {
        offset: 16 * 1024 * 1024,
        len: 16 * 1024 * 1024,
    };
}

// ─── SDRAM Timing ────────────────────────────────────────────────────────────
//
// Web research findings (2026-02-20):
//
// W9825G6KH-6 datasheet (Winbond, -6 speed grade, 166 MHz capable):
//   Source: https://datasheet.lcsc.com/lcsc/1809291411_Winbond-Elec-W9825G6KH-6_C62246.pdf
//   tRC  (Row Cycle time, ACTIVATE→ACTIVATE same bank) = 60 ns
//   tRAS (Row Active / self-refresh time)              = 42 ns
//   tRCD (Row to Column Delay)                         = 15 ns
//   tRP  (Row Precharge time)                          = 15 ns
//   tWR  (Write Recovery)                              = 2 CLK cycles minimum
//   tMRD (Load Mode Register to Active)                = 2 CLK cycles minimum
//   tXSR (Exit Self-Refresh to Active)                 = 70 ns
//   tREF (Refresh period, 4096 rows / 64 ms)           = 15,625 ns
//
// STM32H743 FMC clock (HCLK3):
//   Source: https://community.st.com/t5/stm32-mcus-products/stm32h743-max-fmc-sdram-clock-100mhz/td-p/92417
//   FMC kernel clock = HCLK3, typically 200 MHz for STM32H743 at 480 MHz sysclk.
//   FMC_CLK (SDRAM clock output) = FMC kernel clock / 2 → 100 MHz.
//   Max SDRAM clock: 110 MHz (rev V), 100 MHz (rev Y). We use 100 MHz = 10 ns period.
//
// stm32-fmc crate timing API:
//   Source: https://github.com/stm32-rs/stm32-fmc/blob/master/src/devices/is42s32800g.rs
//   SdramTiming struct fields (all in clock cycles, not nanoseconds):
//     mode_register_to_active  → tMRD
//     exit_self_refresh        → tXSR
//     active_to_precharge      → tRAS
//     row_cycle                → tRC
//     row_precharge            → tRP
//     row_to_column            → tRCD
//   Our SdramTiming mirrors these names adjusted for our internal API.
//
// Embassy SAI OverrunError (issue #3205):
//   Source: https://github.com/embassy-rs/embassy/issues/3205
//   Once a SAI overrun occurs, all subsequent write() calls immediately return
//   OverrunError — no built-in reset mechanism exists.
//   Recovery: drop the driver (peripheral resets on Drop), then reconstruct it
//   with new_sai4() (or equivalent). Workaround: fill with silence continuously.
//   This module documents the timing; sai_recovery.rs (firmware crate) handles state.

/// SDRAM timing parameters in nanoseconds (from datasheet).
///
/// Computed for W9825G6KH-6 at -6 speed grade (166 MHz capable).
/// Run at 100 MHz FMC clock (10 ns period) for conservative margin.
///
/// # Sources
/// - W9825G6KH-6 datasheet (Winbond): tRC=60ns, tRAS=42ns, tRCD/tRP=15ns,
///   tWR/tMRD=2CLK, tXSR=70ns
/// - STM32H743 reference manual: FMC_CLK = HCLK3/2, max 100 MHz (rev Y)
#[derive(Debug, Clone, Copy)]
pub struct SdramTimingNs {
    /// tMRD — Load Mode Register to Activate (ns). W9825G6KH-6: 2 CLK cycles min.
    pub t_mrd_ns: u32,
    /// tXSR — Exit Self-Refresh to Activate (ns). W9825G6KH-6: 70 ns.
    pub t_xsr_ns: u32,
    /// tRAS — Row Active time / self-refresh time (ns). W9825G6KH-6: 42 ns.
    pub t_ras_ns: u32,
    /// tRC — Row Cycle time: ACTIVATE to ACTIVATE same bank (ns). W9825G6KH-6: 60 ns.
    pub t_rc_ns: u32,
    /// tWR — Write Recovery time (ns). W9825G6KH-6: 2 CLK minimum.
    pub t_wr_ns: u32,
    /// tRP — Row Precharge time (ns). W9825G6KH-6: 15 ns.
    pub t_rp_ns: u32,
    /// tRCD — Row to Column Delay (ns). W9825G6KH-6: 15 ns.
    pub t_rcd_ns: u32,
}

/// SDRAM timing validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdramTimingError {
    /// A timing value computed to 0 cycles (FMC minimum is 1).
    /// The `field` string names the offending parameter (e.g. `"t_rp"`).
    TooSmall {
        /// Name of the timing field that was zero or too small.
        field: &'static str,
    },
}

/// SDRAM timing parameters converted to FMC clock cycles.
///
/// Ready to pass to stm32-fmc `SdramChip` implementation.
/// All fields are 1-based cycle counts (FMC register encoding: value = cycles − 1
/// for most fields, but we store the raw cycle count here for clarity).
///
/// # Field mapping to stm32-fmc `SdramTiming`
/// | This struct field       | stm32-fmc field          | Datasheet param |
/// |-------------------------|--------------------------|-----------------|
/// | `load_to_active_delay`  | `mode_register_to_active`| tMRD            |
/// | `exit_self_refresh_delay`| `exit_self_refresh`     | tXSR            |
/// | `self_refresh_time`     | `active_to_precharge`    | tRAS            |
/// | `row_cycle_delay`       | `row_cycle`              | tRC             |
/// | `write_recovery_time`   | *(implicit in tRP+tWR)*  | tWR             |
/// | `rp_delay`              | `row_precharge`          | tRP             |
/// | `rc_delay`              | `row_to_column`          | tRCD            |
#[derive(Debug, Clone, Copy)]
pub struct SdramTiming {
    /// tMRD in cycles (W9825G6KH-6: ≥ 2 CLK)
    pub load_to_active_delay: u32,
    /// tXSR in cycles (W9825G6KH-6: 70 ns → 7 at 100 MHz)
    pub exit_self_refresh_delay: u32,
    /// tRAS in cycles (W9825G6KH-6: 42 ns → 5 at 100 MHz; FMC min 1)
    pub self_refresh_time: u32,
    /// tRC in cycles (W9825G6KH-6: 60 ns → 6 at 100 MHz)
    pub row_cycle_delay: u32,
    /// tWR in cycles (W9825G6KH-6: ≥ 2 CLK)
    pub write_recovery_time: u32,
    /// tRP in cycles (W9825G6KH-6: 15 ns → 2 at 100 MHz; FMC min 1)
    pub rp_delay: u32,
    /// tRCD in cycles (W9825G6KH-6: 15 ns → 2 at 100 MHz; FMC min 1)
    pub rc_delay: u32,
}

impl SdramTiming {
    /// Convert nanoseconds to FMC clock cycles (ceiling division).
    ///
    /// `fmc_hz`: FMC clock frequency in Hz (e.g., `100_000_000` for 100 MHz).
    /// Returns at least 1 (FMC minimum).
    ///
    /// Formula: `cycles = ceil(ns * fmc_hz / 1_000_000_000)`.
    /// Uses integer arithmetic to avoid floating-point in `no_std`.
    pub fn ns_to_cycles(ns: u32, fmc_hz: u32) -> u32 {
        // period_ns = 1_000_000_000 / fmc_hz
        // cycles    = ceil(ns / period_ns)
        //           = ceil(ns * fmc_hz / 1_000_000_000)
        // Integer ceiling: (a + b - 1) / b  where b = 1_000_000_000
        let numer = ns as u64 * fmc_hz as u64;
        let cycles = (numer + 999_999_999) / 1_000_000_000;
        cycles.max(1) as u32
    }

    /// Create timing from nanosecond specs, converting to cycles at `fmc_hz`.
    ///
    /// Returns `Err` if any field converts to 0 cycles (only possible when
    /// `t_rp_ns = 0` or similar explicitly-zero inputs, since `ns_to_cycles`
    /// already clamps to ≥ 1, but `t_rp_ns = 0` produces `ceil(0/period) = 0`
    /// before the max(1) guard — so we validate `t_rp_ns > 0` semantically).
    pub fn new(ns: SdramTimingNs, fmc_hz: u32) -> Result<Self, SdramTimingError> {
        // Validate that explicitly-zero ns values are caught.
        // ns_to_cycles(0, _) = ceil(0) = 0, then max(1) = 1, which would mask
        // a user error. Validate fields that the FMC requires > 0 cycles.
        if ns.t_rp_ns == 0 {
            return Err(SdramTimingError::TooSmall { field: "t_rp" });
        }
        if ns.t_rcd_ns == 0 {
            return Err(SdramTimingError::TooSmall { field: "t_rcd" });
        }
        if ns.t_rc_ns == 0 {
            return Err(SdramTimingError::TooSmall { field: "t_rc" });
        }
        if ns.t_ras_ns == 0 {
            return Err(SdramTimingError::TooSmall { field: "t_ras" });
        }
        if ns.t_xsr_ns == 0 {
            return Err(SdramTimingError::TooSmall { field: "t_xsr" });
        }

        Ok(Self {
            // W9825G6KH-6 tMRD: minimum 2 CLK cycles (datasheet specifies CLK-based,
            // not ns-based). We take whichever is larger: ns conversion or 2 CLK min.
            load_to_active_delay: Self::ns_to_cycles(ns.t_mrd_ns, fmc_hz).max(2),
            exit_self_refresh_delay: Self::ns_to_cycles(ns.t_xsr_ns, fmc_hz),
            self_refresh_time: Self::ns_to_cycles(ns.t_ras_ns, fmc_hz),
            row_cycle_delay: Self::ns_to_cycles(ns.t_rc_ns, fmc_hz),
            // W9825G6KH-6 tWR: minimum 2 CLK cycles (datasheet specifies CLK-based).
            write_recovery_time: Self::ns_to_cycles(ns.t_wr_ns, fmc_hz).max(2),
            rp_delay: Self::ns_to_cycles(ns.t_rp_ns, fmc_hz),
            rc_delay: Self::ns_to_cycles(ns.t_rcd_ns, fmc_hz),
        })
    }

    /// Pre-computed W9825G6KH-6 timing at 100 MHz FMC clock.
    ///
    /// Values from W9825G6KH-6 datasheet (Winbond, -6 speed grade, 166 MHz capable).
    /// Operating at 100 MHz FMC clock (10 ns period) for conservative margin.
    ///
    /// Resulting cycle counts:
    /// - tMRD = 2 CLK min → `load_to_active_delay = 2`
    /// - tXSR = 70 ns     → `exit_self_refresh_delay = ceil(70/10) = 7`
    /// - tRAS = 42 ns     → `self_refresh_time = ceil(42/10) = 5`
    /// - tRC  = 60 ns     → `row_cycle_delay = ceil(60/10) = 6`
    /// - tWR  = 2 CLK min → `write_recovery_time = 2`
    /// - tRP  = 15 ns     → `rp_delay = ceil(15/10) = 2`
    /// - tRCD = 15 ns     → `rc_delay = ceil(15/10) = 2`
    pub fn w9825g6kh6_at_100mhz() -> Self {
        Self::new(
            SdramTimingNs {
                t_mrd_ns: 20, // 2 CLK @ 100 MHz = 20 ns (datasheet: 2 CLK min)
                t_xsr_ns: 70, // 70 ns (datasheet: tXSR = 70 ns)
                t_ras_ns: 42, // 42 ns (datasheet: tRAS min = 42 ns)
                t_rc_ns: 60,  // 60 ns (datasheet: tRC = tRAS + tRP = 42+18 ≈ 60 ns)
                t_wr_ns: 20,  // 2 CLK @ 100 MHz = 20 ns (datasheet: 2 CLK min)
                t_rp_ns: 15,  // 15 ns (datasheet: tRP = 15 ns)
                t_rcd_ns: 15, // 15 ns (datasheet: tRCD = 15 ns)
            },
            100_000_000,
        )
        .expect("W9825G6KH-6 timing values are statically valid at 100 MHz")
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test A ────────────────────────────────────────────────────────────────
    /// Verify W9825G6KH-6 pre-computed timing values at 100 MHz FMC clock.
    ///
    /// FMC clock period = 10 ns at 100 MHz.
    #[test]
    fn test_w9825g6kh6_timing_values_at_100mhz() {
        let timing = SdramTiming::w9825g6kh6_at_100mhz();

        // tMRD = 2 CLK cycles minimum (from datasheet)
        assert_eq!(timing.load_to_active_delay, 2);

        // tXSR (exit self-refresh) = 70 ns → ceil(70/10) = 7 cycles
        assert_eq!(timing.exit_self_refresh_delay, 7);

        // tRAS = 42 ns → ceil(42/10) = 5 cycles (with 6-cycle minimum per FMC)
        assert!(timing.self_refresh_time >= 5);

        // tRC = 60 ns → ceil(60/10) = 6 cycles
        assert_eq!(timing.row_cycle_delay, 6);

        // tWR = 2 clock cycles (from datasheet: 1 CLK + tRAS min)
        assert!(timing.write_recovery_time >= 2);

        // tRP = 15 ns → ceil(15/10) = 2 cycles (min 2)
        assert_eq!(timing.rp_delay, 2);

        // tRCD = 15 ns → ceil(15/10) = 2 cycles (min 2)
        assert_eq!(timing.rc_delay, 2);
    }

    // ── Test B ────────────────────────────────────────────────────────────────
    /// Verify nanosecond-to-cycle ceiling conversion at 100 MHz (10 ns period).
    #[test]
    fn test_timing_ns_to_cycles_conversion() {
        // ceil(42 / 10.0) = 5 (tRAS at 100 MHz)
        assert_eq!(SdramTiming::ns_to_cycles(42, 100_000_000), 5);

        // ceil(15 / 10.0) = 2 (tRP/tRCD at 100 MHz)
        assert_eq!(SdramTiming::ns_to_cycles(15, 100_000_000), 2);

        // ceil(70 / 10.0) = 7 (tXSR at 100 MHz)
        assert_eq!(SdramTiming::ns_to_cycles(70, 100_000_000), 7);

        // ceil(60 / 10.0) = 6 (tRC at 100 MHz)
        assert_eq!(SdramTiming::ns_to_cycles(60, 100_000_000), 6);

        // ceil(1 / 10.0) = 1 (minimum 1 cycle)
        assert_eq!(SdramTiming::ns_to_cycles(1, 100_000_000), 1);
    }

    // ── Test C ────────────────────────────────────────────────────────────────
    /// FMC requires all timing values >= 1 cycle; zero-ns tRP must be rejected.
    #[test]
    fn test_timing_validates_fmc_minimums() {
        let result = SdramTiming::new(
            SdramTimingNs {
                t_mrd_ns: 10,
                t_xsr_ns: 70,
                t_ras_ns: 42,
                t_rc_ns: 60,
                t_wr_ns: 20,
                t_rp_ns: 0, // INVALID — must be rejected
                t_rcd_ns: 15,
            },
            100_000_000,
        );
        assert!(result.is_err());
    }

    // ── Test D ────────────────────────────────────────────────────────────────
    /// Verify the 32 MB SDRAM region layout has no overlaps and sums to 32 MB.
    #[test]
    fn test_sdram_region_layout_32mb() {
        // Library → Album art: contiguous
        let lib = RamRegion::LIBRARY_INDEX;
        let art = RamRegion::ALBUM_ART;
        assert_eq!(lib.offset + lib.len, art.offset);

        // Album art → Audio scratch: contiguous
        let scratch = RamRegion::AUDIO_SCRATCH;
        assert_eq!(art.offset + art.len, scratch.offset);

        // Total must fit in 32 MB exactly
        let ui = RamRegion::UI_OVERFLOW;
        let total = ui.offset + ui.len;
        assert_eq!(total, 32 * 1024 * 1024);
    }
}
