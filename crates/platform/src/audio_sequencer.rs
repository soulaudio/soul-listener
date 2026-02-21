//! Audio power sequencing typestate machine.
//!
//! Enforces the safe power-on/off ordering for TPA6120A2 + ES9038Q2M:
//!
//! ## Power-on sequence (prevents audible pop):
//! [DacOutputting] --mute_dac()--> [DacMuted] --enable_amp()--> [AmpEnabled] --unmute_dac()--> [FullyOn]
//!
//! ## Power-off sequence:
//! [FullyOn] --mute_dac_for_shutdown()--> [DacMuted] --disable_amp()--> [DacOutputting]
//!
//! The ES9038Q2M register map: register 15 (ATT_L) = 0xFF -> muted, 0x00 -> 0 dB.

use core::marker::PhantomData;

/// DAC is outputting audio; amplifier is disabled (SHUTDOWN low).
pub struct DacOutputting;

/// DAC attenuation set to maximum (muted); amplifier still disabled.
pub struct DacMuted;

/// Amplifier enabled (SHUTDOWN high); DAC is still muted.
pub struct AmpEnabled;

/// Fully operational: DAC outputting audio, amplifier enabled.
pub struct FullyOn;

/// Typestate machine for audio power sequencing.
pub struct AudioPowerSequencer<State> {
    _state: PhantomData<State>,
}

impl AudioPowerSequencer<DacOutputting> {
    /// Create sequencer in initial state (DAC running, amp off).
    #[must_use]
    pub fn new() -> Self { Self { _state: PhantomData } }

    /// Mute the DAC (set ATT registers to 0xFF) before enabling the amplifier.
    ///
    /// Stub variant: no I2C write. Use `mute_dac_with_i2c` in production firmware.
    #[deprecated(
        since = "0.1.0",
        note = "Use mute_dac_with_i2c() for hardware builds. This stub does nothing."
    )]
    #[must_use]
    pub fn mute_dac(self) -> AudioPowerSequencer<DacMuted> {
        AudioPowerSequencer { _state: PhantomData }
    }

    /// Mute the ES9038Q2M DAC by writing 0xFF to ATT_L (reg 15) and ATT_R (reg 16).
    ///
    /// # Errors
    ///
    /// Propagates any I2C bus error returned by the underlying HAL driver.
    pub fn mute_dac_with_i2c<I2C, E>(
        self,
        i2c: &mut I2C,
        addr: u8,
    ) -> Result<AudioPowerSequencer<DacMuted>, E>
    where
        I2C: embedded_hal::i2c::I2c<Error = E>,
    {
        i2c.write(addr, &[crate::es9038q2m::REG_ATT_L, crate::es9038q2m::ATT_MUTED])?;
        i2c.write(addr, &[crate::es9038q2m::REG_ATT_R, crate::es9038q2m::ATT_MUTED])?;
        Ok(AudioPowerSequencer { _state: PhantomData })
    }
}

impl AudioPowerSequencer<DacMuted> {
    /// Enable the headphone amplifier (raise TPA6120A2 SHUTDOWN pin).
    ///
    /// Stub variant: no GPIO write. Use `enable_amp_with_gpio` in production firmware.
    #[deprecated(
        since = "0.1.0",
        note = "Use enable_amp_with_gpio() for hardware builds. This stub does nothing."
    )]
    #[must_use]
    pub fn enable_amp(self) -> AudioPowerSequencer<AmpEnabled> {
        AudioPowerSequencer { _state: PhantomData }
    }

    /// Enable the TPA6120A2 headphone amplifier by driving SHUTDOWN high.
    ///
    /// # Errors
    ///
    /// Returns the GPIO error type E if the pin write fails.
    pub fn enable_amp_with_gpio<GPIO, E>(
        self,
        gpio: &mut GPIO,
    ) -> Result<AudioPowerSequencer<AmpEnabled>, E>
    where
        GPIO: embedded_hal::digital::OutputPin<Error = E>,
    {
        gpio.set_high()?;
        Ok(AudioPowerSequencer { _state: PhantomData })
    }

    /// Disable amplifier during power-down (returns to initial state).
    ///
    /// Stub variant: no GPIO write. Use `disable_amp_with_gpio` in production firmware.
    #[deprecated(
        since = "0.1.0",
        note = "Use disable_amp_with_gpio() for hardware builds. This stub does nothing."
    )]
    #[must_use]
    pub fn disable_amp(self) -> AudioPowerSequencer<DacOutputting> {
        AudioPowerSequencer { _state: PhantomData }
    }

    /// Disable the TPA6120A2 headphone amplifier by driving SHUTDOWN low.
    ///
    /// # Errors
    ///
    /// Returns the GPIO error type E if the pin write fails.
    pub fn disable_amp_with_gpio<GPIO, E>(
        self,
        gpio: &mut GPIO,
    ) -> Result<AudioPowerSequencer<DacOutputting>, E>
    where
        GPIO: embedded_hal::digital::OutputPin<Error = E>,
    {
        gpio.set_low()?;
        Ok(AudioPowerSequencer { _state: PhantomData })
    }
}

