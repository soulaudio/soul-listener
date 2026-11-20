//! DVFS clock policy — maps [`PowerProfile`] onto a target CPU clock using
//! [`model`](super::model) (design doc §4.2).
//!
//! [`crate::power::PowerService`] decides *which* [`PowerProfile`] is
//! active from battery state; it has no notion of clock speed or decode
//! cost. This module is the separate policy layer a board driver combines
//! with that profile and the current track's measured/estimated real-time
//! factor (RTF) to pick a clock:
//!
//! | Profile | Policy |
//! |---|---|
//! | [`PowerProfile::Performance`] | Always [`model::H7A3_MAX_CLOCK_MHZ`] — boost clock, least sleep, no codec-dependent throttling. |
//! | [`PowerProfile::Balanced`] | [`model::min_clock_mhz_for_rtf`] with [`BALANCED_SAFETY`] headroom. |
//! | [`PowerProfile::BatterySaver`] | [`model::min_clock_mhz_for_rtf`] with the tighter [`BATTERY_SAVER_SAFETY`] headroom. |
//!
//! This is **model-only**: [`clock_mhz_for_profile`] picks a target clock
//! in MHz; no VOS/PLL register gets written here — the board applies that
//! once `board-h7a3` exists (design doc §4.2's framing: "here it's the
//! policy").

use super::model::{self, H7A3_MAX_CLOCK_MHZ, IDLE_CLOCK_MHZ};
use super::PowerProfile;

/// Safety margin [`clock_mhz_for_profile`] applies for
/// [`PowerProfile::Balanced`]: 30% RTF headroom above the bare 1x real-time
/// point above the clock [`model::min_clock_mhz_for_rtf`] derives — enough
/// to absorb a harder-than-average track/decoder-table-init spike without
/// under-running the output ring, while still biasing toward a low clock.
pub const BALANCED_SAFETY: f32 = 1.3;

/// Safety margin [`clock_mhz_for_profile`] applies for
/// [`PowerProfile::BatterySaver`]: a tighter 15% RTF headroom — the
/// "lowest clock+VOS that sustains the *current* codec" policy from the
/// design doc §4.2 table. Note (see this module's unit tests): this
/// minimizes *clock* (and so V²f instantaneous power), but — because it
/// runs the CPU close to continuously at that bare-minimum clock — it does
/// NOT minimize *duty cycle*/maximize sleep time. Whether a "race-to-sleep"
/// policy (boost to a high clock, decode a burst fast, WFI for longer) would
/// draw *less* was previously flagged here as an open refinement; it is now
/// **analyzed and answered — it would not**, for this DAP. See
/// [`model::system_avg_current_at_clock`] and its
/// `min_clock_not_beaten_by_race_to_sleep` test: the H7A3's ~linear
/// current-vs-clock curve plus the always-on ~9.5 mA output path mean total
/// current at the minimum sustaining clock is `<=` at max clock for every
/// format, so minimize-clock is already near energy-optimal and a high-clock
/// burst strategy does not beat it. (Model-level; confirm on silicon.)
pub const BATTERY_SAVER_SAFETY: f32 = 1.15;

