//! `PowerService` — a deterministic state machine over [`crate::ports::Power`]
//! readings: smoothed state-of-charge, charge/discharge/full detection, low
//! and critical battery handling, thermal-aware charge throttling, automatic
//! performance/battery-saver profile switching, idle sleep, runtime
//! estimation, and cycle counting.
//!
//! The service never touches hardware directly — every effect flows through
//! the caller-supplied [`crate::ports::Power`] (actuator calls) or the
//! returned [`PowerEvents`] (for the driver to act on, e.g. pausing
//! playback). Time is passed in (`now_ms`); this crate has no clock of its
//! own, so the same logic runs identically on-target and in a headless host
//! test.
//!
//! [`PowerService`] itself only ever decides *which* [`PowerProfile`] is
//! active (via [`PowerService::profile`]/[`Power::set_clock_profile`]) —
//! it has no notion of clock speed or decode cost. [`model`] is a
//! separate, pure battery energy model (datasheet run-mode currents x
//! measured decode cost) a board driver can combine with the active
//! profile to decide how aggressively to throttle the clock.

use heapless::Vec;

use crate::ports::{BatteryReading, Power};

pub mod model;

/// Max events a single [`PowerService::poll`] can return. Eight distinct
/// [`PowerEvent`] kinds exist; this bound is generous enough that every kind
/// could in principle fire in the same poll without truncation, while still
/// being a small, fixed, no-alloc allocation.
pub const MAX_POWER_EVENTS: usize = 8;

/// The bounded, no-alloc event set [`PowerService::poll`] returns.
pub type PowerEvents = Vec<PowerEvent, MAX_POWER_EVENTS>;

/// System clock/performance profile, driven by [`PowerService`] and applied
/// through [`Power::set_clock_profile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile {
    /// Full performance: no battery-driven throttling.
    Performance,
    /// Default balanced profile (reserved for a future manual/UI-driven
    /// mode; [`PowerService`] itself only ever auto-engages between
    /// [`Performance`](Self::Performance) and
    /// [`BatterySaver`](Self::BatterySaver)).
    Balanced,
    /// Reduced-power profile, auto-engaged below
    /// [`PowerConfig::saver_soc_permille`].
    BatterySaver,
}

/// Charge/discharge state, derived each poll from the latest
/// [`BatteryReading`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeState {
    /// No charger present (or present but not delivering current):
    /// discharging.
    Discharging,
    /// Charger present and actively delivering current.
    Charging,
    /// Charger present, charge IC reports not-charging, and SoC has reached
    /// [`PowerConfig::full_soc_permille`].
    Full,
}

/// One state transition [`PowerService::poll`] detected this tick. Each
/// variant that represents a threshold crossing (as opposed to a level
/// query) fires once per crossing — see each variant's docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerEvent {
    /// Smoothed SoC dropped below [`PowerConfig::low_batt_soc_permille`).
    /// Fires once per crossing; re-arms once SoC recovers past the
    /// threshold plus [`PowerConfig::recovery_hysteresis_permille`].
    LowBattery,
    /// Smoothed SoC dropped below [`PowerConfig::critical_soc_permille`].
    /// Fires once per crossing, alongside a [`Power::request_power_off`]
    /// call. Re-arms the same way as [`LowBattery`](Self::LowBattery).
    CriticalShutdown,
    /// Charge state transitioned into [`ChargeState::Charging`].
    ChargeStarted,
    /// Charge state transitioned into [`ChargeState::Full`].
    ChargeComplete,
    /// [`PowerProfile`] auto-switched (saver engaged or released).
    ProfileChanged(PowerProfile),
    /// Pack temperature left the safe band
    /// (`[safe_temp_min_centi_c, safe_temp_max_centi_c]`); charge current was
    /// reduced to `charge_ma` via [`Power::set_charge_current_ma`]. Fires
    /// once per excursion; re-arms once temperature returns to the safe
    /// band.
    ThermalThrottle {
        /// The charge current (mA) now in effect.
        charge_ma: u16,
    },
    /// `user_active` has been false for at least
    /// [`PowerConfig::idle_timeout_ms`]. Fires once per idle episode.
    EnterSleep,
    /// `user_active` became true again after [`EnterSleep`](Self::EnterSleep).
    Wake,
}

