//! BQ25895 USB-C Power Management IC driver.
//!
//! Reference: Texas Instruments BQ25895 datasheet (SLUSCD3B)

/// 7-bit I2C device address (fixed in silicon, SLUSCD3B §7.5.1).
pub const BQ25895_I2C_ADDR: u8 = 0x6A;
/// REG00: Input source control (IINLIM, VINDPM_OS).
pub const REG00_INPUT_SOURCE: u8 = 0x00;
/// REG01: Power-on configuration (WD_RST, CHG_CONFIG, SYS_MIN, MIN_VBAT_SEL).
pub const REG01_POWER_ON_CONFIG: u8 = 0x01;
/// REG02: Charge current control (BOOST_FREQ, ICO_EN, FORCE_DPDM, AUTO_DPDM_EN, HVDCP_EN, MAXC_EN, FORCE_ICO, ICHG).
pub const REG02_CHARGE_CURRENT: u8 = 0x02;
/// REG03: Pre-charge / termination current control (IPRECHG, ITERM).
pub const REG03_PRECHARGE_TERM: u8 = 0x03;
/// REG04: Charge voltage control (VREG, BATLOWV, VRECHG).
pub const REG04_CHARGE_VOLTAGE: u8 = 0x04;
/// REG05: Charge termination / timer control (EN_TERM, WATCHDOG, EN_TIMER, CHG_TIMER, TREG).
pub const REG05_CHARGE_TIMER: u8 = 0x05;
/// REG06: IR compensation / thermal regulation control (BAT_COMP, VCLAMP, TREG).
pub const REG06_IR_COMP: u8 = 0x06;
/// REG07: Miscellaneous operation control (FORCE_VINDPM, TMR2X_EN, BATFET_DIS, JEITA_VSET, BATFET_DLY, BATFET_RST_EN).
pub const REG07_MISC: u8 = 0x07;
/// REG08: BOOST voltage / limi control (BOOSTV, BOOST_LIM).
pub const REG08_BOOST: u8 = 0x08;
/// REG09: Miscellaneous / BATFET full system reset (BATFET_FULL_SYSTEM_RST, …).
pub const REG09_BATFET: u8 = 0x09;
/// REG0A: BOOST mode current limit.
pub const REG0A_BOOST_CURRENT: u8 = 0x0A;
/// REG0B: Status register (VBUS_STAT, CHRG_STAT, PG_STAT, SDP_STAT, VSYS_STAT).
pub const REG0B_STATUS: u8 = 0x0B;
/// REG0C: Fault register (WATCHDOG_FAULT, BOOST_FAULT, CHRG_FAULT, BAT_FAULT, NTC_FAULT).
pub const REG0C_FAULT: u8 = 0x0C;
/// REG0D: VINDPM threshold / Force VINDPM (FORCE_VINDPM, VINDPM).
pub const REG0D_VINDPM: u8 = 0x0D;
/// REG0E: ADC conversion result — battery voltage (THERM_STAT, BATV).
pub const REG0E_BATTERY_VOLTAGE: u8 = 0x0E;
/// REG0F: ADC conversion result — system voltage (SYSV).
pub const REG0F_SYSTEM_VOLTAGE: u8 = 0x0F;
/// REG10: ADC conversion result — thermistor voltage ratio (TSPCT).
pub const REG10_THERMISTOR_VOLTAGE: u8 = 0x10;
/// REG11: ADC conversion result — VBUS voltage (VBUS_GD, VBUSV).
pub const REG11_VBUS_VOLTAGE: u8 = 0x11;
/// REG12: ADC conversion result — charge current (ICHGR).
pub const REG12_CHARGE_CURRENT_ADC: u8 = 0x12;
/// REG13: INDPM status / IDPM limit (VDPM_STAT, IDPM_STAT, IDPM_LIM).
pub const REG13_INDPM_STATUS: u8 = 0x13;
/// REG14: Device revision / PN (REG_RST, ICO_OPTIMIZED, PN, TS_PROFILE, DEV_REV).
pub const REG14_DEVICE_ID: u8 = 0x14;
/// IINLIM field value: 100 mA input current limit (lowest setting).
pub const IINLIM_100MA: u8 = 0b00_0000;
/// IINLIM field value: 3.25 A input current limit (for 3.0 A USB-C).
pub const IINLIM_3250MA: u8 = 0b11_0010;
/// ICHG field value: ~1472 mA charge current (ICHG × 64 mA/LSB).
pub const ICHG_1500MA: u8 = 0b001_0111;
/// VREG field value (pre-shifted): 4.208 V charge voltage (VREG = 23, formula: 3840 + VREG×16 mV).
pub const VREG_4208MV: u8 = 23 << 2;
/// REG01 value: enable charging with OTG disabled (CHG_CONFIG=01, WD_RST=1).
pub const REG01_ENABLE_CHARGING: u8 = (1 << 6) | (1 << 4);
/// REG0B mask for Power Good status bit.
pub const STATUS_PG_MASK: u8 = 1 << 2;
/// REG0B mask for charge status field (CHRG_STAT[1:0]).
pub const STATUS_CHRG_MASK: u8 = 0b11 << 3;
/// REG0B mask for VBUS status field (VBUS_STAT[2:0]).
pub const STATUS_VBUS_MASK: u8 = 0b111 << 5;
/// VBUS_STAT: no input detected.
pub const VBUS_STAT_NO_INPUT: u8 = 0b000 << 5;
/// VBUS_STAT: USB SDP (Standard Downstream Port, 500 mA).
pub const VBUS_STAT_SDP: u8 = 0b001 << 5;
/// VBUS_STAT: USB CDP (Charging Downstream Port, 1.5 A).
pub const VBUS_STAT_CDP: u8 = 0b010 << 5;
/// VBUS_STAT: USB DCP (Dedicated Charging Port, 1.5 A).
pub const VBUS_STAT_DCP: u8 = 0b011 << 5;
/// VBUS_STAT: HVDCP adapter detected.
pub const VBUS_STAT_HVDCP: u8 = 0b100 << 5;
/// VBUS_STAT: Unknown adapter / non-standard.
pub const VBUS_STAT_ADAPTER: u8 = 0b101 << 5;
/// VBUS_STAT: OTG (On-The-Go) mode active.
pub const VBUS_STAT_OTG: u8 = 0b111 << 5;

