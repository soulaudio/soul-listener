//! Audio subsystem — DAC drivers and headphone amplifier control
//!
//! Vertically sliced: one sub-directory per hardware component.
//!
//! # Structure
//!
//! - `dac/` — DAC drivers (`Es9038q2mDriver` hardware, `MockDac` for tests)
//! - `amp/` — Headphone amplifier control (`Tpa6120a2` hardware, `MockAmp` for tests)
//!
//! # Dependency Injection
//!
//! Application code targets the [`platform::AudioCodec`] trait.
//! Concrete types are injected at the call site:
//!
//! ```rust,ignore
//! async fn audio_task<C: platform::AudioCodec>(mut codec: C) { /* ... */ }
//!
//! // Hardware:
//! audio_task(Es9038q2mDriver::new(i2c));
//! // Tests:
//! audio_task(MockDac::new());
//! ```

pub mod amp;
pub mod dac;
pub mod sai_recovery;
pub mod clock_math;
pub mod sai_task;

// Re-export the primary DAC type for each build target.
#[cfg(feature = "hardware")]
pub use dac::es9038q2m::Es9038q2mDriver;

pub use dac::mock::MockDac;

// Re-export the primary amp type for each build target.
#[cfg(feature = "hardware")]
pub use amp::tpa6120a2::Tpa6120a2;

pub use amp::mock::MockAmp;
