//! Tests for AudioPowerSequencer — enforces safe power-on/off ordering.
//!
//! TPA6120A2 datasheet and TI E2E forum confirm:
//! amplifier SHUTDOWN must not be raised while DAC is outputting signal.
//! This causes an audible "thump" and over time can damage headphones.
//!
//! The AudioPowerSequencer typestate enforces at compile time:
//!   1. Mute DAC (Es9038q2m)
//!   2. Enable amp (Tpa6120a2)  — ONLY callable after step 1
//!   3. Unmute DAC              — ONLY callable after step 2

use platform::audio_sequencer::{AmpEnabled, AudioPowerSequencer, DacMuted, FullyOn};

/// The sequencer starts in the DacOutputting state (DAC running, amp disabled).
#[test]
fn sequencer_starts_in_dac_outputting_state() {
    // AudioPowerSequencer::new() returns the initial state
    // Type system: new() → AudioPowerSequencer<DacOutputting>
    let _seq: AudioPowerSequencer<platform::audio_sequencer::DacOutputting> =
        AudioPowerSequencer::new();
}

/// After muting the DAC, the sequencer transitions to DacMuted state.
#[test]
fn mute_dac_transitions_to_dac_muted() {
    use platform::audio_sequencer::DacOutputting;
    let seq: AudioPowerSequencer<DacOutputting> = AudioPowerSequencer::new();
    // mute_dac() consumes DacOutputting state and returns DacMuted state
    let _muted: AudioPowerSequencer<DacMuted> = seq.mute_dac();
}

/// Enabling the amp is ONLY possible from the DacMuted state.
/// This test verifies the DacMuted state has an enable_amp() method.
#[test]
fn enable_amp_only_from_dac_muted_state() {
    use platform::audio_sequencer::DacOutputting;
    let seq: AudioPowerSequencer<DacOutputting> = AudioPowerSequencer::new();
    let muted = seq.mute_dac();
    // enable_amp() consumes DacMuted → AmpEnabled
    let _amp_on: AudioPowerSequencer<AmpEnabled> = muted.enable_amp();
}

/// Unmuting the DAC is ONLY possible after the amp is enabled.
#[test]
fn unmute_dac_only_from_amp_enabled_state() {
    use platform::audio_sequencer::DacOutputting;
    let seq: AudioPowerSequencer<DacOutputting> = AudioPowerSequencer::new();
    let muted = seq.mute_dac();
    let amp_on = muted.enable_amp();
    // unmute_dac() consumes AmpEnabled → FullyOn
    let _fully_on: AudioPowerSequencer<FullyOn> = amp_on.unmute_dac();
}

/// Power-down sequence: FullyOn → mute DAC → disable amp
#[test]
fn power_down_sequence_is_enforced() {
    use platform::audio_sequencer::DacOutputting;
    let seq: AudioPowerSequencer<DacOutputting> = AudioPowerSequencer::new();
    let fully_on: AudioPowerSequencer<FullyOn> =
        seq.mute_dac().enable_amp().unmute_dac();
    // Power down: mute first, then disable amp
    let muted = fully_on.mute_dac_for_shutdown();
    let _off: AudioPowerSequencer<DacOutputting> = muted.disable_amp();
}

/// The DAC mute state constant (attenuation = 255) is correct.
#[test]
fn dac_mute_uses_max_attenuation() {
    use platform::audio_types::{AttenuationRegister, VolumePercent};
    let mute = AttenuationRegister::from_volume(VolumePercent::new(0));
    assert_eq!(mute.get(), 255, "Muted DAC must use max attenuation (255)");
}