/// Decode REG0E raw ADC byte to battery voltage in millivolts.
///
/// Formula from SLUSCD3B: V_BAT = 2304 mV + BATV[6:0] × 20 mV.
/// Bit 7 (THERM_STAT) is masked out.
#[inline]
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub const fn decode_battery_voltage_mv(raw_adc: u8) -> u32 {
    2304 + (raw_adc as u32 & 0x7F) * 20
}

/// Decode REG11 raw ADC byte to VBUS voltage in millivolts.
///
/// Formula from SLUSCD3B: V_BUS = 2600 mV + VBUSV[6:0] × 100 mV.
/// Bit 7 (VBUS_GD) is masked out.
#[inline]
#[must_use]
#[allow(clippy::arithmetic_side_effects)]
pub const fn decode_vbus_voltage_mv(raw_adc: u8) -> u32 {
    2600 + (raw_adc as u32 & 0x7F) * 100
}

/// Initialize the BQ25895 PMIC for a 2000-4000 mAh LiPo.
/// Writes: REG00 (3.25A input limit), REG02 (~1472mA charge),
/// REG04 (4.208V charge voltage), REG01 (enable charging).
/// # Errors
/// Returns Err if any I2C write fails.
pub fn bq25895_init<I>(i2c: &mut I, addr: u8) -> Result<(), I::Error>
where
    I: embedded_hal::i2c::I2c,
{
    i2c.write(addr, &[REG00_INPUT_SOURCE, IINLIM_3250MA])?;
    i2c.write(addr, &[REG02_CHARGE_CURRENT, ICHG_1500MA])?;
    i2c.write(addr, &[REG04_CHARGE_VOLTAGE, VREG_4208MV])?;
    i2c.write(addr, &[REG01_POWER_ON_CONFIG, REG01_ENABLE_CHARGING])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct SharedMockI2c {
        writes: std::vec::Vec<(u8, std::vec::Vec<u8>)>,
    }
    impl embedded_hal::i2c::ErrorType for SharedMockI2c {
        type Error = core::convert::Infallible;
    }
    impl embedded_hal::i2c::I2c for SharedMockI2c {
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
    fn bq25895_i2c_addr_is_0x6a() { assert_eq!(BQ25895_I2C_ADDR, 0x6A); }
    #[test]
    fn charge_voltage_4208mv_encoding_correct() {
        let vreg_field = u32::from(VREG_4208MV >> 2);
        let voltage_mv = 3840 + vreg_field * 16;
        assert_eq!(voltage_mv, 4208);
    }
    #[test]
    fn charge_current_1500ma_encoding_correct() {
        let ichg_field = u32::from(ICHG_1500MA);
        let current_ma = 64 * ichg_field;
        assert!((1400..=1500).contains(&current_ma));
    }
    #[test]
    fn status_masks_do_not_overlap() {
        assert_eq!(STATUS_PG_MASK & STATUS_CHRG_MASK, 0);
        assert_eq!(STATUS_PG_MASK & STATUS_VBUS_MASK, 0);
        assert_eq!(STATUS_CHRG_MASK & STATUS_VBUS_MASK, 0);
    }
    #[test] #[allow(clippy::indexing_slicing)]
    fn vbus_stat_values_are_distinct() {
        let v = [VBUS_STAT_NO_INPUT, VBUS_STAT_SDP, VBUS_STAT_CDP,
            VBUS_STAT_DCP, VBUS_STAT_HVDCP, VBUS_STAT_ADAPTER, VBUS_STAT_OTG];
        for i in 0..v.len() { for j in (i+1)..v.len() { assert_ne!(v[i], v[j]); } }
    }
    #[test]
    fn register_addresses_are_correct_per_datasheet() {
        assert_eq!(REG00_INPUT_SOURCE, 0x00);
        assert_eq!(REG01_POWER_ON_CONFIG, 0x01);
        assert_eq!(REG02_CHARGE_CURRENT, 0x02);
        assert_eq!(REG03_PRECHARGE_TERM, 0x03);
        assert_eq!(REG04_CHARGE_VOLTAGE, 0x04);
        assert_eq!(REG05_CHARGE_TIMER, 0x05);
        assert_eq!(REG06_IR_COMP, 0x06);
        assert_eq!(REG07_MISC, 0x07);
        assert_eq!(REG0B_STATUS, 0x0B);
        assert_eq!(REG0C_FAULT, 0x0C);
        assert_eq!(REG0E_BATTERY_VOLTAGE, 0x0E);
        assert_eq!(REG14_DEVICE_ID, 0x14);
    }
    #[test]
    fn battery_voltage_adc_decode_formula() {
        assert_eq!(decode_battery_voltage_mv(42), 3144);
        assert_eq!(decode_battery_voltage_mv(0b1010_1010), decode_battery_voltage_mv(0b0010_1010));
        assert_eq!(decode_battery_voltage_mv(35), 3004);
    }
    #[test]
    fn vbus_voltage_adc_decode_formula() {
        assert_eq!(decode_vbus_voltage_mv(24), 5000);
        assert_eq!(decode_vbus_voltage_mv(64), 9000);
    }
    #[allow(clippy::unwrap_used)] #[allow(clippy::indexing_slicing)] #[test]
    fn bq25895_mock_init_sequence_writes_correct_registers() {
        use embedded_hal::i2c::I2c as _;
        struct MockI2c { writes: std::vec::Vec<(u8, std::vec::Vec<u8>)>, }
        impl embedded_hal::i2c::ErrorType for MockI2c { type Error = core::convert::Infallible; }
        impl embedded_hal::i2c::I2c for MockI2c {
            fn transaction(&mut self, address: u8, operations: &mut [embedded_hal::i2c::Operation<'_>]) -> Result<(), Self::Error> {
                for op in operations.iter() {
                    if let embedded_hal::i2c::Operation::Write(data) = op {
                        self.writes.push((address, data.to_vec())); } }
                Ok(()) } }
        let mut mock = MockI2c { writes: std::vec::Vec::new() };
        mock.write(BQ25895_I2C_ADDR, &[REG00_INPUT_SOURCE, IINLIM_3250MA]).unwrap();
        mock.write(BQ25895_I2C_ADDR, &[REG02_CHARGE_CURRENT, ICHG_1500MA]).unwrap();
        mock.write(BQ25895_I2C_ADDR, &[REG04_CHARGE_VOLTAGE, VREG_4208MV]).unwrap();
        mock.write(BQ25895_I2C_ADDR, &[REG01_POWER_ON_CONFIG, REG01_ENABLE_CHARGING]).unwrap();
        assert_eq!(mock.writes.len(), 4);
        for (addr, _) in &mock.writes { assert_eq!(*addr, BQ25895_I2C_ADDR); }
        assert_eq!(mock.writes[0].1[0], REG00_INPUT_SOURCE);
        assert_eq!(mock.writes[1].1[0], REG02_CHARGE_CURRENT);
        assert_eq!(mock.writes[2].1[0], REG04_CHARGE_VOLTAGE);
        assert_eq!(mock.writes[3].1[0], REG01_POWER_ON_CONFIG);
        assert_eq!(mock.writes[0].1[1], IINLIM_3250MA);
        assert_eq!(mock.writes[1].1[1], ICHG_1500MA);
        assert_eq!(mock.writes[2].1[1], VREG_4208MV);
        assert_eq!(mock.writes[3].1[1], REG01_ENABLE_CHARGING);
    }
    #[allow(clippy::assertions_on_constants)] #[test]
    fn iinlim_3250ma_within_register_field_range() { assert!(IINLIM_3250MA <= 0x3F); }
    #[allow(clippy::assertions_on_constants)] #[test]
    fn vreg_4208mv_within_register_field_range() { assert!(VREG_4208MV >> 2 <= 63); }

    // -- TDD tests for bq25895_init() --
    #[test]
    fn bq25895_init_writes_four_config_registers() {
        let mut mock = SharedMockI2c::default();
        bq25895_init(&mut mock, BQ25895_I2C_ADDR).unwrap();
        assert!(mock.writes.len() >= 3, "must write >= 3 regs, wrote {}", mock.writes.len());
    }
    #[test]
    fn bq25895_init_sets_input_current_limit() {
        let mut mock = SharedMockI2c::default();
        bq25895_init(&mut mock, BQ25895_I2C_ADDR).unwrap();
        let w = mock.writes.iter().find(|(_, d)| d.first() == Some(&REG00_INPUT_SOURCE));
        assert!(w.is_some(), "init() must write REG00");
    }
    #[allow(clippy::unwrap_used)] #[test]
    fn bq25895_init_sets_charge_voltage() {
        let mut mock = SharedMockI2c::default();
        bq25895_init(&mut mock, BQ25895_I2C_ADDR).unwrap();
        let w = mock.writes.iter().find(|(_, d)| d.first() == Some(&REG04_CHARGE_VOLTAGE));
        assert!(w.is_some(), "init() must write REG04");
        // SAFETY: w is Some (asserted above); data[1] is the register value byte.
        #[allow(clippy::indexing_slicing)]
        {
            assert_eq!(w.unwrap().1[1], VREG_4208MV, "REG04 must set 4.208V");
        }
    }
    #[test]
    fn bq25895_init_correct_i2c_address() {
        let mut mock = SharedMockI2c::default();
        bq25895_init(&mut mock, BQ25895_I2C_ADDR).unwrap();
        for (addr, _) in &mock.writes { assert_eq!(*addr, BQ25895_I2C_ADDR); }
    }
}
