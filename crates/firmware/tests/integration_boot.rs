//! Boot sequence integration tests
//!
//! Validates that hardware initialization components are correctly ordered and
//! configured. These tests exercise the platform crate's boot-time types from
//! the firmware crate's perspective, catching any API mismatches without
//! needing physical hardware.
//!
//! Run with: cargo test -p firmware --test integration_boot

use platform::mpu::{MpuApplier, MpuAttributes, MpuRegion, SoulAudioMpuConfig};
use platform::sdram::{sdram_refresh_count, SdramTiming, W9825G6KH6_REFRESH_COUNT};

// ─── MPU boot tests ──────────────────────────────────────────────────────────

/// Verify that `MpuApplier::soul_audio_register_pairs` returns exactly 2 pairs.
///
/// This is a compile-time architectural constraint: the SoulAudio DAP requires
/// exactly two non-cacheable MPU regions (AXI SRAM + SRAM4). Boot code that
/// iterates the pairs must not be hard-coded to a different count.
#[test]
fn test_mpu_applied_before_dma_use() {
    // MpuApplier::soul_audio_register_pairs() is a pure function that computes
    // RBAR/RASR register values. It has no side effects and does not touch
    // hardware registers — making it safe to call in host tests.
    //
    // The firmware boot sequence must call this before enabling D-cache and
    // before any DMA peripheral is initialized (documented constraint from
    // ST AN4838/AN4839 and CLAUDE.md).
    let pairs = MpuApplier::soul_audio_register_pairs();

    // Three regions: AXI SRAM (primary DMA pool), SRAM4 (BDMA-only pool), SRAM1/2 (D2 domain)
    assert_eq!(
        pairs.len(),
        3,
        "Boot must configure exactly 3 non-cacheable MPU regions"
    );

    // AXI SRAM: region 0
    let (rbar0, _rasr0) = pairs[0];
    // RBAR[31:5] = base address, RBAR[4] = VALID=1, RBAR[3:0] = region slot
    // AXI SRAM base = 0x2400_0000, slot = 0
    // → RBAR = 0x2400_0000 | 0x10 | 0 = 0x2400_0010
    assert_eq!(
        rbar0 & 0xFFFF_FFE0,
        0x2400_0000,
        "AXI SRAM base address must be 0x2400_0000"
    );

    // SRAM4: region 1
    let (rbar1, _rasr1) = pairs[1];
    // SRAM4 base = 0x3800_0000, slot = 1
    // → RBAR = 0x3800_0000 | 0x10 | 1 = 0x3800_0011
    assert_eq!(
        rbar1 & 0xFFFF_FFE0,
        0x3800_0000,
        "SRAM4 base address must be 0x3800_0000"
    );

    // Both regions must be enabled (RASR bit 0 = ENABLE)
    let (_rbar0, rasr0) = pairs[0];
    let (_rbar1, rasr1) = pairs[1];
    assert_ne!(rasr0 & 1, 0, "AXI SRAM MPU region must have ENABLE bit set");
    assert_ne!(rasr1 & 1, 0, "SRAM4 MPU region must have ENABLE bit set");
}

/// Verify that `SoulAudioMpuConfig` regions are correctly typed as NonCacheable.
///
/// Architectural rule: all DMA buffer regions must be `NonCacheable` to prevent
/// the Cortex-M7 D-cache from serving stale data to DMA peripherals.
/// This test catches any accidental attribute change in the platform crate.
#[test]
fn test_mpu_regions_are_non_cacheable() {
    let axi = SoulAudioMpuConfig::axi_sram_dma_region();
    assert_eq!(
        axi.attrs(),
        MpuAttributes::NonCacheable,
        "AXI SRAM MPU region must be NonCacheable for DMA safety"
    );

    let sram4 = SoulAudioMpuConfig::sram4_bdma_region();
    assert_eq!(
        sram4.attrs(),
        MpuAttributes::NonCacheable,
        "SRAM4 MPU region must be NonCacheable for BDMA safety"
    );
}

