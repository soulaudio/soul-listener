//! Clock/voltage **transition sequencer** — the ordering rule the DAP's DVFS
//! policy must obey when it actually changes clock, expressed as a pure,
//! host-testable plan instead of prose.
//!
//! # Why this module exists
//! [`dvfs`](super::dvfs) answers *which clock* to run at; [`model`](super::model)
//! answers *what that costs*. Neither answers the question that actually bricks a
//! board: **in what order** do you touch the regulator, the flash wait-states,
//! and the PLL? On an STM32H7 that ordering is a hard rule, and getting it wrong
//! does not degrade gracefully — under-waited flash reads or an over-clocked
//! undervolted core hard-fault. The project's memory/power enforcement roster
//! (`docs/plans/2026-11-20-memory-power-best-practices-and-enforcement.md`,
//! "Clock sequencing") listed it as needing "a tested clock-transition routine;
//! the ordering is a hard rule". This module is that routine, minus the register
//! writes: it emits an ordered [`ClockPlan`] of [`ClockStep`]s, and its tests
//! prove the two safety invariants hold at **every intermediate instant** of
//! every transition the DVFS policy can ask for. `board-h7a3` executes the plan
//! (RCC/PWR/FLASH writes); the *decision* is gated here, on the host.
//!
//! # The two invariants (what the tests actually prove)
//! At every step boundary of every plan:
//! 1. **Never under-waited** — the programmed flash wait-state count is `>=` the
//!    count the datasheet requires for the clock *and* voltage scale in effect
//!    right then (see [`Vos::flash_wait_states`]). Over-waiting is merely slow;
//!    under-waiting corrupts instruction fetch.
//! 2. **Never over-clocked for the voltage** — the running clock is `<=` the
//!    maximum the in-effect [`Vos`] supports (see [`Vos::max_clock_mhz`]).
//!
//! Those two force the familiar asymmetry, which falls out of the invariants
//! rather than being asserted on faith:
//!
//! | Direction | Order |
//! |---|---|
//! | **Clock down** | clock ↓ → wait-states → VOS ↓ (voltage drops *last*) |
//! | **Clock up** | VOS ↑ → wait-states → clock ↑ (voltage rises *first*) |
//!
//! i.e. the anti-pattern the enforcement doc names — "VOS-before-freq
//! transitions (hard faults)" — is exactly what invariant 2 rejects on a
//! downshift, and invariant 1 rejects the mirror-image wait-state mistake.
//!
//! # Datasheet provenance
//! Every number here is ST's own, taken from the `STM32H7_DEV_ID == 0x480`
//! branch of `stm32h7xx_ll_utils.c` (STMicroelectronics/stm32h7xx-hal-driver) —
//! the CubeH7 LL implementation of RM0455's "FLASH recommended number of wait
//! states" table and the per-scale frequency ceilings. `0x480` is the
//! STM32H7A3/7B3/7B0 device ID, i.e. **this** board, not the H743 whose tables
//! differ (see [`Vos::max_clock_mhz`]'s note — an earlier design-doc figure in
//! this repo was H743's).
//!
//! # Scope
//! Pure, `no_std`, no hardware: no register encoding, no PLL (M/N/P) solving, no
//! SMPS/LDO selection, no `VOSRDY` polling — those are `board-h7a3`'s, and
//! [`ClockStep`] documents the blocking each step implies. HPRE is assumed `1`
//! (`hclk == cpu_ck`), which the H7A3 permits since its AXI/AHB ceiling is the
//! full 280 MHz; a board that divides HPRE calls
//! [`Vos::flash_wait_states`] with its own HCLK instead of going through
//! [`ClockPoint::for_clock`].

use heapless::Vec;

/// Core voltage scaling level (`PWR` regulator voltage scale), highest voltage
/// (`Vos0`) to lowest (`Vos3`) — the "V" in DVFS. Lower voltage is less power
/// (`P = α·C·V²·f`) but caps the clock harder *and* costs more flash wait
/// states, which is the whole tension this module sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Vos {
    /// Highest voltage, highest clock ceiling (280 MHz on this part).
    Vos0,
    /// 225 MHz ceiling.
    Vos1,
    /// 160 MHz ceiling.
    Vos2,
    /// Lowest voltage, lowest clock ceiling (88 MHz on this part) — the
    /// cheapest operating point, but *not* reachable by every codec (see
    /// `power::model`'s `heaviest_format_needs_vos2_not_vos3`).
    Vos3,
}

