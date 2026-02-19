//! Headphone amplifier driver abstractions
//!
//! Concrete drivers are feature-gated by build target:
//! - `tpa6120a2` — TI TPA6120A2 hardware driver (`hardware` feature)
//! - `mock` — In-process mock for host tests (always available)

#![allow(async_fn_in_trait)]

pub mod mock;
pub mod tpa6120a2;

pub use mock::MockAmp;

#[cfg(feature = "hardware")]
pub use tpa6120a2::Tpa6120a2;

/// Headphone amplifier driver trait.
///
/// The TPA6120A2 is controlled by a single active-low `SHUTDOWN` GPIO pin.
/// This trait abstracts that control for both hardware and mock implementations.
pub trait AmpDriver {
    /// Error type returned by amplifier operations.
    type Error: core::fmt::Debug;

    /// Enable the amplifier (bring SHUTDOWN high).
    ///
    /// After this call the amplifier is active and audio will pass through.
    async fn enable(&mut self) -> Result<(), Self::Error>;

    /// Disable the amplifier (drive SHUTDOWN low).
    ///
    /// After this call the amplifier is in shutdown mode (~1 µA current draw).
    async fn disable(&mut self) -> Result<(), Self::Error>;

    /// Returns `true` if the amplifier is currently enabled.
    fn is_enabled(&self) -> bool;
}
