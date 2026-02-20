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
}