/// Number of [`Vos`] levels — the length of [`Vos::ALL`].
pub const VOS_LEVELS: usize = 4;

impl Vos {
    /// Every level, highest voltage first (ascending `Vos` ordinal = descending
    /// voltage), so `ALL.iter().rev()` walks cheapest-first.
    pub const ALL: [Self; VOS_LEVELS] = [Self::Vos0, Self::Vos1, Self::Vos2, Self::Vos3];

    /// The maximum CPU/AXI/AHB clock (MHz) this scale supports, per ST's
    /// `UTILS_MAX_FREQUENCY_SCALE{0,1,2,3}` for `STM32H7_DEV_ID == 0x480`:
    /// **280 / 225 / 160 / 88 MHz**.
    ///
    /// Note the H7A3 numbers are *not* the H743's (480/300/200 — no VOS0 row).
    /// An earlier figure in this repo's design doc §4.2 ("VOS3 1.0 V/200 MHz")
    /// is the H743 table; on this board VOS3 caps at 88 MHz. `Vos0`'s 280 MHz
    /// agrees with `model::H7A3_MAX_CLOCK_MHZ`, which the
    /// `vos0_ceiling_matches_the_models_max_clock` test pins.
    #[must_use]
    pub const fn max_clock_mhz(self) -> u32 {
        match self {
            Self::Vos0 => 280,
            Self::Vos1 => 225,
            Self::Vos2 => 160,
            Self::Vos3 => 88,
        }
    }

    /// The **cheapest** (lowest-voltage) scale that still supports `clock_mhz` —
    /// the DVFS choice, given [`dvfs`](super::dvfs) has already picked the lowest
    /// clock that sustains the codec. `None` if no scale supports the clock
    /// (i.e. `clock_mhz` exceeds [`Vos0`](Self::Vos0)'s 280 MHz ceiling).
    #[must_use]
    pub fn lowest_supporting(clock_mhz: u32) -> Option<Self> {
        let mut found = None;
        // ALL is highest-voltage-first, so the last match is the cheapest.
        for vos in Self::ALL {
            if clock_mhz <= vos.max_clock_mhz() {
                found = Some(vos);
            }
        }
        found
    }

    /// The flash wait-state count (`FLASH_ACR.LATENCY`) required at this voltage
    /// scale for a bus clock of `hclk_mhz`, per ST's
    /// `UTILS_SCALE{n}_LATENCY{w}_FREQ` thresholds for `STM32H7_DEV_ID == 0x480`
    /// (RM0455's wait-state table). Each threshold is an **inclusive upper
    /// bound**, and the answer is the smallest `w` whose bound covers `hclk_mhz`
    /// — matching `LL_SetFlashLatency`'s own `(prev < f) && (f <= this)` ladder:
    ///
    /// | scale | 0 WS | 1 WS | 2 WS | 3 WS | 4 WS | 5 WS | 6 WS |
    /// |---|---|---|---|---|---|---|---|
    /// | VOS0 | ≤42 | ≤84 | ≤126 | ≤168 | ≤210 | ≤252 | ≤280 |
    /// | VOS1 | ≤38 | ≤76 | ≤114 | ≤152 | ≤190 | ≤225 | — |
    /// | VOS2 | ≤34 | ≤68 | ≤102 | ≤136 | ≤160 | — | — |
    /// | VOS3 | ≤22 | ≤44 | ≤66 | ≤88 | — | — | — |
    ///
    /// `None` if `hclk_mhz` exceeds this scale's last row — the same condition
    /// `LL_SetFlashLatency` returns `ERROR` for, and (by construction) exactly
    /// when `hclk_mhz > self.max_clock_mhz()`.
    #[must_use]
    pub fn flash_wait_states(self, hclk_mhz: u32) -> Option<u8> {
        let thresholds: &[u32] = match self {
            Self::Vos0 => &[42, 84, 126, 168, 210, 252, 280],
            Self::Vos1 => &[38, 76, 114, 152, 190, 225],
            Self::Vos2 => &[34, 68, 102, 136, 160],
            Self::Vos3 => &[22, 44, 66, 88],
        };
        let mut ws = 0usize;
        while ws < thresholds.len() {
            if hclk_mhz <= thresholds[ws] {
                return Some(ws as u8);
            }
            ws += 1;
        }
        None
    }
}