impl AudioPowerSequencer<AmpEnabled> {
    /// Unmute the DAC (restore ATT registers to desired volume level).
    ///
    /// Stub variant: no I2C write. Use `unmute_dac_with_i2c` in production firmware.
    #[deprecated(
        since = "0.1.0",
        note = "Use unmute_dac_with_i2c() for hardware builds. This stub does nothing."
    )]
    #[must_use]
    pub fn unmute_dac(self) -> AudioPowerSequencer<FullyOn> {
        AudioPowerSequencer { _state: PhantomData }
    }

    /// Unmute the ES9038Q2M DAC by writing 0x00 to ATT_L (reg 15) and ATT_R (reg 16).
    ///
    /// # Errors
    ///
    /// Propagates any I2C bus error returned by the underlying HAL driver.
    pub fn unmute_dac_with_i2c<I2C, E>(
        self,
        i2c: &mut I2C,
        addr: u8,
    ) -> Result<AudioPowerSequencer<FullyOn>, E>
    where
        I2C: embedded_hal::i2c::I2c<Error = E>,
    {
        i2c.write(addr, &[crate::es9038q2m::REG_ATT_L, crate::es9038q2m::ATT_FULL_VOLUME])?;
        i2c.write(addr, &[crate::es9038q2m::REG_ATT_R, crate::es9038q2m::ATT_FULL_VOLUME])?;
        Ok(AudioPowerSequencer { _state: PhantomData })
    }
}

impl AudioPowerSequencer<FullyOn> {
    /// Mute the DAC as the first step of power-down.
    ///
    /// Stub variant: no I2C write. Use `mute_dac_for_shutdown_with_i2c` in production firmware.
    #[deprecated(
        since = "0.1.0",
        note = "Use mute_dac_for_shutdown_with_i2c() for hardware builds. This stub does nothing."
    )]
    #[must_use]
    pub fn mute_dac_for_shutdown(self) -> AudioPowerSequencer<DacMuted> {
        AudioPowerSequencer { _state: PhantomData }
    }

    /// Mute the ES9038Q2M DAC as the first step of power-down via I2C.
    ///
    /// # Errors
    ///
    /// Propagates any I2C bus error returned by the underlying HAL driver.
    pub fn mute_dac_for_shutdown_with_i2c<I2C, E>(
        self,
        i2c: &mut I2C,
        addr: u8,
    ) -> Result<AudioPowerSequencer<DacMuted>, E>
    where
        I2C: embedded_hal::i2c::I2c<Error = E>,
    {
        i2c.write(addr, &[crate::es9038q2m::REG_ATT_L, crate::es9038q2m::ATT_MUTED])?;
        i2c.write(addr, &[crate::es9038q2m::REG_ATT_R, crate::es9038q2m::ATT_MUTED])?;
        Ok(AudioPowerSequencer { _state: PhantomData })
    }
}

