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

// ── Lint policy ─────────────────────────────────────────────────────────────
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(unused_must_use)]
// Note: panic! allowed in firmware main task panics (handled by panic-probe)
// but not in library code. Use defmt::panic! with context on hardware.
// Note: build.rs is not a lib file — clippy::unwrap_used does not cover it.
// ────────────────────────────────────────────────────────────────────────────
#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
// Upgrade relevant warns to deny; keep pedantic as warn (too noisy for firmware)
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// Critical correctness: deny these
#![deny(unsafe_op_in_unsafe_fn)]
// unsafe fn body is not implicitly unsafe block
// Logging discipline (allow println in tests via clippy.toml)
#![warn(clippy::print_stdout)] // prefer tracing/defmt over println! in lib code
// Intentional allows for this codebase:
#![allow(clippy::module_name_repetitions)] // common in Rust crates; not a real issue
#![allow(clippy::missing_errors_doc)] // most errors are self-explanatory
// Pedantic lints too noisy for firmware application code:
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::similar_names)]
#![allow(clippy::unused_self)]
#![allow(clippy::unused_async)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::uninlined_format_args)]

// ── HARDWARE INIT REQUIREMENTS ───────────────────────────────────────────────
// Embassy issue #3049: SDMMC on STM32H743 silently hangs during init_card()
// unless HSI48 is enabled in RCC BEFORE initializing SDMMC.
// In main.rs hardware init, ensure RCC configuration enables HSI48:
//
//   config.rcc.hsi48 = Some(Hsi48Config { sync_from_usb: false });
//
// Failure to do this produces a silent chip lockup with no error code.
// ─────────────────────────────────────────────────────────────────────────────

// ── Critical Hardware Constraints (DO NOT IGNORE) ────────────────────────────
//
// ### BDMA Peripheral Buffer Placement
// Peripherals served by BDMA (SPI6, I2C4, LPUART1, ADC3, SAI4) can ONLY
// DMA to/from SRAM4 (`0x3800_0000`, 64KB). Using any other region causes
// silent transfer failure. Use `#[link_section = ".sram4"]` for BDMA buffers.
//
// ### QSPI Memory-Mapped (XiP) Mode
// Embassy issue #3149: `embassy_stm32::qspi` does NOT implement memory-mapped
// mode. To enable XiP from QSPI NOR flash, you must write PAC-level registers:
//
//   QUADSPI.CCR: FMODE = 0b11 (memory-mapped)
//   QUADSPI.AR:  set base address (0x9000_0000)
//   QUADSPI.CR:  TCEN = 0 (disable timeout)
//
// This must be done AFTER firmware has finished using QSPI in command mode.
// ─────────────────────────────────────────────────────────────────────────────

pub mod audio;
pub mod boot;
pub mod display;
pub mod dma;
pub mod exception_handlers;
pub mod hal;
pub mod ui;

#[cfg(any(feature = "keyboard-input", feature = "hardware"))]
pub mod input;

// Re-export key types
pub use display::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE, GDEM0397T81P_SPEC};
pub use hal::{Color, DapDisplay, DisplayConfig};

#[cfg(any(test, feature = "emulator"))]
pub use audio::{MockAmp, MockDac};

// SSD1677 driver is always available (generic over HAL traits, no hardware gate).
pub use display::{DisplayError, Ssd1677};

#[cfg(feature = "hardware")]
pub use audio::Es9038q2mDriver;

#[cfg(feature = "hardware")]
pub use audio::Tpa6120a2;

// Legacy type alias (used by main.rs on the embedded target)
#[cfg(feature = "hardware")]
pub use display::Ssd1677Display;

#[cfg(feature = "emulator")]
pub use display::EmulatorDisplay;
