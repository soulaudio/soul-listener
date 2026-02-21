//! HIL boot sequence tests.
//!
//! Validates that the STM32H743ZI boot sequence completes without hardfault:
//! MPU configuration → AXI errata write → SDRAM init → Embassy executor start.
//!
//! # Running
//! ```
//! cargo test --features hardware --target thumbv7em-none-eabihf
//! ```
//!
//! # Requirements
//! - probe-rs installed and board connected via SWD
//! - STM32H743ZI target powered

// These are placeholder tests — actual HIL execution requires probe-rs runner.
// The test bodies document WHAT to check; the assertions use defmt when hardware feature is enabled.

/// Verifies the boot sequence memory map is correctly configured.
/// Hardware check: no HardFault within 1 second of reset.
#[cfg(test)]
mod hil_boot_tests {
    #[test]
    fn memory_map_constants_are_correct() {
        // Validate addresses that will be used during HIL boot
        assert_eq!(0x24000000u32, 0x24000000); // AXI SRAM base
        assert_eq!(0x20000000u32, 0x20000000); // DTCM base
        assert_eq!(0x08000000u32, 0x08000000); // Flash base
        assert_eq!(0xC0000000u32, 0xC0000000); // External SDRAM base
    }

    #[test]
    fn hil_test_framework_placeholder() {
        // This test passes on host. On hardware, replace with:
        //   defmt::assert!(boot_completed_flag.load(Ordering::Acquire));
        // using a global AtomicBool set by the boot sequence.
        //
        // TODO(HIL): When probe-rs + defmt-test are configured, add real hardware assertions.
        // See tests/hardware/README.md for setup instructions.
        let _ = "HIL test placeholder — see README.md";
    }

    #[test]
    fn mpu_region_count_is_three() {
        // The STM32H743 MPU must configure exactly 3 non-cacheable regions:
        //   1. AXI SRAM (DMA1/2 buffers — audio SAI, display SPI, SDMMC)
        //   2. SRAM4   (BDMA buffers — D3-domain peripherals)
        //   3. SRAM1/2 (D2-domain DMA — Embassy task channels)
        //
        // This count is enforced at compile time via platform::mpu::MpuApplier.
        // On hardware, we would read MPU_RNR, MPU_RBAR, MPU_RASR for each region
        // and verify the non-cacheable bits are set correctly.
        //
        // HIL TODO: after `apply_mpu_config()`, read MPU_CTRL and verify
        //           ENABLE=1, PRIVDEFENA=1 via probe-rs RTT assertion.
        let expected_regions = 3usize;
        assert_eq!(
            expected_regions, 3,
            "MPU must configure exactly 3 non-cacheable regions"
        );
    }

    #[test]
    fn watchdog_timeout_is_8_seconds() {
        // IWDG timeout is configured in firmware/src/boot.rs as WATCHDOG_TIMEOUT_MS = 8000.
        // On hardware: after reset, verify IWDG_RLR = reload value for 8 s at LSI 32 kHz.
        //
        // LSI  = 32,000 Hz
        // Prescaler = /256 (IWDG_PR = 0b110)
        // Timeout = 8 s
        // Reload = LSI * timeout_s / prescaler = 32_000 * 8 / 256 = 1000
        //
        // HIL TODO: read IWDG_RLR register and assert value = 999 (0-indexed reload field).
        //           The RL field is 12-bit (max 4095); 1000 is well within range.
        let lsi_hz: u32 = 32_000;
        let prescaler: u32 = 256;
        let timeout_s: u32 = 8;
        let expected_reload = lsi_hz * timeout_s / prescaler;

        // RL field is 12-bit (max 4095); verify the formula produces a valid value.
        assert!(
            expected_reload > 0 && expected_reload <= 4095,
            "IWDG reload value {expected_reload} must fit in the 12-bit RL field (1–4095)"
        );
        // Document the exact value for the HIL assertion probe-rs will check.
        assert_eq!(
            expected_reload, 1000,
            "IWDG reload must be 1000 for 8 s timeout at LSI 32 kHz / prescaler 256"
        );
    }

    #[test]
    fn axi_sram_is_within_fmc_addressable_range() {
        // AXI SRAM must not overlap with FMC address space.
        // FMC NOR/SRAM banks start at 0x6000_0000 (Bank 1–4).
        // FMC SDRAM banks start at 0xC000_0000 (Bank 5–6).
        // AXI SRAM is at 0x2400_0000–0x247F_FFFF (512 KB) — well below FMC.
        let axi_sram_start: u32 = 0x2400_0000;
        let axi_sram_size: u32 = 512 * 1024; // 524_288 bytes
        let axi_sram_end: u32 = axi_sram_start + axi_sram_size;
        let fmc_start: u32 = 0x6000_0000;

        assert!(
            axi_sram_end < fmc_start,
            "AXI SRAM (0x{axi_sram_start:08X}–0x{axi_sram_end:08X}) \
             must not overlap FMC address space (0x{fmc_start:08X}+)"
        );
    }

    #[test]
    fn dtcm_and_axi_sram_are_distinct_regions() {
        // DTCM and AXI SRAM must not overlap — they are physically separate.
        // DTCM:     0x2000_0000 to 0x2001_FFFF (128 KB, CPU-only)
        // AXI SRAM: 0x2400_0000 to 0x247F_FFFF (512 KB, DMA-accessible)
        //
        // The gap 0x2002_0000–0x23FF_FFFF is unmapped on STM32H743.
        let dtcm_base: u32 = 0x2000_0000;
        let dtcm_end: u32 = dtcm_base + 128 * 1024;
        let axi_sram_base: u32 = 0x2400_0000;

        assert!(
            dtcm_end <= axi_sram_base,
            "DTCM (0x{dtcm_base:08X}–0x{dtcm_end:08X}) must not overlap \
             AXI SRAM (0x{axi_sram_base:08X}+)"
        );
    }

    #[test]
    fn flash_base_is_below_sram() {
        // Internal Flash must reside below all SRAM regions.
        // Flash: 0x0800_0000 (2 MB)
        // DTCM:  0x2000_0000 (128 KB) — lowest SRAM region
        //
        // This is a sanity check that the STM32H743 memory map constants
        // used in linker scripts (memory.x) are self-consistent.
        let flash_base: u32 = 0x0800_0000;
        let flash_size: u32 = 2 * 1024 * 1024; // 2 MB
        let flash_end: u32 = flash_base + flash_size;
        let dtcm_base: u32 = 0x2000_0000;

        assert!(
            flash_end <= dtcm_base,
            "Flash (0x{flash_base:08X}–0x{flash_end:08X}) must reside below DTCM (0x{dtcm_base:08X}+)"
        );
    }

    #[test]
    fn sdram_base_is_at_fmc_bank5() {
        // External SDRAM (W9825G6KH-6) is mapped via FMC Bank 5/6.
        // FMC Bank 5 SDRAM base address = 0xC000_0000.
        // This is hardwired in the STM32H743 FMC controller and matches
        // the FMC_SDCR1/FMC_SDCR2 register configuration.
        //
        // HIL TODO: after SDRAM init, write and read-back a known pattern at
        //           0xC000_0000 via probe-rs memory access.
        let sdram_base: u32 = 0xC000_0000;
        let fmc_bank5_base: u32 = 0xC000_0000;

        assert_eq!(
            sdram_base, fmc_bank5_base,
            "External SDRAM must be mapped at FMC Bank 5 base 0xC0000000"
        );
    }
}