/// A complete, self-consistent operating point: a clock, the cheapest [`Vos`]
/// that supports it, and the wait-states that pair requires.
///
/// Only constructible via [`for_clock`](Self::for_clock), which derives the VOS
/// and wait-states from the clock. That is deliberate: it makes an inconsistent
/// point (a clock its VOS can't sustain, wait-states that don't match) *
/// unrepresentable*, and it makes the ordering monotone — a higher clock always
/// implies an equal-or-higher voltage — which is what lets [`plan`] stay a
/// three-step sequence with a provable safety argument rather than a
/// case-analysis over mixed clock-up/voltage-down transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockPoint {
    clock_mhz: u32,
    vos: Vos,
    wait_states: u8,
}

impl ClockPoint {
    /// The canonical minimum-voltage operating point for `clock_mhz` (HPRE = 1,
    /// so `hclk == cpu_ck` — see the module doc). `None` if no voltage scale
    /// supports that clock (`clock_mhz > 280`), or if it is `0`.
    #[must_use]
    pub fn for_clock(clock_mhz: u32) -> Option<Self> {
        if clock_mhz == 0 {
            return None;
        }
        let vos = Vos::lowest_supporting(clock_mhz)?;
        let wait_states = vos.flash_wait_states(clock_mhz)?;
        Some(Self {
            clock_mhz,
            vos,
            wait_states,
        })
    }

    /// CPU/bus clock, MHz.
    #[must_use]
    pub const fn clock_mhz(self) -> u32 {
        self.clock_mhz
    }

    /// The voltage scale this point runs at — the cheapest supporting its clock.
    #[must_use]
    pub const fn vos(self) -> Vos {
        self.vos
    }

    /// The flash wait-states this point requires.
    #[must_use]
    pub const fn wait_states(self) -> u8 {
        self.wait_states
    }
}

/// One hardware action in a [`ClockPlan`], in the order the board must perform
/// them. Each variant names the blocking it implies — the plan is a sequence of
/// *completed* effects, not fire-and-forget register pokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockStep {
    /// Program the regulator voltage scale and **block until the new scale is
    /// actually reached** (`PWR_SRDCR.VOS` then poll `PWR_CSR1.ACTVOSRDY` /
    /// `VOSRDY`). Returning before the voltage has settled would void invariant
    /// 2 for the following [`SetSysClock`](Self::SetSysClock).
    SetVoltageScale(Vos),
    /// Program `FLASH_ACR.LATENCY` and **read it back** until it reads the
    /// written value (ST's own requirement — the write is not immediate).
    SetFlashWaitStates(u8),
    /// Switch the CPU/bus clock to this frequency (MHz): reprogram/re-lock the
    /// PLL and block until the new clock is the live `sys_ck`.
    SetSysClock(u32),
}

/// Upper bound on a [`ClockPlan`]'s length: a transition touches at most the
/// three knobs once each ([`ClockStep`] has three variants).
pub const MAX_CLOCK_STEPS: usize = 3;

/// The ordered, bounded, no-alloc step sequence [`plan`] returns.
pub type ClockPlan = Vec<ClockStep, MAX_CLOCK_STEPS>;