/// Thresholds and tuning knobs for [`PowerService`]. All SoC fields are
/// per-mille (0..=1000); all have sensible defaults via [`Default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerConfig {
    /// EMA smoothing weight for the latest raw reading, per-mille (e.g.
    /// `200` = 20% new sample / 80% history each poll). Higher = more
    /// responsive, less smoothing.
    pub ema_alpha_permille: u16,
    /// Smoothed SoC threshold below which [`PowerEvent::LowBattery`] fires.
    pub low_batt_soc_permille: u16,
    /// Smoothed SoC threshold below which [`PowerEvent::CriticalShutdown`]
    /// fires and [`Power::request_power_off`] is called.
    pub critical_soc_permille: u16,
    /// Smoothed SoC threshold below which [`PowerProfile::BatterySaver`] is
    /// auto-engaged.
    pub saver_soc_permille: u16,
    /// Smoothed SoC at/above which, combined with `charging == false` and a
    /// charger present, the pack is considered [`ChargeState::Full`].
    pub full_soc_permille: u16,
    /// Hysteresis (per-mille) added back on top of `low_batt_soc_permille`/
    /// `critical_soc_permille`/`saver_soc_permille` before a one-shot
    /// condition is allowed to re-arm (prevents flapping right at a
    /// threshold).
    pub recovery_hysteresis_permille: u16,
    /// Lower bound of the safe charge temperature band (centi-°C).
    pub safe_temp_min_centi_c: i16,
    /// Upper bound of the safe charge temperature band (centi-°C).
    pub safe_temp_max_centi_c: i16,
    /// Charge current (mA) applied while outside the safe temperature band.
    pub throttled_charge_current_ma: u16,
    /// Charge current (mA) applied/restored while inside the safe
    /// temperature band.
    pub nominal_charge_current_ma: u16,
    /// Idle duration (ms) of `user_active == false` before
    /// [`PowerEvent::EnterSleep`] fires.
    pub idle_timeout_ms: u64,
    /// Cumulative SoC-drop (per-mille) that counts as one full-discharge
    /// equivalent cycle (default `1000` = one full 100%→0% equivalent,
    /// accumulated across any number of partial discharges).
    pub cycle_soc_span_permille: u32,
    /// Assumed design capacity (mAh), used only for
    /// [`PowerService::time_to_empty_s`]/[`PowerService::time_to_full_s`]
    /// runtime estimation. [`BatteryReading`] carries no capacity field, so
    /// this is a per-board constant supplied via config.
    pub design_capacity_mah: u32,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            ema_alpha_permille: 200,
            low_batt_soc_permille: 150,
            critical_soc_permille: 30,
            saver_soc_permille: 250,
            full_soc_permille: 990,
            recovery_hysteresis_permille: 50,
            safe_temp_min_centi_c: 0,
            safe_temp_max_centi_c: 4500,
            throttled_charge_current_ma: 0,
            nominal_charge_current_ma: 500,
            idle_timeout_ms: 30_000,
            cycle_soc_span_permille: 1000,
            design_capacity_mah: 2000,
        }
    }
}

/// A deterministic state machine over [`Power`] readings — see the module
/// docs. Construct with [`PowerService::new`] (or [`Default`] for
/// `PowerConfig::default()`), then drive it with [`PowerService::poll`]
/// every tick.
#[derive(Debug, Clone)]
pub struct PowerService {
    cfg: PowerConfig,
    initialized: bool,
    smoothed_soc_permille: u16,
    last_current_ma: i16,
    charge_state: ChargeState,
    profile: PowerProfile,
    low_battery_fired: bool,
    critical_fired: bool,
    thermal_throttled: bool,
    asleep: bool,
    last_active_ms: Option<u64>,
    cumulative_discharge_permille: u32,
    cycle_count: u32,
}

