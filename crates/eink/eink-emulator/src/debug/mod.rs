//! Debug system for e-ink emulator
//!
//! Provides visual debug overlays, interactive inspector, and power monitoring.

#[cfg(feature = "debug")]
pub mod state;

#[cfg(feature = "debug")]
pub mod overlay;

#[cfg(feature = "debug")]
pub mod manager;

#[cfg(feature = "debug")]
pub use state::*;

#[cfg(feature = "debug")]
pub use manager::*;
