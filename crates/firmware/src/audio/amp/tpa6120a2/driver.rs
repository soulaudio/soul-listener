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
    pub fn new(mut shutdown_pin: P) -> Self {
        // Drive low on construction — amplifier starts disabled.
        // Ignore the error here; if the pin is broken the first enable()
        // call will surface it.
        let _ = shutdown_pin.set_low();
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
