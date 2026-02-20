//! Hardware Abstraction Layer (HAL) for `SoulAudio` DAP
//!
//! This crate provides trait-based abstractions for all hardware components,
//! enabling development and testing without physical hardware.
//!
//! # Architecture Layers
//!
//! ```text
//! Application Layer (firmware crate)
//!         ↓
//! Feature Layers (playback, ui, library, bluetooth)
//!         ↓
//! Platform HAL (this crate - trait abstractions)
//!         ↓
//! Hardware Layer (Embassy HAL + PAC)
//! ```
//!
//! # Abstraction Levels
//!
//! ## High-Level Peripherals
//! - [`DisplayDriver`] - E-ink display control
//! - [`InputDevice`] - Button and rotary encoder input
//! - [`AudioCodec`] - Audio output
//! - [`Storage`] - File system access
//! - [`BluetoothAdapter`] - Wireless connectivity
//!
//! ## Mid-Level Peripherals
//! - [`gpio`] - Pin control with typestate
//! - [`peripheral`] - SPI, I2C, UART abstractions
//! - [`dma`] - DMA transfer management
//! - [`power`] - Power management
//!
//! # Features
//!
//! - `std`: Enable standard library support (for testing)
//! - `simulator`: Desktop simulator implementations
//! - `hardware`: Physical hardware implementations
//! - `defmt`: Enable defmt logging
//!
//! # Example
//!
//! ```no_run
//! use platform::{DisplayDriver, InputEvent};
//!
//! async fn example<D: DisplayDriver>(display: &mut D) {
//!     display.refresh_full().await.unwrap();
//! }
//! ```

// ── Lint policy ─────────────────────────────────────────────────────────────
#![deny(clippy::unwrap_used)] // no .unwrap() in production code
#![deny(clippy::expect_used)] // no .expect() in production code
#![deny(clippy::panic)] // no panic!() in production code
#![deny(clippy::unreachable)] // no unreachable!() that isn't documented
#![deny(unused_must_use)]
// all Results must be handled
// ────────────────────────────────────────────────────────────────────────────
#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(unsafe_op_in_unsafe_fn)] // unsafe fn body is not implicitly unsafe block
#![warn(clippy::print_stdout)] // prefer tracing/defmt over println! in lib code
// Pedantic lints suppressed for this hardware HAL crate:
#![allow(clippy::doc_markdown)] // hex addresses and register names in doc comments
#![allow(clippy::missing_panics_doc)] // statically-valid expect() with safety comments
#![allow(clippy::must_use_candidate)] // hardware accessors — callers decide
#![allow(clippy::match_same_arms)] // intentional for readability in DMA access tables
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(async_fn_in_trait)] // Embassy no_std: single-threaded, Send bounds not needed

pub mod asset_store;
pub mod audio;
pub mod audio_config;
pub mod audio_types;
pub mod bluetooth;
pub mod clock_config;
pub mod config;
pub mod display;
pub mod dma;
pub mod dma_safety;
pub mod gpio;
pub mod input;
pub mod mpu;
pub mod peripheral;
pub mod power;
pub mod qspi_config;
pub mod sdram;
pub mod storage;
pub mod storage_config;

// Re-export main high-level traits
pub use asset_store::{AssetKey, AssetStore};
pub use audio::{AudioCodec, AudioConfig, DsdMode, OversamplingFilter};
pub use bluetooth::BluetoothAdapter;
pub use display::{DisplayDriver, DisplayError, DisplayInfo, EinkDisplay, RefreshMode};
pub use input::{Button, InputDevice, InputEvent};
pub use sdram::{ExternalRam, RamRegion};
pub use storage::{File, Storage};

// Re-export GPIO types
pub use gpio::{
    Analog, Input, InputPin, InterruptMode, InterruptPin, Output, OutputPin, Pin, PinGroup,
    PinState,
};

// Re-export peripheral types
pub use peripheral::{
    AddressMode, BitOrder, DataBits, I2cConfig, I2cPeripheral, Parity, SpiConfig, SpiMode,
    SpiPeripheral, StopBits, UartConfig, UartPeripheral,
};

// Re-export DMA types
pub use dma::{CircularBuffer, DmaBuffer, DmaBufferMut, DmaChannel, DmaTransfer};

// Re-export power types
pub use power::{Peripheral, PowerManager, PowerMonitor, SleepMode, VoltageScale, WakeSource};
