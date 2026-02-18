//! Hardware Abstraction Layer (HAL) for SoulAudio DAP
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

#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod audio;
pub mod bluetooth;
pub mod config;
pub mod display;
pub mod dma;
pub mod gpio;
pub mod input;
pub mod peripheral;
pub mod power;
pub mod storage;

// Re-export main high-level traits
pub use audio::{AudioCodec, AudioConfig};
pub use bluetooth::BluetoothAdapter;
pub use display::{DisplayDriver, DisplayError, EinkDisplay, RefreshMode};
pub use input::{Button, InputDevice, InputEvent};
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
