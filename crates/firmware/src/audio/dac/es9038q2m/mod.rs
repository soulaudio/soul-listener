//! ES9038Q2M DAC driver module (ESS Technology)
//!
//! The concrete driver implementation is hardware-only (`hardware` feature).
//! On host builds only the register map is compiled for reference.
//! The driver module is also compiled during `cargo test` so that the
//! `#[cfg(test)]` block inside `driver.rs` runs on the host test harness.

pub mod registers;

// Include the driver module for hardware targets and also during host tests.
// Tests use `embedded_hal_mock` as a stand-in for the real I²C bus.
#[cfg(any(feature = "hardware", test))]
mod driver;

#[cfg(feature = "hardware")]
pub use driver::Es9038q2mDriver;