impl Default for AudioPowerSequencer<DacOutputting> {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
#[allow(deprecated)] // Tests legitimately exercise stub methods to verify typestate transitions
mod tests {
    use super::*;
    use crate::es9038q2m::{ATT_FULL_VOLUME, ATT_MUTED, REG_ATT_L, REG_ATT_R};

    struct MockI2c { writes: std::vec::Vec<(u8, u8, u8)> }

    impl MockI2c {
        fn new() -> Self { Self { writes: std::vec::Vec::new() } }
    }

    impl embedded_hal::i2c::ErrorType for MockI2c {
        type Error = core::convert::Infallible;
    }

    impl embedded_hal::i2c::I2c for MockI2c {
        fn transaction(
            &mut self,
            addr: u8,
            ops: &mut [embedded_hal::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
            for op in ops {
                if let embedded_hal::i2c::Operation::Write(data) = op {
                    if data.len() >= 2 {
                        // SAFETY: guarded by `data.len() >= 2` above.
                        #[allow(clippy::indexing_slicing)]
                        self.writes.push((addr, data[0], data[1]));
                    }
                }
            }
            Ok(())
        }
    }

    struct MockGpio { high: bool }

    impl MockGpio {
        fn new() -> Self { Self { high: false } }
    }

    impl embedded_hal::digital::ErrorType for MockGpio {
        type Error = core::convert::Infallible;
    }

    impl embedded_hal::digital::OutputPin for MockGpio {
        fn set_high(&mut self) -> Result<(), Self::Error> { self.high = true; Ok(()) }
        fn set_low(&mut self) -> Result<(), Self::Error> { self.high = false; Ok(()) }
    }
    /// Muting the DAC must write 0xFF to both ATT_L (reg 15) and ATT_R (reg 16).
    #[test]
    fn mute_dac_writes_i2c_attenuation_registers() {
        let mut mock = MockI2c::new();
        let addr = crate::es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
        let _muted = AudioPowerSequencer::<DacOutputting>::new()
            .mute_dac_with_i2c(&mut mock, addr)
            .expect("mute_dac_with_i2c must not fail with MockI2c");
        assert!(
            mock.writes.contains(&(addr, REG_ATT_L, ATT_MUTED)),
            "must write 0xFF to REG_ATT_L; writes: {:?}",
            mock.writes
        );
        assert!(
            mock.writes.contains(&(addr, REG_ATT_R, ATT_MUTED)),
            "must write 0xFF to REG_ATT_R; writes: {:?}",
            mock.writes
        );
    }

    /// Unmuting the DAC must write 0x00 to both ATT_L (reg 15) and ATT_R (reg 16).
    #[test]
    fn unmute_dac_writes_volume_registers() {
        let addr = crate::es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
        let mut mute_mock = MockI2c::new();
        let muted = AudioPowerSequencer::<DacOutputting>::new()
            .mute_dac_with_i2c(&mut mute_mock, addr)
            .expect("mute step must succeed");
        let mut gpio = MockGpio::new();
        let amp_on = muted.enable_amp_with_gpio(&mut gpio).expect("enable_amp");
        let mut mock = MockI2c::new();
        let _on = amp_on.unmute_dac_with_i2c(&mut mock, addr).expect("unmute");
        assert!(
            mock.writes.contains(&(addr, REG_ATT_L, ATT_FULL_VOLUME)),
            "must write 0x00 to REG_ATT_L; writes: {:?}",
            mock.writes
        );
        assert!(
            mock.writes.contains(&(addr, REG_ATT_R, ATT_FULL_VOLUME)),
            "must write 0x00 to REG_ATT_R; writes: {:?}",
            mock.writes
        );
    }
    /// Enabling the amplifier must drive the SHUTDOWN GPIO pin high.
    #[test]
    fn enable_amp_drives_gpio_high() {
        let mut i2c = MockI2c::new();
        let addr = crate::es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
        let muted = AudioPowerSequencer::<DacOutputting>::new()
            .mute_dac_with_i2c(&mut i2c, addr).expect("mute");
        let mut gpio = MockGpio::new();
        assert!(!gpio.high, "GPIO must start low");
        let _amp = muted.enable_amp_with_gpio(&mut gpio).expect("amp");
        assert!(gpio.high, "enable_amp_with_gpio must drive SHUTDOWN high");
    }

