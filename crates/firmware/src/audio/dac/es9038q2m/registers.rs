//! ES9038Q2M register map
//!
//! Source: ESS Technology ES9038Q2M Datasheet v1.4
//! <https://www.esstech.com/wp-content/uploads/2022/09/ES9038Q2M-Datasheet-v1.4.pdf>
//!
//! # Key I²C Constraints (confirmed from datasheet and community sources)
//!
//! ## Single-byte reads only
//! The ES9038Q2M does NOT support multi-byte sequential I²C reads. Attempting
//! to clock out more than one byte after a register address will cause the
//! internal I²C decoder to enter an undefined state; a full reset is required
//! to recover. Every register read must be a separate `write_read` transaction
//! that sends exactly 1 address byte and reads back exactly 1 data byte.
//! Source: diyAudio thread and community driver implementations.
//!
//! ## Power-on volume state
//! The chip powers up with REG_VOLUME_LEFT (0x04) and REG_VOLUME_RIGHT (0x05)
//! set to 0x00 (0 dB = loudest). Any audio present on the I²S bus at startup
//! will be passed through at full volume. The driver MUST write VOLUME_MUTE
//! (0xFF) to both volume registers BEFORE performing a soft reset or any other
//! configuration, to prevent a loud pop.
//!
//! ## REG_VOLUME_CTRL (0x09)
//! This register must be written to put the chip into individual-channel volume
//! control mode. Writing 0x00 enables direct control via REG_VOLUME_LEFT /
//! REG_VOLUME_RIGHT. Without this write the volume registers may not take
//! effect. Source: Linux driver reference implementations (royno/Rpi-ES90x8-DAC).
//!
//! ## REG_INPUT_CONFIG bits \[3:2\] = 0b00
//! The `input_select` field occupies bits \[3:2\] of REG_INPUT_CONFIG (0x01).
//! For I²S (as opposed to SPDIF) the field must be 0b00. The constant
//! `INPUT_I2S_32BIT = 0b0001_0000` has bits\[3:2\] = 0b00, so this is satisfied.
//! Bit 4 (0x10) selects 32-bit word length within the I²S format.

// ---------------------------------------------------------------------------
// Register addresses
// ---------------------------------------------------------------------------

/// System register — bit 0 = soft reset (self-clearing after reset)
pub const REG_SYSTEM: u8 = 0x00;

/// Input configuration — I²S format, bit depth, justification
///
/// bits\[3:2\] = input_select: 0b00 = I²S (default), 0b01/0b10/0b11 = SPDIF sources.
/// Must be 0b00 for I²S operation.
pub const REG_INPUT_CONFIG: u8 = 0x01;

/// Automute configuration
pub const REG_AUTOMUTE: u8 = 0x02;

/// Automute time constant
pub const REG_AUTOMUTE_TIME: u8 = 0x03;

/// Volume attenuation — left channel
///
/// 0x00 = 0 dB (loudest / no attenuation).
/// 0xFF = maximum attenuation (quietest / mute).
/// Power-on default is 0x00 — the chip starts at maximum output level.
pub const REG_VOLUME_LEFT: u8 = 0x04;

/// Volume attenuation — right channel (same encoding as REG_VOLUME_LEFT)
pub const REG_VOLUME_RIGHT: u8 = 0x05;

/// Master mode / sync configuration
pub const REG_MASTER_MODE: u8 = 0x07;

/// Channel mapping
pub const REG_CHANNEL_MAP: u8 = 0x08;

/// Volume control register
///
/// Must be written to select the volume control mode.
/// Writing 0x00 (VOLUME_CTRL_INDIVIDUAL_CHANNELS) enables direct per-channel
/// attenuation via REG_VOLUME_LEFT / REG_VOLUME_RIGHT.
/// Without this write the volume registers are not guaranteed to be active.
pub const REG_VOLUME_CTRL: u8 = 0x09;

/// GPIO / IRQ configuration
pub const REG_GPIO: u8 = 0x0A;

/// Oversampling filter shape (bits 2:0 select filter 1–7)
pub const REG_OSF_FILTER: u8 = 0x0B;

/// DSD configuration — DoP / native DSD enable
pub const REG_DSD_CONFIG: u8 = 0x0C;

/// Soft-start configuration
pub const REG_SOFT_START: u8 = 0x0D;

/// Volume rate / fade time
pub const REG_VOLUME_RATE: u8 = 0x0E;

/// General settings
pub const REG_GENERAL: u8 = 0x0F;

/// THD compensation coefficient 2
pub const REG_THD_C2: u8 = 0x10;

/// THD compensation coefficient 3
pub const REG_THD_C3: u8 = 0x11;

// ---------------------------------------------------------------------------
// Register field values
// ---------------------------------------------------------------------------

/// System register: initiate soft reset (self-clearing)
pub const SYSTEM_SOFT_RESET: u8 = 0x01;

/// Master mode: I²S slave (STM32 SAI drives MCLK/BCLK/LRCLK)
pub const MASTER_MODE_SLAVE: u8 = 0x00;

/// Input config: I²S format, 32-bit, normal polarity
///
/// Bit 4 = 1 selects 32-bit I²S word length.
/// Bits \[3:2\] = 0b00 keeps input_select = I²S (required).
/// Bits \[1:0\] = 0b00 selects normal (non-inverted) polarity.
pub const INPUT_I2S_32BIT: u8 = 0b0001_0000;

/// DSD config: DoP (DSD over PCM) enable
pub const DSD_DOP_ENABLE: u8 = 0b0000_0001;

/// DSD config: native DSD bitstream enable
pub const DSD_NATIVE_ENABLE: u8 = 0b0000_0010;

/// Volume: mute (maximum attenuation)
pub const VOLUME_MUTE: u8 = 0xFF;

/// Volume: 0 dB (no attenuation — loudest)
pub const VOLUME_0DB: u8 = 0x00;

/// Volume control: use individual per-channel registers (REG_VOLUME_LEFT / REG_VOLUME_RIGHT).
///
/// Write to REG_VOLUME_CTRL (0x09) to activate direct channel attenuation.
/// This is the correct mode for per-channel software volume control.
pub const VOLUME_CTRL_INDIVIDUAL_CHANNELS: u8 = 0x00;
