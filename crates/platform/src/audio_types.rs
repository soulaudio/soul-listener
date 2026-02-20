//! Audio domain newtypes for compile-time safety.
//!
//! These zero-cost abstractions prevent common errors:
//! - `VolumePercent`: clamps 0–100, prevents register overflow
//! - `AttenuationRegister`: ES9038Q2M-specific, derived from VolumePercent only
//! - `SampleRateHz`: validates 8000–768000 Hz range
//! - `I2cAddr<Bus>`: phantom type binds address to correct bus

use core::marker::PhantomData;

// ── Error type ───────────────────────────────────────────────────────────────

/// Error returned when a value is out of the valid range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfRangeError {
    /// The value that was out of range.
    pub value: u32,
    /// The inclusive minimum allowed value.
    pub min: u32,
    /// The inclusive maximum allowed value.
    pub max: u32,
}

// ── VolumePercent ────────────────────────────────────────────────────────────

/// Volume as a percentage, clamped to 0–100.
///
/// Wraps a `u8` with the invariant `0 <= value <= 100`.
/// Construct with [`VolumePercent::new`] (clamping) or
/// [`VolumePercent::try_new`] (fallible, strict).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VolumePercent(u8);

impl VolumePercent {
    /// Create a `VolumePercent`, clamping values above 100 to 100.
    #[must_use]
    pub fn new(value: u8) -> Self {
        Self(value.min(100))
    }

    /// Create a `VolumePercent`, returning an error if `value > 100`.
    ///
    /// # Errors
    ///
    /// Returns [`OutOfRangeError`] if `value > 100`.
    pub fn try_new(value: u8) -> Result<Self, OutOfRangeError> {
        if value > 100 {
            Err(OutOfRangeError {
                value: u32::from(value),
                min: 0,
                max: 100,
            })
        } else {
            Ok(Self(value))
        }
    }

    /// Return the inner volume value (0–100).
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

// ── AttenuationRegister ──────────────────────────────────────────────────────

/// ES9038Q2M master attenuation register value (0x00 = full volume, 0xFF = mute).
///
/// The ES9038Q2M uses a linear attenuation scale:
/// - Register 0x00 → 0 dB (no attenuation, full volume)
/// - Register 0xFF → maximum attenuation (effectively muted)
///
/// This type can only be constructed from a [`VolumePercent`], ensuring
/// the conversion formula is applied consistently.
///
/// Formula: `attenuation = (100 - volume_percent) * 255 / 100`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct AttenuationRegister(u8);

impl AttenuationRegister {
    /// Convert a `VolumePercent` to an ES9038Q2M attenuation register value.
    ///
    /// - 100% volume → register 0x00 (0 dB, no attenuation)
    /// - 0% volume   → register 0xFF (maximum attenuation / mute)
    #[must_use]
    pub fn from_volume(vol: VolumePercent) -> Self {
        // (100 - vol) * 255 / 100
        // All values are u8/u16, no overflow possible (max: 100 * 255 = 25500 < u16::MAX)
        let attenuation = (u16::from(100 - vol.get()) * 255) / 100;
        Self(attenuation as u8)
    }

    /// Return the raw register value.
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

// ── SampleRateHz ─────────────────────────────────────────────────────────────

/// Sample rate in Hz, validated to the range supported by the ES9038Q2M.
///
/// Valid range: 8000–768000 Hz (8 kHz to 768 kHz PCM).
/// DSD rates (2.8/5.6/11.2/22.5 MHz) are not represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct SampleRateHz(u32);

impl SampleRateHz {
    /// Minimum supported sample rate: 8000 Hz (telephony).
    pub const MIN_HZ: u32 = 8_000;

    /// Maximum supported sample rate: 768000 Hz (ES9038Q2M PCM max).
    pub const MAX_HZ: u32 = 768_000;

    /// Create a `SampleRateHz`, returning an error if out of 8000–768000 Hz.
    ///
    /// # Errors
    ///
    /// Returns [`OutOfRangeError`] if `hz < 8000` or `hz > 768000`.
    pub fn new(hz: u32) -> Result<Self, OutOfRangeError> {
        if hz < Self::MIN_HZ || hz > Self::MAX_HZ {
            Err(OutOfRangeError {
                value: hz,
                min: Self::MIN_HZ,
                max: Self::MAX_HZ,
            })
        } else {
            Ok(Self(hz))
        }
    }

    /// Return the sample rate in Hz.
    #[must_use]
    pub fn get(self) -> u32 {
        self.0
    }
}

// ── I2C bus phantom types ────────────────────────────────────────────────────

/// Phantom type for I2C bus 2 (BQ25895 PMIC: address 0x6A).
#[derive(Debug, Clone, Copy)]
pub struct I2cBus2;

/// Phantom type for I2C bus 3 (ES9038Q2M DAC: address 0x48).
#[derive(Debug, Clone, Copy)]
pub struct I2cBus3;

// ── I2cAddr ──────────────────────────────────────────────────────────────────

/// I2C 7-bit address bound to a specific bus via phantom type.
///
/// The phantom type `Bus` ensures addresses are not accidentally
/// used on the wrong bus at compile time.
///
/// ## Reserved I2C addresses (I2C specification):
/// - 0x00–0x07: reserved (general call, CBUS, etc.)
/// - 0x78–0x7F: reserved (10-bit address prefix, device ID, etc.)
///
/// ## Usage:
/// ```rust
/// use platform::audio_types::{I2cAddr, I2cBus2, I2cBus3};
///
/// // BQ25895 PMIC on I2C2
/// let pmic_addr: I2cAddr<I2cBus2> = I2cAddr::new(0x6A);
///
/// // ES9038Q2M DAC on I2C3
/// let dac_addr: I2cAddr<I2cBus3> = I2cAddr::new(0x48);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I2cAddr<Bus> {
    addr: u8,
    _bus: PhantomData<Bus>,
}

impl<Bus> I2cAddr<Bus> {
    /// Create an I2C address without checking reserved ranges.
    ///
    /// Prefer [`try_new`][Self::try_new] in generic code. Use this only when
    /// the address is a known hardware-fixed constant.
    #[must_use]
    pub fn new(addr: u8) -> Self {
        Self {
            addr,
            _bus: PhantomData,
        }
    }

    /// Create an I2C address, rejecting I2C-reserved ranges.
    ///
    /// Reserved: 0x00–0x07 (general call etc.) and 0x78–0x7F (10-bit prefix).
    ///
    /// # Errors
    ///
    /// Returns [`OutOfRangeError`] if `addr <= 0x07` or `addr >= 0x78`.
    pub fn try_new(addr: u8) -> Result<Self, OutOfRangeError> {
        if addr <= 0x07 || addr >= 0x78 {
            Err(OutOfRangeError {
                value: u32::from(addr),
                min: 0x08,
                max: 0x77,
            })
        } else {
            Ok(Self {
                addr,
                _bus: PhantomData,
            })
        }
    }

    /// Return the 7-bit I2C address.
    #[must_use]
    pub fn get(self) -> u8 {
        self.addr
    }
}