impl Default for PowerService {
    fn default() -> Self {
        Self::new(PowerConfig::default())
    }
}

impl PowerService {
    /// Build a service with the given thresholds.
    #[must_use]
    pub fn new(cfg: PowerConfig) -> Self {
        Self {
            cfg,
            initialized: false,
            smoothed_soc_permille: 0,
            last_current_ma: 0,
            charge_state: ChargeState::Discharging,
            profile: PowerProfile::Performance,
            low_battery_fired: false,
            critical_fired: false,
            thermal_throttled: false,
            asleep: false,
            last_active_ms: None,
            cumulative_discharge_permille: 0,
            cycle_count: 0,
        }
    }

    /// Smoothed state of charge, per-mille (0..=1000).
    #[must_use]
    pub fn soc_permille(&self) -> u16 {
        self.smoothed_soc_permille
    }

    /// Current charge/discharge state.
    #[must_use]
    pub fn charge_state(&self) -> ChargeState {
        self.charge_state
    }

    /// Current clock/performance profile.
    #[must_use]
    pub fn profile(&self) -> PowerProfile {
        self.profile
    }

    /// Cumulative full-discharge-equivalent cycle count.
    #[must_use]
    pub fn cycle_count(&self) -> u32 {
        self.cycle_count
    }

    /// Whether the service currently considers the device asleep (idle
    /// timeout elapsed with no activity since).
    #[must_use]
    pub fn is_asleep(&self) -> bool {
        self.asleep
    }

    /// Estimated seconds until empty, from the smoothed SoC and the last
    /// observed discharge current. `None` unless currently discharging with
    /// a non-zero discharge current.
    #[must_use]
    pub fn time_to_empty_s(&self) -> Option<u32> {
        if self.charge_state != ChargeState::Discharging || self.last_current_ma >= 0 {
            return None;
        }
        let discharge_ma = u64::from(self.last_current_ma.unsigned_abs());
        if discharge_ma == 0 {
            return None;
        }
        let remaining_mah =
            u64::from(self.smoothed_soc_permille) * u64::from(self.cfg.design_capacity_mah) / 1000;
        let seconds = remaining_mah * 3600 / discharge_ma;
        Some(u32::try_from(seconds).unwrap_or(u32::MAX))
    }

    /// Estimated seconds until full, from the smoothed SoC and the last
    /// observed charge current. `None` unless currently charging (not yet
    /// full) with a non-zero charge current.
    #[must_use]
    pub fn time_to_full_s(&self) -> Option<u32> {
        if self.charge_state != ChargeState::Charging || self.last_current_ma <= 0 {
            return None;
        }
        let charge_ma = u64::try_from(self.last_current_ma).unwrap_or(0);
        if charge_ma == 0 {
            return None;
        }
        let target = u32::from(self.cfg.full_soc_permille);
        let remaining_permille = target.saturating_sub(u32::from(self.smoothed_soc_permille));
        let remaining_mah =
            u64::from(remaining_permille) * u64::from(self.cfg.design_capacity_mah) / 1000;
        let seconds = remaining_mah * 3600 / charge_ma;
        Some(u32::try_from(seconds).unwrap_or(u32::MAX))
    }

