//! ES9038Q2M DAC driver module (ESS Technology)
//!
//! The concrete driver implementation is hardware-only (`hardware` feature).
//! On host builds only the register map is compiled for reference.

pub mod registers;

#[cfg(feature = "hardware")]
mod driver;

#[cfg(feature = "hardware")]
pub use driver::Es9038q2mDriver;
