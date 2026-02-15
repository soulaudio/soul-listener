//! SoulAudio DAP Firmware
//!
//! Professional-grade Digital Audio Player firmware for STM32H7 with e-ink display.
//!
//! # Architecture
//!
//! This firmware follows a layered architecture:
//!
//! ```text
//! Application Layer (main.rs, ui)
//!         ↓
//! HAL Abstraction (hal module)
//!         ↓
//! Hardware Drivers (display, audio, etc.)
//!         ↓
//! Platform HAL (Embassy, STM32)
//! ```
//!
//! # Features
//!
//! - `hardware` - Build for STM32H7 target (embassy, embedded HAL)
//! - `emulator` - Build for desktop testing (tokio, eink-emulator)
//! - `std` - Enable standard library (for emulator and testing)
//!
//! # Examples
//!
//! ## Hardware Target
//!
//! ```bash
//! cargo build --release --target thumbv7em-none-eabihf --features hardware
//! ```
//!
//! ## Emulator Target
//!
//! ```bash
//! cargo run --example display_emulator_test --features emulator
//! ```

#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]

pub mod display;
pub mod hal;
pub mod ui;

// Re-export key types
pub use display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE, GDEM0397T81P_SPEC};
pub use hal::{Color, DapDisplay, DisplayConfig};

#[cfg(feature = "hardware")]
pub use display::Ssd1677Display;

#[cfg(feature = "emulator")]
pub use display::EmulatorDisplay;
