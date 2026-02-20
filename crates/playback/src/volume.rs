//! Volume control mapping for the ES9038Q2M DAC.
//!
//! The ES9038Q2M uses an 8-bit attenuation register where:
//! - `0x00` = 0 dB attenuation (maximum loudness, no attenuation)
//! - `0xFF` = maximum attenuation (~−127.5 dB, effectively muted)
//!
//! This module provides a mapping from a user-facing linear percentage
//! (0 – 100) to the hardware register value.  The mapping is deliberately
//! linear (not perceptual/dB-stepped) to keep the implementation simple and
//! deterministic; a perceptual curve can be applied in the UI layer before
//! calling this function.

/// Map a linear volume percentage (0 – 100) to an ES9038Q2M attenuation
/// register value.
///
/// # Register encoding
///
/// ```text
/// attenuation = 255 - clamp(volume, 0, 100) * 255 / 100
/// ```
///
/// | `volume` | Register | Effect        |
/// |----------|----------|---------------|
/// | 0        | 255      | Max atten. (muted) |
/// | 50       | 128      | ~50 % loudness |
/// | 100      | 0        | 0 dB (full volume) |
/// | >100     | 0        | Same as 100 (clamped) |
///
/// # Arguments
///
/// * `volume` — Percentage in the range 0 – 100.  Values above 100 are
///   clamped to 100.
///
/// # Returns
///
/// The 8-bit attenuation register value to write to ES9038Q2M register 0x0E
/// (master volume) or 0x0F / 0x10 (per-channel volume).
pub fn volume_to_attenuation(volume: u8) -> u8 {
    let clamped = volume.min(100) as u16;
    // Compute in u16 to avoid overflow before the cast back to u8.
    let attenuation = 255u16 - clamped * 255 / 100;
    attenuation as u8
}
