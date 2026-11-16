//! `SimBattery` -- a host test double for `dap_core::ports::Power`, driven
//! by a scriptable timeline of `(at_ms, soc_permille, temp_centi_c,
//! charger_present)` keyframes that `read()` interpolates/steps through.
//!
//! Every actuator call (`set_charge_current_ma`, `set_clock_profile`,
//! `request_power_off`) is recorded for assertions -- the same shape as
//! `dap_core::power`'s private `MockPower` test double, but public so
//! `tests/power_headless.rs` can drive the real `PowerService` state
//! machine (and, in one scenario, a real `AudioPlayer` playing through it)
//! headlessly from the host.
//!
//! ## `charging` derivation (Task 1 left this unspecified)
//! [`crate::ports::BatteryReading::charging`](dap_core::ports::BatteryReading)
//! means "the charge IC currently reports actively charging" -- `SimBattery`
//! has no charge IC to ask, so it derives it the only way a
//! coulomb-counter-backed board plausibly could without one: **`charging =
//! charger_present && soc rose since the previous `read()``**. The very
//! first `read()` has no previous sample to compare against, so `charging`
//! is `false` on it even if the timeline starts rising with a charger
//! already present -- a documented edge case, not expected to matter in
//! practice (a real charger plug event is itself a discrete occurrence
//! `SimBattery`'s caller controls via keyframes, so there's always at least
//! one prior `read()` once charging has actually started).
//!
//! ## `voltage_mv` / `current_ma` model
//! Deliberately simple, per the brief ("a simple model is fine"): both are
//! pure functions of `(charging, charger_present, soc_permille)`, not of the
//! timeline's instantaneous slope (which would make them
//! keyframe-spacing-dependent and brittle for tests to reason about).
//! - `voltage_mv`: linear in `soc_permille` from 3300 mV (empty) to 4200 mV
//!   (full) -- the typical single-cell Li-ion terminal-voltage range,
//!   ignoring IR-drop/relaxation detail.
//! - `current_ma`: a constant nominal discharge current while discharging;
//!   a constant nominal charge current while actively charging below 80%
//!   SoC, tapering linearly down to a trickle at 100% (a simplified CC/CV
//!   charge profile); ~0 while a charger is present but not actively
//!   charging (full/maintenance).

use dap_core::ports::{BatteryReading, Power};
use dap_core::power::PowerProfile;

/// Empty-pack terminal voltage, millivolts -- see module docs' voltage model.
const MIN_VOLTAGE_MV: u32 = 3300;
/// Full-pack terminal voltage, millivolts.
const MAX_VOLTAGE_MV: u32 = 4200;
/// Constant discharge current magnitude (mA, negative per
/// [`BatteryReading`]'s sign convention) while not charging.
const NOMINAL_DISCHARGE_MA: i16 = -200;
/// Constant charge current (mA) while actively charging below
/// [`TAPER_START_SOC_PERMILLE`].
const NOMINAL_CHARGE_MA: i16 = 500;
/// Trickle charge current (mA) at 100% SoC (top of the CC/CV taper).
const TRICKLE_CHARGE_MA: i16 = 50;
/// SoC (per-mille) above which charge current tapers from
/// [`NOMINAL_CHARGE_MA`] down to [`TRICKLE_CHARGE_MA`].
const TAPER_START_SOC_PERMILLE: u16 = 800;

/// One scripted sample point on a [`SimBattery`]'s timeline -- see
/// [`SimBattery::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryKeyframe {
    /// Simulated clock time, milliseconds.
    pub at_ms: u64,
    /// State of charge, per-mille (0..=1000).
    pub soc_permille: u16,
    /// Pack temperature, centi-°C.
    pub temp_centi_c: i16,
    /// Whether a charger is physically connected at this point.
    pub charger_present: bool,
}

impl BatteryKeyframe {
    /// Build a keyframe from its four fields (same order as the struct).
    #[must_use]
    pub fn new(at_ms: u64, soc_permille: u16, temp_centi_c: i16, charger_present: bool) -> Self {
        Self {
            at_ms,
            soc_permille,
            temp_centi_c,
            charger_present,
        }
    }
}

/// A scriptable [`Power`] test double: a timeline of [`BatteryKeyframe`]s
/// that [`read`](Power::read) samples at the battery's current simulated
/// clock (advanced via [`set_now_ms`](Self::set_now_ms)/
/// [`advance_ms`](Self::advance_ms)), plus every actuator call recorded for
/// assertions -- see the module docs.
#[derive(Debug, Clone)]
pub struct SimBattery {
    /// Sorted ascending by `at_ms`.
    keyframes: Vec<BatteryKeyframe>,
    now_ms: u64,
    /// The previous `read()`'s `soc_permille`, used to derive `charging`
    /// (see module docs). `None` before the first `read()`.
    last_soc_permille: Option<u16>,
    charge_current_calls: Vec<u16>,
    profile_calls: Vec<PowerProfile>,
    power_off_calls: u32,
}

