//! Input driver selection.
//!
//! At compile time exactly one concrete driver is selected via Cargo features:
//!
//! | Feature          | Driver              | Source           |
//! |------------------|---------------------|------------------|
//! | `keyboard-input` | [`EmulatorInput`]   | winit keyboard   |
//! | `hardware`       | [`HardwareInput`]   | GPIO / encoder   |
//!
//! Both implement [`platform::InputDevice`], so application code is identical
//! across targets.
//!
//! A fluent [`builder`] API allows declaring input sources with debounce
//! configuration and emulated key mappings without touching platform-specific
//! code at the call site.
//!
//! # Example
//!
//! ```no_run
//! use firmware::input::{InputDevice as _, InputEvent};
//!
//! async fn input_task(mut input: impl firmware::input::InputDevice) {
//!     loop {
//!         let ev = input.wait_for_event().await;
//!         println!("{:?}", ev);
//!     }
//! }
//! ```

// Re-export the trait and event types so callers only need `firmware::input`.
pub use platform::{Button, InputDevice, InputEvent};

/// Desktop emulator driver (keyboard + scroll wheel).
#[cfg(feature = "keyboard-input")]
pub use eink_emulator::input::EmulatorInput;

/// Hardware GPIO driver (rotary encoder + debounced buttons).
#[cfg(feature = "hardware")]
pub mod hardware;
#[cfg(feature = "hardware")]
pub use hardware::HardwareInput;

/// Fluent builder API for configuring input sources.
pub mod builder;
pub use builder::{EmulatedAxis, EmulatedKey, InputBuilder};
