//! Volume control mapping for the ES9038Q2M DAC.
//!
//! The ES9038Q2M uses an 8-bit attenuation register where:
//! - `0x00` = 0 dB attenuation (maximum loudness, no attenuation)
//! - `0xFF` = maximum attenuation (~−127.5 dB, effectively muted)
//!
//! This module provides a mapping from a user-facing linear percentage
//! (0 – 100) to the hardware register value via the [`VolumePercent`] and
//! [`AttenuationRegister`] newtypes, which enforce valid ranges at compile time.

use platform::audio_types::{AttenuationRegister, VolumePercent};

/// Map a [`VolumePercent`] to an ES9038Q2M [`AttenuationRegister`] value.
///
/// # Register encoding
///
/// ```text
/// attenuation = (100 - volume_percent) * 255 / 100
/// ```
///
/// | `volume` | Register | Effect             |
/// |----------|----------|--------------------|
/// | 0%       | 255      | Max atten. (muted)  |
/// | 50%      | ~127     | ~50% loudness       |
/// | 100%     | 0        | 0 dB (full volume)  |
///
/// # Arguments
///
/// * `volume` — A [`VolumePercent`] in the range 0–100 (enforced by the type).
///
/// # Returns
///
/// The [`AttenuationRegister`] value to write to ES9038Q2M register 0x0E
/// (master volume) or 0x0F / 0x10 (per-channel volume).
pub fn volume_to_attenuation(volume: VolumePercent) -> AttenuationRegister {
    AttenuationRegister::from_volume(volume)
}
