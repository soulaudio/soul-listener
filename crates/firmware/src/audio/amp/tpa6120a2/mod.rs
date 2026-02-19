//! TPA6120A2 headphone amplifier driver (Texas Instruments)
//!
//! The TPA6120A2 is a high-performance class-AB headphone amplifier with gain
//! fixed by external resistors. It has no I²C/SPI interface — power and mute
//! are controlled via a single active-low `SHUTDOWN` GPIO pin.
//!
//! # Signal Path
//!
//! ```text
//! PCM5242 (I²S/DAC) → analog out → TPA6120A2 → 3.5 mm TRS jack
//! ```
//!
//! # SHUTDOWN Pin Logic
//!
//! ```text
//! Pin high → amplifier enabled  (audio passes through)
//! Pin low  → amplifier disabled (shutdown, ~1 µA draw)
//! ```
//!
//! # Hardware Pin
//!
//! STM32H743 — exact pin TBD (PCB layout pending).
//! Connect to TPA6120A2 SHUTDOWN pin (active-low logic):
//!   Pin high → amplifier enabled
//!   Pin low  → amplifier disabled (shutdown)

#[cfg(feature = "hardware")]
mod driver;

#[cfg(feature = "hardware")]
pub use driver::Tpa6120a2;
