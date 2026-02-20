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
// Upgrade relevant warns to deny; keep pedantic as warn (too noisy for firmware)
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// Critical correctness: deny these
#![deny(clippy::await_holding_lock)] // holding a blocking Mutex across .await is a bug
#![deny(unsafe_op_in_unsafe_fn)]
// unsafe fn body is not implicitly unsafe block
// Logging discipline (allow println in tests via clippy.toml)
#![warn(clippy::print_stdout)] // prefer tracing/defmt over println! in lib code
#![warn(clippy::dbg_macro)] // dbg! should not be left in committed code
// Intentional allows for this codebase:
#![allow(clippy::module_name_repetitions)] // common in Rust crates; not a real issue
#![allow(clippy::missing_errors_doc)] // most errors are self-explanatory
// Pedantic lints too noisy for firmware application code:
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::similar_names)]
#![allow(clippy::unused_self)]
#![allow(clippy::unused_async)]

pub mod audio;
pub mod display;
pub mod hal;
pub mod ui;

#[cfg(any(feature = "keyboard-input", feature = "hardware"))]
pub mod input;

// Re-export key types
pub use display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE, GDEM0397T81P_SPEC};
pub use hal::{Color, DapDisplay, DisplayConfig};

pub use audio::MockAmp;
pub use audio::MockDac;

#[cfg(feature = "hardware")]
pub use audio::Es9038q2mDriver;

#[cfg(feature = "hardware")]
pub use audio::Tpa6120a2;

#[cfg(feature = "hardware")]
pub use display::Ssd1677Display;

#[cfg(feature = "emulator")]
pub use display::EmulatorDisplay;
