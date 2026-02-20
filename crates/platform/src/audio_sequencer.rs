//! Audio power sequencing typestate machine.
//!
//! Enforces the safe power-on/off ordering for TPA6120A2 + ES9038Q2M:
//!
//! ## Power-on sequence (prevents audible pop):
//! ```text
//! [DacOutputting] --mute_dac()--> [DacMuted] --enable_amp()--> [AmpEnabled] --unmute_dac()--> [FullyOn]
//! ```
//!
//! ## Power-off sequence:
//! ```text
//! [FullyOn] --mute_dac_for_shutdown()--> [DacMuted] --disable_amp()--> [DacOutputting]
//! ```
//!
//! ## Why this matters:
//! The TPA6120A2 datasheet (SLOS398E §8.3.2) and TI E2E forum thread #708638 confirm:
//! raising SHUTDOWN while the amplifier input carries signal causes a "thump"
//! transient at the headphone output. Over repeated power cycles this degrades
//! driver cone alignment and can damage sensitive headphones.
//!
//! The ES9038Q2M register map: register 15 (ATT_L) = 0xFF → muted, 0x00 → 0 dB.

use core::marker::PhantomData;

// ── State types (zero-sized) ──────────────────────────────────────────────────

/// DAC is outputting audio; amplifier is disabled (SHUTDOWN low).
pub struct DacOutputting;

/// DAC attenuation set to maximum (muted); amplifier still disabled.
/// Safe to enable amplifier from this state.
pub struct DacMuted;

/// Amplifier enabled (SHUTDOWN high); DAC is still muted.
/// Safe to unmute DAC from this state.
pub struct AmpEnabled;

/// Fully operational: DAC outputting audio, amplifier enabled.
/// Must mute DAC before disabling amplifier.
pub struct FullyOn;

// ── Sequencer ────────────────────────────────────────────────────────────────

/// Typestate machine for audio power sequencing.
///
/// The phantom type `State` encodes the current power state.
/// Only valid transitions are provided as methods.
/// Attempting to `enable_amp()` before `mute_dac()` is a compile error.
pub struct AudioPowerSequencer<State> {
    _state: PhantomData<State>,
}

impl AudioPowerSequencer<DacOutputting> {
    /// Create sequencer in initial state (DAC running, amp off).
    #[must_use]
    pub fn new() -> Self {
        Self {
            _state: PhantomData,
        }
    }

    /// Mute the DAC (set ATT registers to 0xFF) before enabling the amplifier.
    ///
    /// In hardware this writes 0xFF to ES9038Q2M registers 15 (ATT_L) and 16 (ATT_R).
    /// The caller must perform the actual I2C write before the state transition is meaningful.
    #[must_use]
    pub fn mute_dac(self) -> AudioPowerSequencer<DacMuted> {
        AudioPowerSequencer {
            _state: PhantomData,
        }
    }
}

impl AudioPowerSequencer<DacMuted> {
    /// Enable the headphone amplifier (raise TPA6120A2 SHUTDOWN pin).
    ///
    /// Safe only after `mute_dac()` — the amp input is silent.
    /// In hardware: GPIO SHUTDOWN → Level::High.
    #[must_use]
    pub fn enable_amp(self) -> AudioPowerSequencer<AmpEnabled> {
        AudioPowerSequencer {
            _state: PhantomData,
        }
    }

    /// Disable amplifier during power-down (returns to initial state).
    #[must_use]
    pub fn disable_amp(self) -> AudioPowerSequencer<DacOutputting> {
        AudioPowerSequencer {
            _state: PhantomData,
        }
    }
}

impl AudioPowerSequencer<AmpEnabled> {
    /// Unmute the DAC (restore ATT registers to desired volume level).
    ///
    /// In hardware: write VolumePercent → AttenuationRegister to ES9038Q2M registers 15/16.
    #[must_use]
    pub fn unmute_dac(self) -> AudioPowerSequencer<FullyOn> {
        AudioPowerSequencer {
            _state: PhantomData,
        }
    }
}

impl AudioPowerSequencer<FullyOn> {
    /// Mute the DAC as the first step of power-down.
    ///
    /// Must be called before `disable_amp()` to prevent pop noise.
    #[must_use]
    pub fn mute_dac_for_shutdown(self) -> AudioPowerSequencer<DacMuted> {
        AudioPowerSequencer {
            _state: PhantomData,
        }
    }
}

impl Default for AudioPowerSequencer<DacOutputting> {
    fn default() -> Self {
        Self::new()
    }
}