/// The ordered steps to move from operating point `from` to `to`, obeying both
/// module-doc invariants at every intermediate instant.
///
/// - `from == to` ⇒ an **empty** plan (no spurious regulator/PLL churn on a
///   DVFS re-evaluation that didn't change its mind).
/// - **Lowering** the clock ⇒ `SetSysClock` → `SetFlashWaitStates` →
///   `SetVoltageScale`: the clock comes down while the old (higher) voltage is
///   still up, and the voltage drops only once the wait-states already cover the
///   lower voltage's needs.
/// - **Raising** the clock ⇒ `SetVoltageScale` → `SetFlashWaitStates` →
///   `SetSysClock`: the voltage goes up first (so the new clock is never
///   over-clocked for the voltage), then the wait-states, then the clock.
///
/// Steps whose value already matches are omitted (e.g. 69 → 74 MHz stays in
/// VOS2 at 2 WS and emits only `SetSysClock`), so the plan is also the minimal
/// one — but never at the cost of an invariant, which the
/// `no_plan_ever_underwaits_flash_or_overclocks_the_voltage` sweep checks over
/// every pair of selectable clocks.
#[must_use]
pub fn plan(from: ClockPoint, to: ClockPoint) -> ClockPlan {
    let mut steps = ClockPlan::new();
    if from == to {
        return steps;
    }

    let vos_step = (to.vos != from.vos).then_some(ClockStep::SetVoltageScale(to.vos));
    let ws_step = (to.wait_states != from.wait_states)
        .then_some(ClockStep::SetFlashWaitStates(to.wait_states));
    let clock_step =
        (to.clock_mhz != from.clock_mhz).then_some(ClockStep::SetSysClock(to.clock_mhz));

    // `ClockPoint` is monotone (higher clock => equal-or-higher voltage), so the
    // clock's direction alone decides the order; a pure voltage change (same
    // clock, different VOS) can only be a drop, which takes the lowering order.
    let lowering =
        to.clock_mhz < from.clock_mhz || (to.clock_mhz == from.clock_mhz && to.vos > from.vos);
    let ordered = if lowering {
        [clock_step, ws_step, vos_step]
    } else {
        [vos_step, ws_step, clock_step]
    };
    for step in ordered.into_iter().flatten() {
        // Infallible: at most three `Some`s into a 3-capacity Vec.
        let _ = steps.push(step);
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::dvfs::clock_mhz_for_profile;
    use crate::power::model::{estimate_target_rtf, H7A3_MAX_CLOCK_MHZ, IDLE_CLOCK_MHZ};
    use crate::power::PowerProfile;

    /// Mutable mirror of the hardware the plan talks to, so a test can replay a
    /// plan step by step and assert the invariants at every boundary.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct HwState {
        clock_mhz: u32,
        vos: Vos,
        wait_states: u8,
    }

    impl HwState {
        fn at(point: ClockPoint) -> Self {
            Self {
                clock_mhz: point.clock_mhz(),
                vos: point.vos(),
                wait_states: point.wait_states(),
            }
        }

        fn apply(&mut self, step: ClockStep) {
            match step {
                ClockStep::SetVoltageScale(vos) => self.vos = vos,
                ClockStep::SetFlashWaitStates(ws) => self.wait_states = ws,
                ClockStep::SetSysClock(mhz) => self.clock_mhz = mhz,
            }
        }

        /// Panics unless BOTH module-doc invariants hold right now.
        fn assert_safe(&self, context: &str) {
            assert!(
                self.clock_mhz <= self.vos.max_clock_mhz(),
                "{context}: {} MHz exceeds {:?}'s {} MHz ceiling — over-clocked for the voltage",
                self.clock_mhz,
                self.vos,
                self.vos.max_clock_mhz()
            );
            let required = self
                .vos
                .flash_wait_states(self.clock_mhz)
                .unwrap_or_else(|| panic!("{context}: no wait-state row for {self:?}"));
            assert!(
                self.wait_states >= required,
                "{context}: flash at {} WS but {:?} @ {} MHz requires {required} WS — \
                 under-waited flash corrupts instruction fetch",
                self.wait_states,
                self.vos,
                self.clock_mhz
            );
        }
    }

    /// A spread of clocks covering every VOS band and both sides of every
    /// wait-state threshold in all four bands, plus the clocks the DVFS policy
    /// actually selects (see `every_dvfs_clock_has_a_legal_operating_point`).
    fn interesting_clocks() -> impl Iterator<Item = u32> {
        const EDGES: [u32; 20] = [
            22, 34, 38, 42, 44, 66, 68, 76, 84, 88, 102, 114, 126, 136, 152, 160, 168, 190, 210,
            252,
        ];
        EDGES
            .into_iter()
            .flat_map(|e| [e.saturating_sub(1), e, e + 1])
            .chain([1, 60, 69, 74, 102, 158, 225, 280])
            .filter(|&c| (1..=H7A3_MAX_CLOCK_MHZ).contains(&c))
    }

    // --- the datasheet tables ------------------------------------------

    /// VOS0's ceiling is the same 280 MHz `power::model` calls this board's max
    /// clock — the two datasheet transcriptions must agree (they're in separate
    /// modules; `model` can't see this one's table).
    #[test]
    fn vos0_ceiling_matches_the_models_max_clock() {
        assert_eq!(Vos::Vos0.max_clock_mhz(), H7A3_MAX_CLOCK_MHZ);
    }

    /// The four ceilings are strictly descending with voltage, and
    /// `lowest_supporting` really picks the cheapest scale that covers a clock
    /// (including exactly *at* each ceiling, where the cheaper scale is the
    /// right answer).
    #[test]
    fn lowest_supporting_picks_the_cheapest_scale_that_covers_the_clock() {
        for pair in Vos::ALL.windows(2) {
            assert!(
                pair[0].max_clock_mhz() > pair[1].max_clock_mhz(),
                "{:?} must have a higher ceiling than {:?}",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(Vos::lowest_supporting(88), Some(Vos::Vos3));
        assert_eq!(Vos::lowest_supporting(89), Some(Vos::Vos2));
        assert_eq!(Vos::lowest_supporting(160), Some(Vos::Vos2));
        assert_eq!(Vos::lowest_supporting(161), Some(Vos::Vos1));
        assert_eq!(Vos::lowest_supporting(225), Some(Vos::Vos1));
        assert_eq!(Vos::lowest_supporting(226), Some(Vos::Vos0));
        assert_eq!(Vos::lowest_supporting(280), Some(Vos::Vos0));
        assert_eq!(
            Vos::lowest_supporting(281),
            None,
            "above the part's rated max no scale applies"
        );
    }

    /// Spot-check the transcribed wait-state ladder against ST's own comments in
    /// `LL_SetFlashLatency` (`STM32H7_DEV_ID == 0x480`), at both sides of a
    /// threshold, and pin that a clock above a scale's ceiling has no row at all
    /// — the `ERROR` branch.
    #[test]
    fn flash_wait_states_match_the_datasheet_ladder() {
        // VOS0: 0 WS <= 42 < 1 WS <= 84 < 2 WS <= 126 ... 6 WS <= 280.
        assert_eq!(Vos::Vos0.flash_wait_states(42), Some(0));
        assert_eq!(Vos::Vos0.flash_wait_states(43), Some(1));
        assert_eq!(Vos::Vos0.flash_wait_states(84), Some(1));
        assert_eq!(Vos::Vos0.flash_wait_states(85), Some(2));
        assert_eq!(Vos::Vos0.flash_wait_states(280), Some(6));
        // VOS3's shorter ladder tops out at 3 WS / 88 MHz.
        assert_eq!(Vos::Vos3.flash_wait_states(22), Some(0));
        assert_eq!(Vos::Vos3.flash_wait_states(88), Some(3));
        assert_eq!(Vos::Vos3.flash_wait_states(89), None);
        // The same clock needs MORE wait-states at a lower voltage — the reason
        // a VOS drop can't be done before the wait-states are raised.
        assert_eq!(Vos::Vos0.flash_wait_states(68), Some(1));
        assert_eq!(Vos::Vos3.flash_wait_states(68), Some(3));
        // Every scale's "no row" boundary is exactly its clock ceiling.
        for vos in Vos::ALL {
            assert!(vos.flash_wait_states(vos.max_clock_mhz()).is_some());
            assert_eq!(vos.flash_wait_states(vos.max_clock_mhz() + 1), None);
        }
    }

    /// Lowering the voltage never *lowers* the required wait-states at a fixed
    /// clock — the monotonicity the sequencing rule depends on.
    #[test]
    fn lower_voltage_never_needs_fewer_wait_states() {
        for clock in interesting_clocks() {
            let mut prev: Option<u8> = None;
            for vos in Vos::ALL {
                // ALL is highest-voltage-first.
                if let Some(ws) = vos.flash_wait_states(clock) {
                    if let Some(prev) = prev {
                        assert!(
                            ws >= prev,
                            "{clock} MHz needs {ws} WS at {vos:?} but {prev} WS at a higher voltage"
                        );
                    }
                    prev = Some(ws);
                }
            }
        }
    }

    // --- ClockPoint ----------------------------------------------------

    #[test]
    fn clock_point_is_self_consistent_and_monotone() {
        for clock in interesting_clocks() {
            let p = ClockPoint::for_clock(clock).expect("every selectable clock has a point");
            assert_eq!(p.clock_mhz(), clock);
            assert_eq!(
                p.wait_states(),
                p.vos().flash_wait_states(clock).expect("row exists"),
                "{clock} MHz: point's wait-states must be its VOS's requirement"
            );
            assert!(clock <= p.vos().max_clock_mhz());
        }
        // Monotone: a higher clock never maps to a *lower* voltage (a higher
        // `Vos` ordinal). `plan`'s three-step shape relies on this.
        let mut clocks: heapless::Vec<u32, 128> = interesting_clocks().collect();
        clocks.sort_unstable();
        for pair in clocks.windows(2) {
            let lo = ClockPoint::for_clock(pair[0]).expect("point");
            let hi = ClockPoint::for_clock(pair[1]).expect("point");
            assert!(
                hi.vos() <= lo.vos(),
                "{} MHz at {:?} but {} MHz at {:?} — voltage must not fall as clock rises",
                pair[1],
                hi.vos(),
                pair[0],
                lo.vos()
            );
        }
        assert_eq!(
            ClockPoint::for_clock(0),
            None,
            "0 MHz is not an operating point"
        );
        assert_eq!(
            ClockPoint::for_clock(H7A3_MAX_CLOCK_MHZ + 1),
            None,
            "above the part's rated max"
        );
    }

    // --- the invariants ------------------------------------------------

    /// **The gate this module exists for.** For every ordered pair of selectable
    /// clocks, replay the plan step by step and assert both invariants hold at
    /// the start, after every single step, and at the end. A reordering of
    /// `plan`'s steps (or a wrong wait-state row) fails here rather than on
    /// silicon.
    #[test]
    fn no_plan_ever_underwaits_flash_or_overclocks_the_voltage() {
        let clocks: heapless::Vec<u32, 128> = interesting_clocks().collect();
        for &from_mhz in &clocks {
            for &to_mhz in &clocks {
                let from = ClockPoint::for_clock(from_mhz).expect("point");
                let to = ClockPoint::for_clock(to_mhz).expect("point");
                let mut hw = HwState::at(from);
                hw.assert_safe("start");
                for (i, step) in plan(from, to).into_iter().enumerate() {
                    hw.apply(step);
                    hw.assert_safe(&alloc_free_context(from_mhz, to_mhz, i));
                }
                assert_eq!(
                    hw,
                    HwState::at(to),
                    "{from_mhz} -> {to_mhz} MHz: the plan must actually arrive at the target point"
                );
            }
        }
    }

    /// `heapless::String` context line for the sweep's assertions — this crate
    /// is `no_std`-first, so no `format!`.
    fn alloc_free_context(from: u32, to: u32, step: usize) -> heapless::String<64> {
        use core::fmt::Write as _;
        let mut s = heapless::String::new();
        let _ = write!(s, "{from} -> {to} MHz, after step {step}");
        s
    }

    /// A no-change re-evaluation emits nothing: the DVFS policy can be polled
    /// every tick without churning the regulator or re-locking the PLL.
    #[test]
    fn identical_points_produce_an_empty_plan() {
        for clock in interesting_clocks() {
            let p = ClockPoint::for_clock(clock).expect("point");
            assert!(
                plan(p, p).is_empty(),
                "{clock} MHz -> itself must emit no steps"
            );
        }
    }

    /// The ordering rule, stated directly (the sweep above proves it's *safe*;
    /// this pins that it's the documented order, so a refactor that happens to
    /// stay safe by over-waiting still can't silently change the contract).
    #[test]
    fn voltage_moves_last_on_the_way_down_and_first_on_the_way_up() {
        let low = ClockPoint::for_clock(60).expect("point"); // VOS3
        let high = ClockPoint::for_clock(280).expect("point"); // VOS0
        assert_eq!(low.vos(), Vos::Vos3);
        assert_eq!(high.vos(), Vos::Vos0);

        let down = plan(high, low);
        assert_eq!(
            down.as_slice(),
            &[
                ClockStep::SetSysClock(60),
                ClockStep::SetFlashWaitStates(low.wait_states()),
                ClockStep::SetVoltageScale(Vos::Vos3),
            ],
            "lowering: clock first, voltage LAST (VOS-before-freq is the hard-fault anti-pattern)"
        );

        let up = plan(low, high);
        assert_eq!(
            up.as_slice(),
            &[
                ClockStep::SetVoltageScale(Vos::Vos0),
                ClockStep::SetFlashWaitStates(high.wait_states()),
                ClockStep::SetSysClock(280),
            ],
            "raising: voltage FIRST, clock last"
        );
    }

    /// A transition inside one VOS band that doesn't cross a wait-state
    /// threshold is a single step — no regulator or flash write at all.
    #[test]
    fn transition_within_a_band_touches_only_the_clock() {
        // 69 and 74 MHz both sit in VOS3's 66 < f <= 88 => 3 WS row.
        let a = ClockPoint::for_clock(69).expect("point");
        let b = ClockPoint::for_clock(74).expect("point");
        assert_eq!(a.vos(), b.vos());
        assert_eq!(a.wait_states(), b.wait_states());
        assert_eq!(plan(a, b).as_slice(), &[ClockStep::SetSysClock(74)]);
        assert_eq!(plan(b, a).as_slice(), &[ClockStep::SetSysClock(69)]);
    }

    // --- tie-in to the DVFS policy -------------------------------------

    // Same HOST RTF set as `power::model`/`power::dvfs`'s own tests (see
    // `model` for the measurement method/caveat) -- duplicated so this
    // module's tests read standalone.
    const ALL_DECODE_FORMATS: [(&str, f32); 7] = [
        ("wav", 5127.3),
        ("flac", 266.6),
        ("mp3", 465.1),
        ("aac", 247.2),
        ("alac", 265.3),
        ("opus", 179.2),
        ("wavpack", 264.6),
    ];

    /// Every clock the DVFS policy can actually select, for every profile and
    /// every shipped format, must be a **legal** operating point — a voltage
    /// scale exists for it and so does a wait-state row. This is the seam
    /// between `dvfs` (which picks MHz from an RTF) and this module (which has
    /// to turn MHz into datasheet-legal register state): a policy change that
    /// starts returning an unachievable clock fails here.
    #[test]
    fn every_dvfs_clock_has_a_legal_operating_point() {
        for profile in [
            PowerProfile::Performance,
            PowerProfile::Balanced,
            PowerProfile::BatterySaver,
        ] {
            for (label, host_rtf) in ALL_DECODE_FORMATS {
                let clock = clock_mhz_for_profile(
                    profile,
                    estimate_target_rtf(host_rtf),
                    H7A3_MAX_CLOCK_MHZ,
                );
                let point = ClockPoint::for_clock(clock).unwrap_or_else(|| {
                    panic!("{label} @ {profile:?}: dvfs picked {clock} MHz with no legal point")
                });
                assert!(point.clock_mhz() <= point.vos().max_clock_mhz());
                // And the transition from the idle floor to it is safe.
                let idle = ClockPoint::for_clock(IDLE_CLOCK_MHZ).expect("idle point");
                let mut hw = HwState::at(idle);
                for step in plan(idle, point) {
                    hw.apply(step);
                    hw.assert_safe("idle -> dvfs clock");
                }
                assert_eq!(hw, HwState::at(point));
            }
        }
    }

    /// The DAP's idle floor (`model::IDLE_CLOCK_MHZ` = 60 MHz) lands in the
    /// cheapest voltage scale — so idling really is the low-power state, not
    /// just a low clock at a high voltage.
    #[test]
    fn the_idle_clock_sits_in_the_cheapest_voltage_scale() {
        let idle = ClockPoint::for_clock(IDLE_CLOCK_MHZ).expect("idle point");
        assert_eq!(
            idle.vos(),
            Vos::Vos3,
            "the idle floor should reach VOS3 ({} MHz ceiling)",
            Vos::Vos3.max_clock_mhz()
        );
    }
}
