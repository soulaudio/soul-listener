//! Cross-slice **seam gates**: invariants that span two `dap-core` modules which
//! deliberately cannot see each other.
//!
//! `tools/check-arch-boundaries.sh` keeps the feature slices (`audio`, `power`,
//! `display`, `screens`) from importing one another — each may reach only into
//! the dependency-free `ports` layer. That layering is what makes the slices
//! independently testable, but it also means an invariant *between* two slices
//! has nowhere inside `dap-core` to live: a test in `power::model` may not
//! mention `display`, and vice versa.
//!
//! This crate sits one layer above all of them (it depends on `dap-core`, not
//! the reverse), so it is the correct home for exactly those gates. Keeping them
//! here rather than weakening the boundary is the point.

use dap_core::display::refresh::{
    self, avg_display_ma, configured_avg_display_ma, ACTIVE_REFRESH_MA, MAX_PARTIALS_BEFORE_FULL,
    MIN_PARTIAL_INTERVAL_MS,
};
use dap_core::firmware::RENDER_PERIOD_MS;
use dap_core::power::model::{DISPLAY_AVG_MA, PERIPHERAL_PLAYBACK_MA};

/// **The display seam.** `power::model::DISPLAY_AVG_MA` is a line item in the
/// battery estimate; `display::refresh` is the policy that actually decides how
/// often the panel runs a refresh waveform. Only one of them can be right, and
/// nothing inside `dap-core` can compare them.
///
/// This is not a hypothetical drift: `DISPLAY_AVG_MA` was `0.5` on the reasoning
/// that "only the brief refresh on a progress change draws", while `App::render`
/// ran a **full** 3 s refresh and `firmware::step` called it every second — a
/// saturated panel at ~34 mA, roughly 68x the budgeted figure. The battery model
/// was describing a device the UI code did not implement.
///
/// Asserted in both directions:
/// - the policy must fit inside the budget (else runtime is over-estimated), and
/// - the budget must not be wildly *larger* than the policy needs (else runtime
///   is needlessly pessimistic and the slack hides a future regression).
#[test]
fn display_refresh_policy_fits_the_battery_model() {
    let policy_ma = configured_avg_display_ma();

    assert!(
        policy_ma <= DISPLAY_AVG_MA,
        "the refresh policy draws {policy_ma:.2} mA (a partial every \
         {MIN_PARTIAL_INTERVAL_MS} ms, one full per {MAX_PARTIALS_BEFORE_FULL} partials) but \
         power::model budgets only {DISPLAY_AVG_MA:.2} mA for the display — the battery \
         estimate would be optimistic. Either raise DISPLAY_AVG_MA or slow the cadence."
    );
    assert!(
        DISPLAY_AVG_MA <= policy_ma * 2.0,
        "power::model budgets {DISPLAY_AVG_MA:.2} mA for a display policy that only draws \
         {policy_ma:.2} mA — more than 2x slack means the budget is stale (runtime is \
         under-estimated, and the headroom would absorb a real regression unnoticed)."
    );
    println!(
        "display seam: policy {policy_ma:.2} mA <= model budget {DISPLAY_AVG_MA:.2} mA \
         (of {PERIPHERAL_PLAYBACK_MA:.1} mA total output path)"
    );
}

/// The old behaviour, pinned as the thing the policy exists to prevent: a full
/// refresh on every render at the previous 1 s cadence saturates the panel —
/// 100 % refresh duty, the full `ACTIVE_REFRESH_MA`. It also busts the battery
/// model's *entire* output-path budget on the display alone, which is the sharp
/// way to state why this was a bug and not merely a missed optimization.
#[test]
fn the_old_always_full_refresh_would_bust_the_whole_output_path_budget() {
    // `partials_per_full = 0` is "every refresh is full"; 1 s was RENDER_PERIOD_MS.
    let old_ma = avg_display_ma(1_000, 0);
    assert!(
        (old_ma - ACTIVE_REFRESH_MA).abs() < 1e-3,
        "a 3 s full refresh demanded every 1 s should pin the panel at its active current, \
         got {old_ma:.2} mA"
    );
    assert!(
        old_ma > PERIPHERAL_PLAYBACK_MA,
        "the old display behaviour ({old_ma:.1} mA) should exceed the model's ENTIRE \
         output-path budget ({PERIPHERAL_PLAYBACK_MA:.1} mA: DAC + SD + display) — that is \
         the size of the discrepancy the refresh policy closed"
    );
}

/// The run loop's redraw cadence and the refresh policy's throttle must agree.
/// `firmware::step` gates the *redraw* (a full layout + widget pass) and
/// `display::refresh` gates the *panel commit*; a redraw cadence faster than the
/// commit throttle is work thrown away every period.
///
/// They are the same constant today (`RENDER_PERIOD_MS` is defined as
/// `MIN_PARTIAL_INTERVAL_MS`), so this gate exists to catch someone redefining
/// one independently.
#[test]
fn the_run_loop_does_not_redraw_faster_than_the_panel_can_commit() {
    // A `const` block, so a divergence fails the *build* rather than the test
    // run — both operands are compile-time constants.
    const {
        assert!(
            RENDER_PERIOD_MS >= MIN_PARTIAL_INTERVAL_MS,
            "firmware::step redraws faster than the refresh policy will commit — the \
             surplus redraws are pure wasted layout work"
        )
    };
}

/// A sanity floor on the panel spec the policy derives everything from: this is
/// the GDEM0397T81P, a partial refresh really is an order of magnitude cheaper
/// than a full one, and the ghosting bound is a real bound. If `eink-specs` is
/// ever repointed at a different panel, the policy's constants (and the battery
/// budget above) need rederiving, and this says so loudly.
#[test]
fn the_policy_is_derived_from_the_expected_panel() {
    assert_eq!(refresh::PANEL.name, "GDEM0397T81P");
    assert_eq!(refresh::PANEL.full_refresh_ms, 3_000);
    assert_eq!(refresh::PANEL.partial_refresh_ms, 300);
    const {
        assert!(
            refresh::PANEL.full_refresh_ms >= refresh::PANEL.partial_refresh_ms * 8,
            "the whole policy rests on a partial refresh being much cheaper than a full one"
        )
    };
    const {
        assert!(
            MAX_PARTIALS_BEFORE_FULL >= 2 && MAX_PARTIALS_BEFORE_FULL <= 32,
            "the ghosting bound is outside any plausible range for a Carta panel — check \
             PANEL.ghosting_rate_partial"
        )
    };
}