    /// Disabling the amplifier must drive the SHUTDOWN GPIO pin low.
    ///
    /// The disable_amp_with_gpio transition is available from DacMuted state.
    /// This test verifies the power-down path: after enabling the amp,
    /// mute the DAC first (mute_dac_for_shutdown_with_i2c on FullyOn), then disable.
    #[test]
    fn disable_amp_drives_gpio_low() {
        let mut i2c = MockI2c::new();
        let addr = crate::es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
        let mut gpio = MockGpio::new();
        // Power-on sequence to reach FullyOn
        let fully_on: AudioPowerSequencer<FullyOn> = AudioPowerSequencer::new()
            .mute_dac_with_i2c(&mut i2c, addr).expect("mute")
            .enable_amp_with_gpio(&mut gpio).expect("enable amp")
            .unmute_dac_with_i2c(&mut i2c, addr).expect("unmute");
        assert!(gpio.high, "GPIO should be high after enable");
        // Power-down: mute DAC first, then disable amp
        let muted: AudioPowerSequencer<DacMuted> =
            fully_on.mute_dac_for_shutdown_with_i2c(&mut i2c, addr).expect("mute shutdown");
        let _off: AudioPowerSequencer<DacOutputting> =
            muted.disable_amp_with_gpio(&mut gpio).expect("disable amp");
        assert!(!gpio.high, "disable_amp_with_gpio must drive SHUTDOWN low");
    }

    /// Full power-on sequence: validates I2C writes and GPIO state.
    #[test]
    // Indices 0–3 are safe: the preceding assert_eq!(len, 4) would fail first.
    #[allow(clippy::indexing_slicing)]
    fn full_power_on_sequence_produces_fully_on_state() {
        let addr = crate::es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
        let mut i2c = MockI2c::new();
        let mut gpio = MockGpio::new();
        let fully_on: AudioPowerSequencer<FullyOn> = AudioPowerSequencer::new()
            .mute_dac_with_i2c(&mut i2c, addr).expect("mute")
            .enable_amp_with_gpio(&mut gpio).expect("amp")
            .unmute_dac_with_i2c(&mut i2c, addr).expect("unmute");
        assert!(gpio.high, "amp should be enabled in FullyOn state");
        assert_eq!(i2c.writes.len(), 4,
            "Expected 4 I2C writes (mute L+R, unmute L+R); got: {:?}", i2c.writes);
        assert_eq!(i2c.writes[0], (addr, REG_ATT_L, ATT_MUTED));
        assert_eq!(i2c.writes[1], (addr, REG_ATT_R, ATT_MUTED));
        assert_eq!(i2c.writes[2], (addr, REG_ATT_L, ATT_FULL_VOLUME));
        assert_eq!(i2c.writes[3], (addr, REG_ATT_R, ATT_FULL_VOLUME));
        let _ = fully_on;
    }

    /// Power-down sequence from FullyOn: mute -> disable amp. GPIO goes low.
    #[test]
    fn power_down_sequence_from_fully_on() {
        let addr = crate::es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
        let mut i2c = MockI2c::new();
        let mut gpio = MockGpio::new();
        let fully_on: AudioPowerSequencer<FullyOn> = AudioPowerSequencer::new()
            .mute_dac_with_i2c(&mut i2c, addr).expect("mute")
            .enable_amp_with_gpio(&mut gpio).expect("amp on")
            .unmute_dac_with_i2c(&mut i2c, addr).expect("unmute");
        assert!(gpio.high, "amp enabled before shutdown");
        let muted: AudioPowerSequencer<DacMuted> =
            fully_on.mute_dac_for_shutdown_with_i2c(&mut i2c, addr).expect("mute shutdown");
        let _off: AudioPowerSequencer<DacOutputting> =
            muted.disable_amp_with_gpio(&mut gpio).expect("amp off");
        assert!(!gpio.high, "amp must be disabled after power-down sequence");
    }
}
