//! ES9038Q2M register map
//!
//! Source: ESS Technology ES9038Q2M Datasheet v1.4
//! <https://www.esstech.com/wp-content/uploads/2022/09/ES9038Q2M-Datasheet-v1.4.pdf>

// ---------------------------------------------------------------------------
// Register addresses
// ---------------------------------------------------------------------------

/// System register — bit 0 = soft reset (self-clearing after reset)
pub const REG_SYSTEM: u8 = 0x00;

/// Input configuration — I²S format, bit depth, justification
pub const REG_INPUT_CONFIG: u8 = 0x01;

/// Automute configuration
pub const REG_AUTOMUTE: u8 = 0x02;

/// Automute time constant
pub const REG_AUTOMUTE_TIME: u8 = 0x03;

/// Volume attenuation — left channel (0x00 = 0 dB, 0xFF = max attenuation)
pub const REG_VOLUME_LEFT: u8 = 0x04;

/// Volume attenuation — right channel
pub const REG_VOLUME_RIGHT: u8 = 0x05;

/// Master mode / sync configuration
pub const REG_MASTER_MODE: u8 = 0x07;

/// Channel mapping
pub const REG_CHANNEL_MAP: u8 = 0x08;

/// Volume control — enable master volume, direct control mode
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
pub const INPUT_I2S_32BIT: u8 = 0b0001_0000;

/// DSD config: DoP (DSD over PCM) enable
pub const DSD_DOP_ENABLE: u8 = 0b0000_0001;

/// DSD config: native DSD bitstream enable
pub const DSD_NATIVE_ENABLE: u8 = 0b0000_0010;

/// Volume: mute (maximum attenuation)
pub const VOLUME_MUTE: u8 = 0xFF;

/// Volume: 0 dB (no attenuation)
pub const VOLUME_0DB: u8 = 0x00;