    /// Drive one tick: sample `power`, update the state machine, issue any
    /// actuator calls the new state requires, and return the (possibly
    /// empty) set of events that fired this tick.
    pub fn poll<P: Power>(&mut self, power: &mut P, now_ms: u64, user_active: bool) -> PowerEvents {
        let mut events = PowerEvents::new();
        let reading = power.read();

        let prev_smoothed = if self.initialized {
            self.smoothed_soc_permille
        } else {
            self.initialized = true;
            self.smoothed_soc_permille = reading.soc_permille;
            reading.soc_permille
        };
        self.smoothed_soc_permille = ema_step(
            prev_smoothed,
            reading.soc_permille,
            self.cfg.ema_alpha_permille,
        );
        self.last_current_ma = reading.current_ma;

        self.update_charge_state(&reading, &mut events);
        self.update_cycle_count(prev_smoothed);
        self.update_battery_thresholds(power, &mut events);
        self.update_thermal(power, &reading, &mut events);
        self.update_profile(power, &mut events);
        self.update_sleep(now_ms, user_active, &mut events);

        events
    }

    fn update_charge_state(&mut self, reading: &BatteryReading, events: &mut PowerEvents) {
        let new_state = if !reading.charger_present {
            ChargeState::Discharging
        } else if !reading.charging && self.smoothed_soc_permille >= self.cfg.full_soc_permille {
            ChargeState::Full
        } else {
            ChargeState::Charging
        };
        if new_state != self.charge_state {
            match new_state {
                ChargeState::Charging => {
                    let _ = events.push(PowerEvent::ChargeStarted);
                }
                ChargeState::Full => {
                    let _ = events.push(PowerEvent::ChargeComplete);
                }
                ChargeState::Discharging => {}
            }
        }
        self.charge_state = new_state;
    }

    fn update_cycle_count(&mut self, prev_smoothed: u16) {
        if self.smoothed_soc_permille < prev_smoothed {
            let drop = u32::from(prev_smoothed - self.smoothed_soc_permille);
            self.cumulative_discharge_permille += drop;
            while self.cumulative_discharge_permille >= self.cfg.cycle_soc_span_permille {
                self.cumulative_discharge_permille -= self.cfg.cycle_soc_span_permille;
                self.cycle_count += 1;
            }
        }
    }

    fn update_battery_thresholds<P: Power>(&mut self, power: &mut P, events: &mut PowerEvents) {
        let soc = self.smoothed_soc_permille;
        let hyst = self.cfg.recovery_hysteresis_permille;

        if soc < self.cfg.critical_soc_permille {
            if !self.critical_fired {
                self.critical_fired = true;
                power.request_power_off();
                let _ = events.push(PowerEvent::CriticalShutdown);
            }
        } else if soc >= self.cfg.critical_soc_permille + hyst {
            self.critical_fired = false;
        }

        if soc < self.cfg.low_batt_soc_permille {
            if !self.low_battery_fired {
                self.low_battery_fired = true;
                let _ = events.push(PowerEvent::LowBattery);
            }
        } else if soc >= self.cfg.low_batt_soc_permille + hyst {
            self.low_battery_fired = false;
        }
    }

    fn update_thermal<P: Power>(
        &mut self,
        power: &mut P,
        reading: &BatteryReading,
        events: &mut PowerEvents,
    ) {
        let out_of_band = reading.temp_centi_c < self.cfg.safe_temp_min_centi_c
            || reading.temp_centi_c > self.cfg.safe_temp_max_centi_c;

        if out_of_band {
            if !self.thermal_throttled {
                self.thermal_throttled = true;
                power.set_charge_current_ma(self.cfg.throttled_charge_current_ma);
                let _ = events.push(PowerEvent::ThermalThrottle {
                    charge_ma: self.cfg.throttled_charge_current_ma,
                });
            }
        } else if self.thermal_throttled {
            self.thermal_throttled = false;
            power.set_charge_current_ma(self.cfg.nominal_charge_current_ma);
        }
    }

    fn update_profile<P: Power>(&mut self, power: &mut P, events: &mut PowerEvents) {
        let soc = self.smoothed_soc_permille;
        let hyst = self.cfg.recovery_hysteresis_permille;

        let target = if soc < self.cfg.saver_soc_permille {
            PowerProfile::BatterySaver
        } else if soc >= self.cfg.saver_soc_permille + hyst {
            PowerProfile::Performance
        } else {
            self.profile
        };

        if target != self.profile {
            self.profile = target;
            power.set_clock_profile(target);
            let _ = events.push(PowerEvent::ProfileChanged(target));
        }
    }