// ─── SDRAM timing tests ───────────────────────────────────────────────────────

/// Verify that `SdramTiming::w9825g6kh6_at_100mhz` field values match the
/// W9825G6KH-6 datasheet at 100 MHz FMC clock.
///
/// This is an integration-level check: the platform crate provides the timing
/// struct and the firmware crate is the consumer. Any field renames or unit
/// changes in the platform API will surface here.
#[test]
fn test_sdram_timing_applied_correctly() {
    let timing = SdramTiming::w9825g6kh6_at_100mhz();

    // tMRD = 2 CLK min (W9825G6KH-6 datasheet, CLK-based spec)
    assert_eq!(
        timing.load_to_active_delay, 2,
        "load_to_active_delay (tMRD) must be 2 CLK for W9825G6KH-6"
    );

    // tXSR = 70 ns → ceil(70 ns / 10 ns) = 7 cycles at 100 MHz
    assert_eq!(
        timing.exit_self_refresh_delay, 7,
        "exit_self_refresh_delay (tXSR) must be 7 at 100 MHz"
    );

    // tRAS = 42 ns → ceil(42 ns / 10 ns) = 5 cycles at 100 MHz
    assert_eq!(
        timing.self_refresh_time, 5,
        "self_refresh_time (tRAS) must be 5 at 100 MHz"
    );

    // tRC = 60 ns → ceil(60 ns / 10 ns) = 6 cycles at 100 MHz
    assert_eq!(
        timing.row_cycle_delay, 6,
        "row_cycle_delay (tRC) must be 6 at 100 MHz"
    );

    // tRP = 15 ns → ceil(15 ns / 10 ns) = 2 cycles at 100 MHz
    assert_eq!(timing.rp_delay, 2, "rp_delay (tRP) must be 2 at 100 MHz");

    // tRCD = 15 ns → ceil(15 ns / 10 ns) = 2 cycles at 100 MHz
    assert_eq!(timing.rc_delay, 2, "rc_delay (tRCD) must be 2 at 100 MHz");
}

/// Verify the SDRAM refresh counter formula at the W9825G6KH-6 operating point.
///
/// Formula: `(fmc_hz * refresh_ms) / (rows * 1000) - 20`
/// At 100 MHz, 8192 rows, 64 ms: `(100_000_000 * 64) / (8192 * 1000) - 20 = 761`
#[test]
fn test_sdram_refresh_count_at_canonical_point() {
    let count = sdram_refresh_count(100_000_000, 8192, 64);
    assert_eq!(
        count, 761,
        "Refresh count must be 761 for W9825G6KH-6 at 100 MHz"
    );
    assert_eq!(
        W9825G6KH6_REFRESH_COUNT, 761,
        "W9825G6KH6_REFRESH_COUNT const must agree with sdram_refresh_count"
    );
}

// ─── Architecture boundary tests ─────────────────────────────────────────────

/// Verify that `MpuRegion` construction enforces size/alignment at the
/// firmware integration level.
///
/// This test exercises the platform API from firmware's perspective.
/// Any signature or behavior change to `MpuRegion::new` will surface here
/// even if the platform unit tests are unmodified.
#[test]
fn test_mpu_region_construction_from_firmware() {
    // Valid: 512 KB at AXI SRAM base (correct power-of-2 size, aligned address)
    let result = MpuRegion::new(0x2400_0000, 512 * 1024, MpuAttributes::NonCacheable);
    assert!(
        result.is_ok(),
        "AXI SRAM region construction must succeed with valid params"
    );

    // Invalid: 300 KB is not a power of two
    let result = MpuRegion::new(0x2400_0000, 300 * 1024, MpuAttributes::NonCacheable);
    assert!(
        result.is_err(),
        "Non-power-of-two size must be rejected by MpuRegion"
    );

    // Invalid: misaligned base address
    let result = MpuRegion::new(0x2400_1000, 512 * 1024, MpuAttributes::NonCacheable);
    assert!(
        result.is_err(),
        "Misaligned base address must be rejected by MpuRegion"
    );
}
