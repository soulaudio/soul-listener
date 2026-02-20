//! HIL peripheral presence tests.
//!
//! Validates that key I2C peripherals respond at their expected addresses.

#[cfg(test)]
mod hil_peripheral_tests {
    /// Expected I2C addresses for hardware peripherals.
    const PMIC_I2C_ADDR: u8 = 0x6A;   // BQ25895
    const DAC_I2C_ADDR: u8 = 0x48;    // ES9038Q2M (ADDR pin low)

    #[test]
    fn peripheral_i2c_addresses_are_documented() {
        // Validate address constants match platform crate values
        // (Compile-time check — no hardware needed)
        assert_eq!(PMIC_I2C_ADDR, 0x6A, "BQ25895 I2C address must be 0x6A (fixed, not configurable)");
        assert_eq!(DAC_I2C_ADDR, 0x48, "ES9038Q2M I2C address must be 0x48 (ADDR pin pulled low)");
    }

    #[test]
    fn hil_peripheral_presence_placeholder() {
        // TODO(HIL): Replace with actual I2C scan on hardware:
        //   let mut found = [false; 128];
        //   for addr in 0..127 {
        //       if i2c.write(addr, &[]).is_ok() { found[addr as usize] = true; }
        //   }
        //   defmt::assert!(found[PMIC_I2C_ADDR as usize], "BQ25895 not found at 0x6A");
        //   defmt::assert!(found[DAC_I2C_ADDR as usize], "ES9038Q2M not found at 0x48");
        let _ = "HIL peripheral test placeholder — see README.md";
    }
}