/// DVFS clock policy for a [`PowerProfile`], given the current codec's RTF
/// at a known reference clock (design doc §4.2) — see the module doc's
/// table. Both non-`Performance` results are clamped to
/// `[IDLE_CLOCK_MHZ, H7A3_MAX_CLOCK_MHZ]` — never below the lowest clock
/// this board has a current figure for ([`model::IDLE_CLOCK_MHZ`]), never
/// above its rated maximum ([`model::H7A3_MAX_CLOCK_MHZ`]).
#[must_use]
pub fn clock_mhz_for_profile(profile: PowerProfile, rtf_at_ref_clock: f32, ref_mhz: u32) -> u32 {
    let raw = match profile {
        PowerProfile::Performance => return H7A3_MAX_CLOCK_MHZ,
        PowerProfile::Balanced => {
            model::min_clock_mhz_for_rtf(rtf_at_ref_clock, ref_mhz, BALANCED_SAFETY)
        }
        PowerProfile::BatterySaver => {
            model::min_clock_mhz_for_rtf(rtf_at_ref_clock, ref_mhz, BATTERY_SAVER_SAFETY)
        }
    };
    raw.clamp(IDLE_CLOCK_MHZ, H7A3_MAX_CLOCK_MHZ)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::model::{decode_cost_cycles_per_sec, duty_cycle, estimate_target_rtf};

    // Same HOST RTF set as `power::model`'s own tests (see that module for
    // the measurement method/caveat) -- duplicated here (not `pub(crate)`
    // re-exported) so this module's tests read standalone.
    const HOST_RTF_WAV: f32 = 5127.3;
    const HOST_RTF_FLAC: f32 = 266.6;
    const HOST_RTF_MP3: f32 = 465.1;
    const HOST_RTF_AAC: f32 = 247.2;
    const HOST_RTF_ALAC: f32 = 265.3;
    const HOST_RTF_OPUS: f32 = 179.2;
    const HOST_RTF_WAVPACK: f32 = 264.6;

    const ALL_DECODE_FORMATS: [(&str, f32); 7] = [
        ("wav", HOST_RTF_WAV),
        ("flac", HOST_RTF_FLAC),
        ("mp3", HOST_RTF_MP3),
        ("aac", HOST_RTF_AAC),
        ("alac", HOST_RTF_ALAC),
        ("opus", HOST_RTF_OPUS),
        ("wavpack", HOST_RTF_WAVPACK),
    ];

    /// `Performance` never throttles: always the board's rated max clock,
    /// regardless of codec.
    #[test]
    fn performance_profile_always_picks_max_clock() {
        for (_, host_rtf) in ALL_DECODE_FORMATS {
            let target_rtf = estimate_target_rtf(host_rtf);
            let clock =
                clock_mhz_for_profile(PowerProfile::Performance, target_rtf, H7A3_MAX_CLOCK_MHZ);
            assert_eq!(clock, H7A3_MAX_CLOCK_MHZ);
        }
    }

    /// `BatterySaver` must never pick a *higher* clock than `Balanced` for
    /// the same format (tighter safety margin => lower or equal clock), and
    /// both must stay within the selectable [`IDLE_CLOCK_MHZ`]..=
    /// [`H7A3_MAX_CLOCK_MHZ`] range.
    #[test]
    fn battery_saver_never_exceeds_balanced_clock() {
        for (label, host_rtf) in ALL_DECODE_FORMATS {
            let target_rtf = estimate_target_rtf(host_rtf);
            let balanced =
                clock_mhz_for_profile(PowerProfile::Balanced, target_rtf, H7A3_MAX_CLOCK_MHZ);
            let saver =
                clock_mhz_for_profile(PowerProfile::BatterySaver, target_rtf, H7A3_MAX_CLOCK_MHZ);
            assert!(
                saver <= balanced,
                "{label}: battery-saver clock {saver} MHz exceeds balanced {balanced} MHz"
            );
            for clock in [balanced, saver] {
                assert!(
                    (IDLE_CLOCK_MHZ..=H7A3_MAX_CLOCK_MHZ).contains(&clock),
                    "{label}: clock {clock} MHz outside the selectable range \
                     [{IDLE_CLOCK_MHZ}, {H7A3_MAX_CLOCK_MHZ}]"
                );
            }
        }
    }

    /// At the bare-minimum sustaining clock (`BatterySaver`'s tight 15%
    /// margin), the CPU is busy MOST of the time -- duty cycle is
    /// approximately `1 / BATTERY_SAVER_SAFETY`, not near zero. This is the
    /// race-to-sleep-vs-low-clock tension flagged in [`BATTERY_SAVER_SAFETY`]'s
    /// doc: minimizing clock (V²f) and maximizing sleep time are different,
    /// sometimes-opposed objectives, and this policy deliberately picks the
    /// former. Asserted explicitly so nobody mistakes "BatterySaver" for
    /// "mostly asleep".
    #[test]
    fn battery_saver_minimizes_clock_not_duty_cycle() {
        let target_rtf = estimate_target_rtf(HOST_RTF_FLAC);
        let clock =
            clock_mhz_for_profile(PowerProfile::BatterySaver, target_rtf, H7A3_MAX_CLOCK_MHZ);
        let cost = decode_cost_cycles_per_sec(target_rtf, H7A3_MAX_CLOCK_MHZ);
        let duty = duty_cycle(cost, f64::from(clock) * 1.0e6);
        let expected = 1.0 / BATTERY_SAVER_SAFETY;
        assert!(
            (duty - expected).abs() < 0.05,
            "flac: duty {duty:.3} far from the expected ~1/safety ({expected:.3}) -- \
             the whole-MHz rounding in min_clock_mhz_for_rtf should only nudge this slightly"
        );
        assert!(
            duty > 0.5,
            "flac: duty {duty:.3} should be well over half -- BatterySaver's bare-minimum \
             clock means the CPU is busy MOST of the time, not mostly asleep (see this \
             test's doc comment)"
        );
    }
}