    fn update_sleep(&mut self, now_ms: u64, user_active: bool, events: &mut PowerEvents) {
        if self.last_active_ms.is_none() {
            self.last_active_ms = Some(now_ms);
        }

        if user_active {
            self.last_active_ms = Some(now_ms);
            if self.asleep {
                self.asleep = false;
                let _ = events.push(PowerEvent::Wake);
            }
        } else {
            let elapsed = now_ms.saturating_sub(self.last_active_ms.unwrap_or(now_ms));
            if !self.asleep && elapsed >= self.cfg.idle_timeout_ms {
                self.asleep = true;
                let _ = events.push(PowerEvent::EnterSleep);
            }
        }
    }
}

/// One integer EMA step: `prev + alpha_permille * (raw - prev) / 1000`,
/// computed in `i32` to safely carry a negative delta, then clamped back
/// into `u16`'s valid per-mille range.
///
/// Integer division truncates `alpha_permille * delta / 1000` to zero once
/// `delta` is small (e.g. `delta == -4` at the default `alpha == 200`:
/// `-800 / 1000 == 0`), which would otherwise let the smoothed value stall
/// a few per-mille short of the true reading forever. When that happens but
/// a real difference remains, nudge by one unit toward `raw` instead, so
/// the smoothed value still converges exactly — it just takes a few extra
/// polls to close the last stretch.
fn ema_step(prev: u16, raw: u16, alpha_permille: u16) -> u16 {
    let delta = i32::from(raw) - i32::from(prev);
    let mut adj = i32::from(alpha_permille) * delta / 1000;
    if adj == 0 && delta != 0 {
        adj = delta.signum();
    }
    let next = i32::from(prev) + adj;
    next.clamp(0, 1000) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scripted [`Power`] test double: `read()` returns whatever reading
    /// the test last set via [`MockPower::set`], and every actuator call is
    /// recorded for assertions.
    struct MockPower {
        reading: BatteryReading,
        charge_current_calls: Vec<u16, 16>,
        profile_calls: Vec<PowerProfile, 16>,
        power_off_calls: u32,
    }

    impl MockPower {
        fn new(reading: BatteryReading) -> Self {
            Self {
                reading,
                charge_current_calls: Vec::new(),
                profile_calls: Vec::new(),
                power_off_calls: 0,
            }
        }

        fn set(&mut self, reading: BatteryReading) {
            self.reading = reading;
        }
    }

    impl Power for MockPower {
        fn read(&mut self) -> BatteryReading {
            self.reading
        }

        fn set_charge_current_ma(&mut self, ma: u16) {
            let _ = self.charge_current_calls.push(ma);
        }

        fn set_clock_profile(&mut self, profile: PowerProfile) {
            let _ = self.profile_calls.push(profile);
        }

        fn request_power_off(&mut self) {
            self.power_off_calls += 1;
        }
    }

    /// A healthy, mid-charge, room-temperature, unplugged reading — the
    /// baseline tests tweak fields from.
    fn healthy_reading(soc_permille: u16) -> BatteryReading {
        BatteryReading {
            soc_permille,
            voltage_mv: 3800,
            current_ma: -200,
            temp_centi_c: 2500,
            charger_present: false,
            charging: false,
        }
    }

    fn count(events: &[PowerEvents], matches: impl Fn(&PowerEvent) -> bool) -> usize {
        events
            .iter()
            .flat_map(|e| e.iter())
            .filter(|e| matches(e))
            .count()
    }

    #[test]
    fn soc_is_ema_smoothed_not_passthrough() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(800));

        svc.poll(&mut power, 0, true);
        assert_eq!(
            svc.soc_permille(),
            800,
            "first reading initializes directly"
        );

        power.set(healthy_reading(400));
        svc.poll(&mut power, 1000, true);
        // alpha = 200/1000: 800 + 200*(400-800)/1000 = 720, NOT 400.
        assert_eq!(
            svc.soc_permille(),
            720,
            "big jump must be smoothed, not passed through"
        );

        power.set(healthy_reading(400));
        svc.poll(&mut power, 2000, true);
        // 720 + 200*(400-720)/1000 = 656.
        assert_eq!(svc.soc_permille(), 656);
    }

    #[test]
    fn low_battery_fires_once_not_every_poll() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(1000));
        let mut all_events = Vec::<PowerEvents, 64>::new();

        // First poll initializes smoothed SoC at 1000 (no smoothing lag),
        // then every subsequent poll feeds a floor reading of 0 so the EMA
        // monotonically decays through the low-battery threshold and stays
        // below it for many polls.
        for i in 0..40u64 {
            power.set(healthy_reading(0));
            let ev = svc.poll(&mut power, i * 1000, true);
            let _ = all_events.push(ev);
        }

        assert!(svc.soc_permille() < PowerConfig::default().low_batt_soc_permille);
        assert_eq!(
            count(&all_events, |e| *e == PowerEvent::LowBattery),
            1,
            "LowBattery must fire exactly once across a sustained discharge, not every poll"
        );
    }

    #[test]
    fn critical_fires_once_and_requests_power_off() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(1000));
        let mut all_events = Vec::<PowerEvents, 64>::new();

        for i in 0..80u64 {
            power.set(healthy_reading(0));
            let ev = svc.poll(&mut power, i * 1000, true);
            let _ = all_events.push(ev);
        }

        assert!(svc.soc_permille() < PowerConfig::default().critical_soc_permille);
        assert_eq!(
            count(&all_events, |e| *e == PowerEvent::CriticalShutdown),
            1,
            "CriticalShutdown must fire exactly once"
        );
        assert_eq!(
            power.power_off_calls, 1,
            "request_power_off must be called exactly once"
        );
        // LowBattery must also have fired exactly once on the way down.
        assert_eq!(count(&all_events, |e| *e == PowerEvent::LowBattery), 1);
    }

    #[test]
    fn charger_plug_then_full_fires_charge_started_then_complete() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(500));
        svc.poll(&mut power, 0, true);
        assert_eq!(svc.charge_state(), ChargeState::Discharging);

        // Charger plugged in, actively charging.
        power.set(BatteryReading {
            soc_permille: 500,
            voltage_mv: 4000,
            current_ma: 300,
            temp_centi_c: 2500,
            charger_present: true,
            charging: true,
        });
        let ev = svc.poll(&mut power, 1000, true);
        assert!(ev.contains(&PowerEvent::ChargeStarted));
        assert_eq!(svc.charge_state(), ChargeState::Charging);

        // Drive SoC up to the full threshold via repeated charging polls.
        for i in 2..60u64 {
            power.set(BatteryReading {
                soc_permille: 1000,
                voltage_mv: 4200,
                current_ma: 300,
                temp_centi_c: 2500,
                charger_present: true,
                charging: true,
            });
            svc.poll(&mut power, i * 1000, true);
        }
        assert!(svc.soc_permille() >= PowerConfig::default().full_soc_permille);

        // Charge IC reports done (charging=false) at a high SoC -> Full.
        power.set(BatteryReading {
            soc_permille: 1000,
            voltage_mv: 4200,
            current_ma: 0,
            temp_centi_c: 2500,
            charger_present: true,
            charging: false,
        });
        let ev = svc.poll(&mut power, 100_000, true);
        assert!(ev.contains(&PowerEvent::ChargeComplete));
        assert_eq!(svc.charge_state(), ChargeState::Full);
    }

    #[test]
    fn overheat_throttles_and_reduces_charge_current() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(BatteryReading {
            soc_permille: 500,
            voltage_mv: 4000,
            current_ma: 300,
            temp_centi_c: 2500, // 25.00C, safe
            charger_present: true,
            charging: true,
        });
        svc.poll(&mut power, 0, true);
        assert!(power.charge_current_calls.is_empty());

        // Push temperature above the safe band (default max 45.00C).
        power.set(BatteryReading {
            soc_permille: 500,
            voltage_mv: 4000,
            current_ma: 300,
            temp_centi_c: 5000, // 50.00C, unsafe
            charger_present: true,
            charging: true,
        });
        let ev = svc.poll(&mut power, 1000, true);
        assert!(ev.contains(&PowerEvent::ThermalThrottle { charge_ma: 0 }));
        assert_eq!(power.charge_current_calls.last().copied(), Some(0));

        // Staying hot must not re-fire the event or re-issue the call.
        let ev2 = svc.poll(&mut power, 2000, true);
        assert!(!ev2
            .iter()
            .any(|e| matches!(e, PowerEvent::ThermalThrottle { .. })));
        assert_eq!(power.charge_current_calls.len(), 1);

        // Cooling back into the safe band restores nominal charge current.
        power.set(BatteryReading {
            soc_permille: 500,
            voltage_mv: 4000,
            current_ma: 300,
            temp_centi_c: 2500,
            charger_present: true,
            charging: true,
        });
        svc.poll(&mut power, 3000, true);
        assert_eq!(
            power.charge_current_calls.last().copied(),
            Some(PowerConfig::default().nominal_charge_current_ma)
        );
    }

    #[test]
    fn idle_timeout_sleeps_then_activity_wakes() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(800));
        let idle_timeout = PowerConfig::default().idle_timeout_ms;

        let ev = svc.poll(&mut power, 0, false);
        assert!(ev.is_empty());
        assert!(!svc.is_asleep());

        let ev = svc.poll(&mut power, idle_timeout - 1, false);
        assert!(ev.is_empty(), "must not sleep before the timeout elapses");
        assert!(!svc.is_asleep());

        let ev = svc.poll(&mut power, idle_timeout, false);
        assert!(ev.contains(&PowerEvent::EnterSleep));
        assert!(svc.is_asleep());

        // Staying idle must not re-fire EnterSleep.
        let ev = svc.poll(&mut power, idle_timeout + 5000, false);
        assert!(!ev.contains(&PowerEvent::EnterSleep));

        let ev = svc.poll(&mut power, idle_timeout + 6000, true);
        assert!(ev.contains(&PowerEvent::Wake));
        assert!(!svc.is_asleep());
    }

    #[test]
    fn battery_saver_auto_engages_below_threshold_and_releases_above() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(1000));
        svc.poll(&mut power, 0, true);
        assert_eq!(svc.profile(), PowerProfile::Performance);

        let mut saw_saver = false;
        for i in 1..40u64 {
            power.set(healthy_reading(0));
            let ev = svc.poll(&mut power, i * 1000, true);
            if ev.contains(&PowerEvent::ProfileChanged(PowerProfile::BatterySaver)) {
                saw_saver = true;
            }
        }
        assert!(
            saw_saver,
            "BatterySaver must auto-engage below the SoC threshold"
        );
        assert_eq!(svc.profile(), PowerProfile::BatterySaver);
        assert_eq!(
            power.profile_calls.last().copied(),
            Some(PowerProfile::BatterySaver)
        );

        // Charge back up past the recovery threshold -> reverts.
        let mut saw_performance = false;
        for i in 40..90u64 {
            power.set(BatteryReading {
                soc_permille: 1000,
                voltage_mv: 4200,
                current_ma: 300,
                temp_centi_c: 2500,
                charger_present: true,
                charging: true,
            });
            let ev = svc.poll(&mut power, i * 1000, true);
            if ev.contains(&PowerEvent::ProfileChanged(PowerProfile::Performance)) {
                saw_performance = true;
            }
        }
        assert!(
            saw_performance,
            "profile must revert once SoC recovers past hysteresis"
        );
        assert_eq!(svc.profile(), PowerProfile::Performance);
    }

    #[test]
    fn time_to_empty_is_finite_and_decreases_as_soc_drops() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(BatteryReading {
            soc_permille: 1000,
            voltage_mv: 4000,
            current_ma: -500,
            temp_centi_c: 2500,
            charger_present: false,
            charging: false,
        });
        svc.poll(&mut power, 0, true);
        let first = svc
            .time_to_empty_s()
            .expect("discharging with current must estimate a runtime");
        // 2000mAh design capacity, full SoC, 500mA draw -> 4h = 14400s.
        assert_eq!(first, 14_400);

        for i in 1..30u64 {
            power.set(BatteryReading {
                soc_permille: 300,
                voltage_mv: 3800,
                current_ma: -500,
                temp_centi_c: 2500,
                charger_present: false,
                charging: false,
            });
            svc.poll(&mut power, i * 1000, true);
        }
        let later = svc.time_to_empty_s().expect("still discharging");
        assert!(later < first, "runtime estimate must shrink as SoC drops");
    }

    #[test]
    fn time_to_empty_is_none_while_charging() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(BatteryReading {
            soc_permille: 500,
            voltage_mv: 4000,
            current_ma: 300,
            temp_centi_c: 2500,
            charger_present: true,
            charging: true,
        });
        svc.poll(&mut power, 0, true);
        assert_eq!(svc.time_to_empty_s(), None);
        assert!(svc.time_to_full_s().is_some());
    }

    #[test]
    fn cycle_count_increments_per_cumulative_full_discharge_equivalent() {
        let mut svc = PowerService::default();
        let mut power = MockPower::new(healthy_reading(1000));
        svc.poll(&mut power, 0, true);
        assert_eq!(svc.cycle_count(), 0);

        // Drain to ~0 (one full-discharge-equivalent) via many small steps
        // so the EMA tracks closely (avoids smoothing lag masking the
        // cumulative drop).
        let mut soc = 1000i32;
        let mut t = 1u64;
        while soc > 0 {
            soc = (soc - 20).max(0);
            power.set(healthy_reading(soc as u16));
            svc.poll(&mut power, t * 1000, true);
            t += 1;
        }
        // Let the EMA fully settle at the floor.
        for _ in 0..20 {
            power.set(healthy_reading(0));
            svc.poll(&mut power, t * 1000, true);
            t += 1;
        }
        assert_eq!(
            svc.cycle_count(),
            1,
            "one full discharge must count as exactly one cycle"
        );

        // Charge back to full, then drain again -> second cycle.
        let mut soc_up = 0i32;
        while soc_up < 1000 {
            soc_up = (soc_up + 20).min(1000);
            power.set(BatteryReading {
                soc_permille: soc_up as u16,
                voltage_mv: 4000,
                current_ma: 300,
                temp_centi_c: 2500,
                charger_present: true,
                charging: true,
            });
            svc.poll(&mut power, t * 1000, true);
            t += 1;
        }
        for _ in 0..20 {
            power.set(BatteryReading {
                soc_permille: 1000,
                voltage_mv: 4200,
                current_ma: 300,
                temp_centi_c: 2500,
                charger_present: true,
                charging: true,
            });
            svc.poll(&mut power, t * 1000, true);
            t += 1;
        }

        let mut soc_down = 1000i32;
        power.set(healthy_reading(1000));
        svc.poll(&mut power, t * 1000, true);
        t += 1;
        while soc_down > 0 {
            soc_down = (soc_down - 20).max(0);
            power.set(healthy_reading(soc_down as u16));
            svc.poll(&mut power, t * 1000, true);
            t += 1;
        }
        for _ in 0..20 {
            power.set(healthy_reading(0));
            svc.poll(&mut power, t * 1000, true);
            t += 1;
        }

        assert_eq!(
            svc.cycle_count(),
            2,
            "two full discharges must count as two cycles"
        );
    }
}
