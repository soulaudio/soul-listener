//! Bridge gates: the firmware's own constants must agree with `dap-hw-spec`.
//!
//! # Why this file is temporary, and what replaces it
//!
//! The end state is that the firmware **consumes** the spec — `memory_map`,
//! `power::model`'s peripheral currents and `memory.x` all derived from one
//! place, so there is nothing to compare. Until that refactor lands, the
//! duplicates exist and this file makes them *checked* duplicates rather than
//! independent ones.
//!
//! Every assertion here should eventually be deletable because its subject
//! became a `use`. Treat a growing file as a signal the refactor is overdue.
//!
//! # This caught a real error on its first run
//!
//! The spec's first draft recorded the CS43131's playback current as 12.8 mA —
//! the correct figure for its 1.8 V core, and the wrong one for a model that
//! works battery-referred (`DAC_AMP_PLAYBACK_MA`'s doc: "playback current at the
//! battery rail"). Same ~23 mW of power, a factor of ~1.8 apart, and nothing but
//! a cross-check would have noticed. The field is now
//! `playback_ma_at_battery`, because a bare "mA" is not a unit for a part with
//! internal rails.

use dap_core::power::model;
use dap_hw_spec::{DacRole, VolumeMechanism, ACTIVE};

/// The spec's battery and runtime target must be the model's.
#[test]
fn the_battery_envelope_matches_the_power_model() {
    assert_eq!(
        ACTIVE.power.battery_mah as f32,
        model::DEFAULT_BATTERY_MAH,
        "spec battery {} mAh vs model {} mAh — the runtime estimate and the spec's budget would \
         describe different products",
        ACTIVE.power.battery_mah,
        model::DEFAULT_BATTERY_MAH
    );
}

/// The spec's peripheral draw must be the model's, component by component.
///
/// Checked as a total AND against the DAC term, because a total can be right
/// while the split is wrong — which is precisely how a rail-referred current
/// hid inside a plausible-looking sum.
#[test]
fn the_peripheral_currents_match_the_power_model() {
    let spec_dac = ACTIVE.dac.playback_ma_at_battery;
    assert!(
        (spec_dac - model::DAC_AMP_PLAYBACK_MA).abs() < 0.01,
        "spec DAC+amp {spec_dac} mA vs model {} mA. Both must be referred to the BATTERY rail: \
         the CS43131's ~23 mW is 12.8 mA at its 1.8 V core but ~7 mA from a 3.7 V battery, and \
         mixing the two silently inflates the audio section by ~1.8x.",
        model::DAC_AMP_PLAYBACK_MA
    );

    let spec_total = ACTIVE.power.peripheral_playback_ma;
    assert!(
        (spec_total - model::PERIPHERAL_PLAYBACK_MA).abs() < 0.01,
        "spec peripherals {spec_total} mA vs model {} mA (= DAC {} + SD {} + display {})",
        model::PERIPHERAL_PLAYBACK_MA,
        model::DAC_AMP_PLAYBACK_MA,
        model::SD_AVG_MA,
        model::DISPLAY_AVG_MA
    );
}

/// The MCU's top clock must be the one the clock planner plans against.
#[test]
fn the_mcu_max_clock_matches_the_power_model() {
    assert_eq!(
        ACTIVE.mcu.max_core_mhz,
        model::H7A3_MAX_CLOCK_MHZ,
        "spec says {} MHz, the model plans against {} MHz",
        ACTIVE.mcu.max_core_mhz,
        model::H7A3_MAX_CLOCK_MHZ
    );
}

/// `memory.x` must declare exactly the spec's linked regions.
///
/// This supersedes half of `linker_script_regions_match_the_rust_memory_map`:
/// that gate made the linker and the Rust constants agree with each other, which
/// still allowed both to be wrong together. Now the spec is the apex and the
/// linker script is checked against it.
#[test]
fn the_linker_script_declares_the_specs_linked_regions() {
    const MEMORY_X: &str = include_str!("../../board-h7a3/memory.x");

    for region in ACTIVE.memory.iter().filter(|r| r.linked) {
        // FLASH is not in the spec's memory list (it is not RAM), and DTCM/SRD
        // are present-but-unlinked, so only linked regions are required here.
        let decl = MEMORY_X
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with(region.name) && l.contains("ORIGIN"));
        let decl = decl.unwrap_or_else(|| {
            panic!(
                "memory.x does not declare region '{}', which the spec marks linked ({} bytes at \
                 {:#x}): {}",
                region.name, region.bytes, region.origin, region.note
            )
        });
        let origin_hex = format!("{:#x}", region.origin);
        assert!(
            decl.contains(&origin_hex),
            "memory.x declares '{}' at a different origin than the spec's {}: {decl}",
            region.name,
            origin_hex
        );
        // Sizes are written in K in the script; compare in K to avoid parsing.
        let kib = region.bytes / 1024;
        assert!(
            decl.contains(&format!("{kib}K")),
            "memory.x declares '{}' at a different size than the spec's {kib}K: {decl}",
            region.name
        );
    }
}

/// The board crate must build for the chip the spec names.
///
/// A firmware compiled for a different MCU than the spec describes would be
/// wrong in a way no amount of host testing could reveal.
#[test]
fn the_board_crate_targets_the_specs_mcu() {
    const BOARD_TOML: &str = include_str!("../../board-h7a3/Cargo.toml");
    assert!(
        BOARD_TOML.contains(ACTIVE.mcu.embassy_feature),
        "board-h7a3 does not enable the embassy chip feature '{}' that the spec requires for {}",
        ACTIVE.mcu.embassy_feature,
        ACTIVE.mcu.mpn
    );
}

/// The audio-path properties the bit-perfect gates depend on.
///
/// `soul-audio-core`'s signal-integrity tests assert that a bypassed stage is
/// bit-exact. That only matters if the *device* keeps volume out of the digital
/// path and can play at the source rate — both facts about the hardware, so they
/// belong to the spec and are asserted here rather than assumed.
#[test]
fn the_audio_path_can_actually_be_bit_perfect() {
    assert_eq!(
        ACTIVE.dac.role,
        DacRole::I2sSlave,
        "native-rate playback needs the DAC as I2S slave; as master the resampler becomes \
         structural, which measured 19.1M instructions per second of audio — 22x the FLAC decode"
    );
    assert!(
        !matches!(ACTIVE.dac.volume, VolumeMechanism::HostDigital),
        "the spec declares host-side digital volume, which discards ~5 bits at -30 dB before the \
         DAC sees the samples"
    );
    assert!(
        ACTIVE.dac.integrated_amp,
        "the spec's DAC has no integrated amp, so the peripheral current budget must include a \
         separate amp part — check `power::model`'s audio-section figure before trusting it"
    );
}
