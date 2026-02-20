//! ES9038Q2M DAC register addresses and constants.
//!
//! Reference: ESS Technology ES9038Q2M datasheet Rev 3.0, Section 9 (Register Map).
//!
//! The ES9038Q2M is a high-performance stereo audio DAC with I2C control.
//! Default I2C address is 0x48 (ADDR pin low) or 0x49 (ADDR pin high).

/// 7-bit I2C device address when ADDR pin is pulled low.
pub const ES9038Q2M_I2C_ADDR_LOW: u8 = 0x48;
/// 7-bit I2C device address when ADDR pin is pulled high.
pub const ES9038Q2M_I2C_ADDR_HIGH: u8 = 0x49;

/// Register 0: System configuration (soft reset, lock mode, CLK_GEAR, …).
pub const REG_SYSTEM: u8 = 0;
/// Register 7: Filter configuration (roll-off shape selection).
pub const REG_FILTER: u8 = 7;
/// Register 11: Control (I2S mode, channel mapping, …).
pub const REG_CONTROL: u8 = 11;
/// Register 14: GPIO configuration.
pub const REG_GPIO: u8 = 14;
/// Register 15: Left-channel attenuation (0x00 = 0 dB, 0xFF = –127.5 dB, muted).
pub const REG_ATT_L: u8 = 15;
/// Register 16: Right-channel attenuation (same encoding as REG_ATT_L).
pub const REG_ATT_R: u8 = 16;

/// Attenuation value that mutes the output (–127.5 dB).
pub const ATT_MUTED: u8 = 0xFF;
/// Attenuation value for full volume (0 dB).
pub const ATT_FULL_VOLUME: u8 = 0x00;

/// Initialize the ES9038Q2M DAC for I2S 192 kHz / 32-bit PCM operation.
///
/// Startup sequence (DAC muted during init):
/// 1. Write REG_ATT_L and REG_ATT_R = ATT_MUTED (0xFF): mute outputs
/// 2. Write REG_SYSTEM = 0x01: soft reset (self-clearing)
/// 3. Write REG_SYSTEM = 0x00: normal operation
/// 4. Write REG_FILTER = 0x00: linear phase fast roll-off filter
///
/// # Errors
/// Returns Err if any I2C write fails.
pub fn es9038q2m_init<I>(i2c: &mut I, addr: u8) -> Result<(), I::Error>
where
    I: embedded_hal::i2c::I2c,
{
    // Mute outputs before any other configuration
    i2c.write(addr, &[REG_ATT_L, ATT_MUTED])?;
    i2c.write(addr, &[REG_ATT_R, ATT_MUTED])?;
    // Soft reset (bit 0 = 1, self-clearing)
    i2c.write(addr, &[REG_SYSTEM, 0x01])?;
    // Normal operation after reset
    i2c.write(addr, &[REG_SYSTEM, 0x00])?;
    // Linear phase fast roll-off filter (bits 4:2 = 0b000)
    i2c.write(addr, &[REG_FILTER, 0x00])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockI2c {
        writes: std::vec::Vec<(u8, std::vec::Vec<u8>)>,
    }
    impl embedded_hal::i2c::ErrorType for MockI2c {
        type Error = core::convert::Infallible;
    }
    impl embedded_hal::i2c::I2c for MockI2c {
        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [embedded_hal::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
            for op in operations.iter() {
                if let embedded_hal::i2c::Operation::Write(data) = op {
                    self.writes.push((address, data.to_vec()));
                }
            }
            Ok(())
        }
    }

    #[test]
    fn i2c_addr_low_matches_datasheet() { assert_eq!(ES9038Q2M_I2C_ADDR_LOW, 0x48); }
    #[test]
    fn i2c_addr_high_matches_datasheet() { assert_eq!(ES9038Q2M_I2C_ADDR_HIGH, 0x49); }
    #[test]
    fn reg_att_l_matches_datasheet() { assert_eq!(REG_ATT_L, 15); }
    #[test]
    fn reg_att_r_matches_datasheet() { assert_eq!(REG_ATT_R, 16); }
    #[test]
    fn att_muted_is_max_value() { assert_eq!(ATT_MUTED, 0xFF); }
    #[test]
    fn att_full_volume_is_zero() { assert_eq!(ATT_FULL_VOLUME, 0x00); }
    #[test]
    fn reg_system_is_register_zero() { assert_eq!(REG_SYSTEM, 0); }
    #[test]
    fn reg_filter_is_register_seven() { assert_eq!(REG_FILTER, 7); }
    #[test]
    fn reg_control_is_register_eleven() { assert_eq!(REG_CONTROL, 11); }
    #[test]
    fn reg_gpio_is_register_fourteen() { assert_eq!(REG_GPIO, 14); }
    #[test]
    fn att_muted_and_full_volume_are_distinct() { assert_ne!(ATT_MUTED, ATT_FULL_VOLUME); }
    #[test]
    fn i2c_addresses_are_adjacent() { assert_eq!(ES9038Q2M_I2C_ADDR_HIGH, ES9038Q2M_I2C_ADDR_LOW + 1); }

    // -- TDD tests for es9038q2m_init() --
    #[test]
    fn es9038q2m_init_powers_up_correctly() {
        let mut mock = MockI2c::default();
        es9038q2m_init(&mut mock, ES9038Q2M_I2C_ADDR_LOW).unwrap();
        assert!(!mock.writes.is_empty());
    }
    #[allow(clippy::unwrap_used)] #[test]
    fn es9038q2m_init_starts_muted() {
        let mut mock = MockI2c::default();
        es9038q2m_init(&mut mock, ES9038Q2M_I2C_ADDR_LOW).unwrap();
        let att_l = mock.writes.iter().find(|(_, d)| d.first() == Some(&REG_ATT_L));
        let att_r = mock.writes.iter().find(|(_, d)| d.first() == Some(&REG_ATT_R));
        assert!(att_l.is_some(), "init() must write REG_ATT_L");
        assert!(att_r.is_some(), "init() must write REG_ATT_R");
        // SAFETY: att_l/att_r are Some (asserted above); data[1] is the register value byte.
        #[allow(clippy::indexing_slicing)]
        {
            assert_eq!(att_l.unwrap().1[1], ATT_MUTED);
            assert_eq!(att_r.unwrap().1[1], ATT_MUTED);
        }
    }
    #[test]
    fn es9038q2m_init_correct_i2c_address() {
        let mut mock = MockI2c::default();
        es9038q2m_init(&mut mock, ES9038Q2M_I2C_ADDR_LOW).unwrap();
        for (addr, _) in &mock.writes {
            assert_eq!(*addr, ES9038Q2M_I2C_ADDR_LOW);
        }
    }
}