impl SimBattery {
    /// Build a battery over `keyframes` (need not be pre-sorted -- sorted by
    /// `at_ms` on construction). Starts at simulated clock `0`.
    ///
    /// # Panics
    /// If `keyframes` is empty -- a [`SimBattery`] with no timeline has no
    /// defined reading.
    pub fn new(keyframes: impl IntoIterator<Item = BatteryKeyframe>) -> Self {
        let mut keyframes: Vec<BatteryKeyframe> = keyframes.into_iter().collect();
        assert!(
            !keyframes.is_empty(),
            "SimBattery needs at least one keyframe"
        );
        keyframes.sort_by_key(|k| k.at_ms);
        Self {
            keyframes,
            now_ms: 0,
            last_soc_permille: None,
            charge_current_calls: Vec::new(),
            profile_calls: Vec::new(),
            power_off_calls: 0,
        }
    }

    /// Set the battery's simulated clock. Takes effect on the next
    /// [`read`](Power::read).
    pub fn set_now_ms(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    /// Advance the battery's simulated clock by `delta_ms`.
    pub fn advance_ms(&mut self, delta_ms: u64) {
        self.now_ms += delta_ms;
    }

    /// Every `ma` value passed to [`Power::set_charge_current_ma`], in call
    /// order.
    #[must_use]
    pub fn charge_current_calls(&self) -> &[u16] {
        &self.charge_current_calls
    }

    /// Every profile passed to [`Power::set_clock_profile`], in call order.
    #[must_use]
    pub fn profile_calls(&self) -> &[PowerProfile] {
        &self.profile_calls
    }

    /// Number of [`Power::request_power_off`] calls observed.
    #[must_use]
    pub fn power_off_calls(&self) -> u32 {
        self.power_off_calls
    }

    /// Sample `(soc_permille, temp_centi_c, charger_present)` at simulated
    /// clock `at_ms`: `soc_permille`/`temp_centi_c` interpolate linearly
    /// between the two keyframes bracketing `at_ms` (clamped to the first/
    /// last keyframe's values outside the timeline's span);
    /// `charger_present` steps -- it holds the earlier keyframe's value
    /// until the later keyframe's own timestamp is reached, rather than
    /// interpolating a boolean.
    fn sample_at(&self, at_ms: u64) -> (u16, i16, bool) {
        let first = self.keyframes.first().expect("non-empty by construction");
        if at_ms <= first.at_ms {
            return (
                first.soc_permille,
                first.temp_centi_c,
                first.charger_present,
            );
        }
        let last = self.keyframes.last().expect("non-empty by construction");
        if at_ms >= last.at_ms {
            return (last.soc_permille, last.temp_centi_c, last.charger_present);
        }

        // Guarded by the early returns above: `first.at_ms < at_ms <
        // last.at_ms`, so `partition_point` lands strictly inside the
        // timeline and both `idx - 1` and `idx` are valid indices.
        let idx = self.keyframes.partition_point(|k| k.at_ms <= at_ms);
        let a = &self.keyframes[idx - 1];
        let b = &self.keyframes[idx];

        let span = (b.at_ms - a.at_ms) as f64;
        let t = if span == 0.0 {
            0.0
        } else {
            (at_ms - a.at_ms) as f64 / span
        };

        let soc = (f64::from(a.soc_permille)
            + t * (f64::from(b.soc_permille) - f64::from(a.soc_permille)))
        .round() as u16;
        let temp = (f64::from(a.temp_centi_c)
            + t * (f64::from(b.temp_centi_c) - f64::from(a.temp_centi_c)))
        .round() as i16;

        (soc, temp, a.charger_present)
    }
}

/// Linear voltage model -- see module docs.
fn voltage_mv_from_soc(soc_permille: u16) -> u16 {
    (MIN_VOLTAGE_MV + u32::from(soc_permille) * (MAX_VOLTAGE_MV - MIN_VOLTAGE_MV) / 1000) as u16
}

/// Charging/discharging current model -- see module docs.
fn current_ma_from_state(charging: bool, charger_present: bool, soc_permille: u16) -> i16 {
    if charging {
        if soc_permille < TAPER_START_SOC_PERMILLE {
            NOMINAL_CHARGE_MA
        } else {
            let span = 1000 - TAPER_START_SOC_PERMILLE;
            let pos = (soc_permille - TAPER_START_SOC_PERMILLE).min(span);
            let drop =
                i32::from(pos) * i32::from(NOMINAL_CHARGE_MA - TRICKLE_CHARGE_MA) / i32::from(span);
            (i32::from(NOMINAL_CHARGE_MA) - drop) as i16
        }
    } else if charger_present {
        0
    } else {
        NOMINAL_DISCHARGE_MA
    }
}

impl Power for SimBattery {
    fn read(&mut self) -> BatteryReading {
        let (soc_permille, temp_centi_c, charger_present) = self.sample_at(self.now_ms);
        let charging = charger_present
            && self
                .last_soc_permille
                .is_some_and(|prev| soc_permille > prev);
        self.last_soc_permille = Some(soc_permille);

        BatteryReading {
            soc_permille,
            voltage_mv: voltage_mv_from_soc(soc_permille),
            current_ma: current_ma_from_state(charging, charger_present, soc_permille),
            temp_centi_c,
            charger_present,
            charging,
        }
    }

    fn set_charge_current_ma(&mut self, ma: u16) {
        self.charge_current_calls.push(ma);
    }

    fn set_clock_profile(&mut self, profile: PowerProfile) {
        self.profile_calls.push(profile);
    }

    fn request_power_off(&mut self) {
        self.power_off_calls += 1;
    }
}
