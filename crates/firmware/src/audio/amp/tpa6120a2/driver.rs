//! TPA6120A2 hardware driver for STM32H7
//!
//! Controls the amplifier via a single `SHUTDOWN` GPIO output pin.
//! Uses `embedded_hal::digital::OutputPin` (v1.0) — the toggle is
//! instantaneous so no async operations are required on the GPIO side.

use embedded_hal::digital::OutputPin;

use crate::audio::amp::AmpDriver;

/// TPA6120A2 headphone amplifier driver.
///
/// Holds ownership of the `SHUTDOWN` GPIO pin and tracks the enabled state.
pub struct Tpa6120a2<P: OutputPin> {
    shutdown_pin: P,
    enabled: bool,
}

impl<P: OutputPin> Tpa6120a2<P> {
    /// Create a new TPA6120A2 driver.
    ///
    /// Takes ownership of the `SHUTDOWN` GPIO pin and immediately drives it
    /// low, placing the amplifier in shutdown (disabled) state.
    #[allow(clippy::expect_used)] // hardware init panic is intentional — firmware cannot continue without SHUTDOWN pin
    pub fn new(mut shutdown_pin: P) -> Self {
        // Drive low on construction — amplifier starts disabled.
        // Panics on GPIO failure: if SHUTDOWN cannot be driven low during init,
        // the hardware is in an unknown state and the firmware must not continue.
        shutdown_pin
            .set_low()
            .expect("SHUTDOWN pin set_low failed during TPA6120A2 init");
        Self {
            shutdown_pin,
            enabled: false,
        }
    }
}

impl<P: OutputPin> AmpDriver for Tpa6120a2<P> {
    type Error = P::Error;

    /// Enable the amplifier by driving `SHUTDOWN` high.
    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.shutdown_pin.set_high()?;
        self.enabled = true;
        Ok(())
    }

    /// Disable the amplifier by driving `SHUTDOWN` low.
    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.shutdown_pin.set_low()?;
        self.enabled = false;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::digital::{
        Mock as PinMock, State as PinState, Transaction as PinTransaction,
    };

    // -----------------------------------------------------------------------
    // Test A: new() drives SHUTDOWN low immediately
    // -----------------------------------------------------------------------

    /// Verify that `new()` drives the SHUTDOWN pin low on construction.
    ///
    /// Status: PASSES — `new()` calls `set_low()` once, which matches the
    /// single mock transaction configured here.
    #[test]
    fn test_new_drives_pin_low() {
        // new() must drive SHUTDOWN low (amp disabled) on construction
        let pin = PinMock::new(&[PinTransaction::set(PinState::Low)]);
        let _drv = Tpa6120a2::new(pin.clone());
        pin.clone().done(); // panics if transactions don't match
    }

    // -----------------------------------------------------------------------
    // Test E: new() starts in disabled state
    // -----------------------------------------------------------------------

    /// Verify that the driver reports `is_enabled() == false` immediately
    /// after construction.
    ///
    /// Status: PASSES.
    #[test]
    fn test_new_starts_disabled() {
        let pin = PinMock::new(&[PinTransaction::set(PinState::Low)]);
        let drv = Tpa6120a2::new(pin.clone());
        assert!(!drv.is_enabled());
        pin.clone().done();
    }

    // -----------------------------------------------------------------------
    // Test B: enable() drives SHUTDOWN high
    // -----------------------------------------------------------------------

    /// Verify that `enable()` drives SHUTDOWN high and `is_enabled()` becomes true.
    ///
    /// Status: PASSES with tokio runtime.
    #[tokio::test]
    async fn test_enable_drives_pin_high() {
        let pin = PinMock::new(&[
            PinTransaction::set(PinState::Low),  // new()
            PinTransaction::set(PinState::High), // enable()
        ]);
        let mut drv = Tpa6120a2::new(pin.clone());
        drv.enable().await.unwrap();
        assert!(drv.is_enabled());
        pin.clone().done();
    }

    // -----------------------------------------------------------------------
    // Test C: disable() after enable() drives SHUTDOWN low
    // -----------------------------------------------------------------------

    /// Verify that `disable()` after `enable()` drives SHUTDOWN low and
    /// `is_enabled()` returns false.
    ///
    /// Status: PASSES.
    #[tokio::test]
    async fn test_disable_after_enable() {
        let pin = PinMock::new(&[
            PinTransaction::set(PinState::Low),  // new()
            PinTransaction::set(PinState::High), // enable()
            PinTransaction::set(PinState::Low),  // disable()
        ]);
        let mut drv = Tpa6120a2::new(pin.clone());
        drv.enable().await.unwrap();
        assert!(drv.is_enabled());
        drv.disable().await.unwrap();
        assert!(!drv.is_enabled());
        pin.clone().done();
    }

    // -----------------------------------------------------------------------
    // Test D: calling enable() twice sets the pin high twice
    // -----------------------------------------------------------------------

    /// Verify that calling `enable()` twice drives set_high twice.
    ///
    /// Current behaviour: each `enable()` call unconditionally drives the pin
    /// high regardless of current state. This is acceptable for GPIO (the
    /// operation is idempotent on real hardware) and this test documents that
    /// contract.
    ///
    /// Status: PASSES — mock expects two set_high calls.
    #[tokio::test]
    async fn test_double_enable_sets_pin_twice() {
        // Current impl: calling enable() twice drives set_high twice.
        // This is acceptable for GPIO (idempotent in hardware) but documents the behavior.
        let pin = PinMock::new(&[
            PinTransaction::set(PinState::Low),  // new()
            PinTransaction::set(PinState::High), // enable()
            PinTransaction::set(PinState::High), // enable() again
        ]);
        let mut drv = Tpa6120a2::new(pin.clone());
        drv.enable().await.unwrap();
        drv.enable().await.unwrap();
        assert!(drv.is_enabled());
        pin.clone().done();
    }
}
