//! E-ink **refresh policy** — whether a rendered frame earns a full-panel
//! flash, a partial update of just the dirty region, or nothing at all.
//!
//! # Why this module exists
//! The memory/power roster asks for a policy that "minimizes full refreshes
//! (power + ghosting)" and gates *"no full refresh on a 1-field change"*. Before
//! this module there was no policy: [`App::render`](crate::app::App::render)
//! called [`EinkRefresh::full_refresh`](super::EinkRefresh::full_refresh)
//! unconditionally, and `firmware::step` called `render` once per
//! `RENDER_PERIOD_MS` (1 s). On this panel a full refresh takes
//! **3 s** ([`PANEL`]`.full_refresh_ms`), so the shipped loop asked for a
//! 3-second flash every second — not merely wasteful but physically
//! unsatisfiable, and ~68x the display current the battery model budgets for
//! (see [`avg_display_ma`] and the `display_refresh_policy_fits_the_battery_model`
//! gate in `dap-arch-tests`). The fix is this policy plus the wiring in
//! `App::render`.
//!
//! # The decision
//! [`RefreshDecider::decide`] takes what changed ([`FrameChange`]), the dirty
//! region `eink-ui`'s [`DirtyTracker`](eink_ui::refresh::DirtyTracker) measured,
//! and `now_ms`, and returns a [`RefreshAction`]:
//!
//! | Situation | Action | Why |
//! |---|---|---|
//! | First frame | [`Full`](RefreshAction::Full) | Nothing on the panel to partially update. |
//! | Nothing changed | [`None`](RefreshAction::None) | A redraw of an identical frame still costs a refresh waveform; don't. |
//! | Content changed (track, screen) | [`Full`](RefreshAction::Full) | A materially different frame; also the natural moment to clear ghosting. |
//! | Ghosting budget spent | [`Full`](RefreshAction::Full) | See [`MAX_PARTIALS_BEFORE_FULL`]. |
//! | Minor change, interval elapsed | [`Partial`](RefreshAction::Partial) | Progress/volume/battery tick: 300 ms, no flash. |
//! | Minor change, too soon | [`None`](RefreshAction::None) | The throttle that makes the power budget hold. |
//!
//! The asymmetry in the last two rows is the point: a **content** change
//! refreshes immediately (a user who presses play should not wait), while a
//! **minor** change — the only kind that recurs on a timer — is rate-limited.
//! So the interval never delays anything a user initiated.
//!
//! # Scope
//! Pure, `no_std`, no hardware: this decides, the caller calls
//! [`full_refresh`](super::EinkRefresh::full_refresh) /
//! [`partial_refresh`](super::EinkRefresh::partial_refresh). It also deliberately
//! does **not** reach into [`crate::power`] — the display slice may not depend on
//! the power slice (see `tools/check-arch-boundaries.sh`), so the seam gate that
//! ties [`avg_display_ma`] to `power::model::DISPLAY_AVG_MA` lives one layer up,
//! in `dap-arch-tests`.

use eink_specs::DisplaySpec;
use embedded_graphics::primitives::Rectangle;

pub use eink_ui::refresh::RefreshKind;

/// The panel this DAP ships, as the single source of refresh timing — Good
/// Display **GDEM0397T81P** (800x480 Carta 1200, SSD1677): full 3000 ms, partial
/// 300 ms, fast 1500 ms, `ghosting_rate_partial` 0.10 (see `eink-specs`, which
/// cites the datasheet). Every number in this module is derived from this spec
/// rather than restated, so a spec correction propagates.
pub const PANEL: DisplaySpec = eink_specs::displays::GDEM0397T81P;

/// Panel current while a refresh waveform is running, mA — ~34 mA average per
/// the panel's entry in `AGENTS.md` ("active refresh: ~34 mA average"; deep
/// sleep ~1 µA, which is why *not* refreshing is nearly free).
///
/// Applied to partial refreshes as well as full ones, which is **conservative**:
/// a partial waveform drives fewer source lines and should draw less, but no
/// per-region figure is published. Replace with a bench measurement on silicon
/// (the Phase-E checklist's step 4).
pub const ACTIVE_REFRESH_MA: f32 = 34.0;

/// How many partial refreshes may accumulate before a full refresh must clear
/// the ghosting they leave.
///
/// **Derived, not chosen**: each partial adds `PANEL.ghosting_rate_partial`
/// (0.10) of visible ghosting, so `1.0 / 0.10 = 10` partials reach saturation.
/// If the spec's ghosting rate is ever corrected, this follows it
/// ([`max_partials_before_full`] is the function; this is it applied to
/// [`PANEL`]).
pub const MAX_PARTIALS_BEFORE_FULL: u32 = max_partials_before_full(&PANEL);

/// How many partials `spec` tolerates before ghosting saturates:
/// `floor(1 / ghosting_rate_partial)`, floored at 1. A non-positive or
/// nonsensical rate degrades to 1 (full refresh every frame — the safe,
/// expensive answer, not an unbounded one).
#[must_use]
pub const fn max_partials_before_full(spec: &DisplaySpec) -> u32 {
    let rate = spec.ghosting_rate_partial;
    if rate <= 0.0 || rate > 1.0 {
        return 1;
    }
    let n = (1.0 / rate) as u32;
    if n == 0 {
        1
    } else {
        n
    }
}

/// Minimum gap between refreshes driven by a **minor** change (the progress
/// readout, volume, battery) — the throttle that makes the display power budget
/// hold. Content changes ignore it entirely (see the module doc's table).
///
/// # Why 10 s, and what it costs
/// This is the one genuine product tradeoff in the policy: the elapsed-time
/// readout advances in steps this long. The panel's refresh cost makes the
/// tradeoff steep — [`avg_display_ma`] over a 10-partials-then-one-full cycle:
///
/// | interval | modeled display draw |
/// |---|---|
/// | 1 s (the old `RENDER_PERIOD_MS`) | ~19.4 mA |
/// | 5 s | ~3.9 mA |
/// | **10 s** | **~1.9 mA** |
/// | 20 s | ~1.0 mA |
/// | 40 s | ~0.5 mA |
///
/// 10 s keeps a visibly-live readout at a display draw comparable to the DAC's
/// (~7 mA) rather than dominating it. `power::model::DISPLAY_AVG_MA` is set to
/// match; the `display_refresh_policy_fits_the_battery_model` gate in
/// `dap-arch-tests` fails if the two ever drift apart, in either direction.
pub const MIN_PARTIAL_INTERVAL_MS: u64 = 10_000;

/// What changed between the previously-refreshed frame and the one just drawn.
///
/// Deliberately a two-bit summary rather than a screen-specific diff: the
/// `display` slice must not depend on [`crate::screens`] (arch boundary), and the
/// policy only ever needs to know *which side of the full/partial line* a change
/// falls on. The caller — [`App::render`](crate::app::App::render) — classifies
/// its own view fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrameChange {
    /// A materially different frame: a new track, a new screen, a transport
    /// state change — anything a user would read as "the display now shows
    /// something else". Earns a [`Full`](RefreshAction::Full), immediately.
    pub content: bool,
    /// A small in-place update inside an otherwise-unchanged frame: the elapsed
    /// readout ticking, a volume or battery indicator moving. Earns a throttled
    /// [`Partial`](RefreshAction::Partial).
    pub minor: bool,
}

impl FrameChange {
    /// Nothing changed — the frame is identical to the one already on the panel.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            content: false,
            minor: false,
        }
    }

    /// Whether anything changed at all.
    #[must_use]
    pub const fn any(self) -> bool {
        self.content || self.minor
    }
}

/// What the caller should do with the frame it just drew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshAction {
    /// Don't touch the panel: either nothing changed, or the only change was a
    /// minor one and [`MIN_PARTIAL_INTERVAL_MS`] hasn't elapsed. The drawn frame
    /// is simply not committed — e-ink holds the previous image for free.
    None,
    /// Commit just this rectangle via
    /// [`partial_refresh`](super::EinkRefresh::partial_refresh) — fast (300 ms)
    /// and flash-free.
    Partial(Rectangle),
    /// Commit the whole panel via
    /// [`full_refresh`](super::EinkRefresh::full_refresh) — 3 s and flashes, but
    /// resets the ghosting budget.
    Full,
}

impl RefreshAction {
    /// The [`RefreshKind`] this action runs, or `None` for
    /// [`RefreshAction::None`] — the bridge to `eink-ui`'s vocabulary.
    #[must_use]
    pub const fn kind(self) -> Option<RefreshKind> {
        match self {
            Self::None => None,
            Self::Partial(_) => Some(RefreshKind::Partial),
            Self::Full => Some(RefreshKind::Full),
        }
    }
}

/// The policy's state: how much ghosting has accumulated and when the panel was
/// last refreshed. Owned by whoever renders (see
/// [`App`](crate::app::App)), driven by [`decide`](Self::decide) once per drawn
/// frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefreshDecider {
    partials_since_full: u32,
    last_refresh_ms: Option<u64>,
}

impl RefreshDecider {
    /// A decider that has never refreshed — its first [`decide`](Self::decide)
    /// returns [`RefreshAction::Full`] whatever the change, since there is no
    /// prior image to partially update.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            partials_since_full: 0,
            last_refresh_ms: None,
        }
    }

    /// Partial refreshes since the last full one — the ghosting accumulator,
    /// bounded by [`MAX_PARTIALS_BEFORE_FULL`].
    #[must_use]
    pub const fn partials_since_full(&self) -> u32 {
        self.partials_since_full
    }

    /// `now_ms` of the last refresh actually performed, or `None` if the panel
    /// has never been refreshed.
    #[must_use]
    pub const fn last_refresh_ms(&self) -> Option<u64> {
        self.last_refresh_ms
    }

    /// Modeled ghosting currently on the panel, `0.0..=1.0` — the accumulator
    /// times the panel's per-partial rate. Reaching `1.0` is what forces a full
    /// refresh.
    #[must_use]
    pub fn ghosting(&self) -> f32 {
        (self.partials_since_full as f32 * PANEL.ghosting_rate_partial).min(1.0)
    }

    /// Decide what to do with a frame drawn at `now_ms`, given what changed and
    /// the `dirty` rectangle the draw touched (`None` = the tracker recorded no
    /// pixels). Updates the decider's state to match the action returned, so a
    /// caller that performs the returned action keeps the accumulator honest.
    ///
    /// See the module doc's table for the rules.
    pub fn decide(
        &mut self,
        now_ms: u64,
        change: FrameChange,
        dirty: Option<Rectangle>,
    ) -> RefreshAction {
        // Nothing on the panel yet: a partial update has no base image to
        // update, so the first commit must be full regardless of `change`.
        if self.last_refresh_ms.is_none() {
            return self.commit_full(now_ms);
        }
        if !change.any() {
            return RefreshAction::None;
        }
        // A materially different frame is also the cheapest moment to clear
        // ghosting: the panel is flashing anyway.
        if change.content {
            return self.commit_full(now_ms);
        }
        // Ghosting saturated — this minor change pays for the reset. Note this
        // is checked BEFORE the throttle: a due full refresh is not something to
        // defer, and it resets the accumulator for the next 10 partials.
        if self.partials_since_full >= MAX_PARTIALS_BEFORE_FULL {
            return self.commit_full(now_ms);
        }
        // Minor-only change: rate-limit it. This is the clause that turns a
        // once-per-second progress tick into a sustainable refresh load.
        let elapsed = now_ms.saturating_sub(self.last_refresh_ms.unwrap_or(now_ms));
        if elapsed < MIN_PARTIAL_INTERVAL_MS {
            return RefreshAction::None;
        }
        match dirty {
            // Something changed but no pixels were drawn: nothing to push.
            None => RefreshAction::None,
            Some(area) => {
                self.partials_since_full = self.partials_since_full.saturating_add(1);
                self.last_refresh_ms = Some(now_ms);
                RefreshAction::Partial(area)
            }
        }
    }

    fn commit_full(&mut self, now_ms: u64) -> RefreshAction {
        self.partials_since_full = 0;
        self.last_refresh_ms = Some(now_ms);
        RefreshAction::Full
    }
}

/// Modeled average panel current, mA, for a steady stream of minor changes
/// refreshing every `interval_ms`, with one full refresh per
/// [`MAX_PARTIALS_BEFORE_FULL`] partials (the ghosting cycle this policy runs).
///
/// `(partials x partial_ms + 1 x full_ms) / (cycle refreshes x interval_ms) x
/// ACTIVE_REFRESH_MA` — i.e. the duty cycle of the refresh waveform times its
/// current. Clamped to [`ACTIVE_REFRESH_MA`]: a duty cycle above 1 means the
/// panel cannot keep up (which is exactly what the old
/// always-full-every-second loop asked for), and the honest answer there is
/// "continuously refreshing", not a number above the active current.
///
/// `interval_ms == 0` degrades to [`ACTIVE_REFRESH_MA`] for the same reason.
#[must_use]
pub fn avg_display_ma(interval_ms: u64, partials_per_full: u32) -> f32 {
    if interval_ms == 0 {
        return ACTIVE_REFRESH_MA;
    }
    let refreshes = u64::from(partials_per_full) + 1;
    let active_ms = u64::from(partials_per_full) * u64::from(PANEL.partial_refresh_ms)
        + u64::from(PANEL.full_refresh_ms);
    let cycle_ms = refreshes * interval_ms;
    let duty = (active_ms as f32 / cycle_ms as f32).min(1.0);
    duty * ACTIVE_REFRESH_MA
}

/// The modeled panel current of the policy as configured — [`avg_display_ma`] at
/// [`MIN_PARTIAL_INTERVAL_MS`] and [`MAX_PARTIALS_BEFORE_FULL`]. This is the
/// number `power::model::DISPLAY_AVG_MA` must cover; the seam is gated in
/// `dap-arch-tests`.
#[must_use]
pub fn configured_avg_display_ma() -> f32 {
    avg_display_ma(MIN_PARTIAL_INTERVAL_MS, MAX_PARTIALS_BEFORE_FULL)
}

/// The shortest minor-change interval whose modeled draw stays within
/// `budget_ma` — the inverse of [`avg_display_ma`], for choosing a cadence from a
/// power budget instead of the other way round. `budget_ma <= 0` yields
/// [`u64::MAX`] (no interval is short enough).
#[must_use]
pub fn min_interval_ms_for_budget_ma(budget_ma: f32, partials_per_full: u32) -> u64 {
    if budget_ma <= 0.0 {
        return u64::MAX;
    }
    let refreshes = u64::from(partials_per_full) + 1;
    let active_ms = u64::from(partials_per_full) * u64::from(PANEL.partial_refresh_ms)
        + u64::from(PANEL.full_refresh_ms);
    // duty = active / (refreshes * interval) <= budget / ACTIVE
    //   =>  interval >= active * ACTIVE / (budget * refreshes)
    let needed = active_ms as f32 * ACTIVE_REFRESH_MA / (budget_ma * refreshes as f32);
    // `f32::ceil` is std-only and this crate is `no_std` — same libm intrinsic
    // `power::model`'s regulator math uses for `sqrt`.
    libm::ceilf(needed) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::geometry::{Point, Size};

    const AREA: Rectangle = Rectangle::new(Point::new(10, 20), Size::new(100, 16));

    fn minor() -> FrameChange {
        FrameChange {
            content: false,
            minor: true,
        }
    }

    fn content() -> FrameChange {
        FrameChange {
            content: true,
            minor: false,
        }
    }

    /// Drive the decider past its mandatory first full refresh.
    fn primed() -> (RefreshDecider, u64) {
        let mut d = RefreshDecider::new();
        assert_eq!(d.decide(0, content(), Some(AREA)), RefreshAction::Full);
        (d, 0)
    }

    // --- the roster's gate ----------------------------------------------

    /// **The gate the roster asked for**: a 1-field change (the progress readout
    /// ticking) must never cost a full-panel flash. Driven for a simulated hour
    /// of playback at the old 1 s render cadence — the regime the shipped loop
    /// actually ran — asserting that every full refresh in that hour is a
    /// ghosting reset, never a reaction to a minor change.
    #[test]
    fn a_minor_change_never_triggers_a_full_refresh() {
        let (mut d, _) = primed();
        let mut fulls = 0;
        let mut partials = 0;
        for tick in 1..=3600u64 {
            let before = d.partials_since_full();
            match d.decide(tick * 1_000, minor(), Some(AREA)) {
                RefreshAction::Full => {
                    fulls += 1;
                    assert_eq!(
                        before, MAX_PARTIALS_BEFORE_FULL,
                        "a full refresh fired at {before} partials — a minor change may only \
                         force one once the ghosting budget is spent"
                    );
                }
                RefreshAction::Partial(_) => partials += 1,
                RefreshAction::None => {}
            }
        }
        // Sanity: the hour did real work, and overwhelmingly partials.
        assert!(
            partials > 300,
            "expected ~327 partials in an hour, got {partials}"
        );
        assert!(
            partials > fulls * 8,
            "expected partials to dominate: {partials} partial vs {fulls} full"
        );
    }

    /// The first frame must be full — there is no prior image to update — even
    /// though it arrives as a content change that would be full anyway. Asserted
    /// for a *minor*-only first frame too, where the general rule would say
    /// partial-or-throttled.
    #[test]
    fn the_first_frame_is_always_full() {
        let mut d = RefreshDecider::new();
        assert_eq!(d.decide(0, minor(), Some(AREA)), RefreshAction::Full);
        assert_eq!(d.partials_since_full(), 0);

        let mut d = RefreshDecider::new();
        assert_eq!(
            d.decide(0, FrameChange::none(), None),
            RefreshAction::Full,
            "even a no-change first frame must be committed — the panel is blank"
        );
    }

    /// An unchanged frame costs nothing. Without this the run loop would pay a
    /// refresh waveform per tick just to redraw identical pixels — the panel
    /// holds its image for free (~1 µA), so the right answer is to do nothing.
    #[test]
    fn an_unchanged_frame_refreshes_nothing() {
        let (mut d, _) = primed();
        for tick in 1..100u64 {
            assert_eq!(
                d.decide(tick * 60_000, FrameChange::none(), Some(AREA)),
                RefreshAction::None,
                "an identical frame must never touch the panel, however long it has been"
            );
        }
        assert_eq!(d.partials_since_full(), 0);
    }

    // --- the two asymmetries --------------------------------------------

    /// A content change refreshes *immediately*, ignoring the throttle — a user
    /// who presses play must not wait up to 10 s for the screen to agree. This
    /// is the asymmetry that makes the throttle acceptable.
    #[test]
    fn a_content_change_bypasses_the_throttle() {
        let (mut d, _) = primed();
        // 1 ms later — far inside MIN_PARTIAL_INTERVAL_MS.
        assert_eq!(d.decide(1, content(), Some(AREA)), RefreshAction::Full);
        assert_eq!(d.decide(2, content(), Some(AREA)), RefreshAction::Full);
        // ...whereas a minor change at the same instant is throttled.
        assert_eq!(d.decide(3, minor(), Some(AREA)), RefreshAction::None);
    }

    /// A minor change is throttled to [`MIN_PARTIAL_INTERVAL_MS`], and the
    /// boundary is inclusive (exactly at the interval, it goes).
    #[test]
    fn a_minor_change_is_rate_limited_to_the_interval() {
        let (mut d, _) = primed();
        assert_eq!(
            d.decide(MIN_PARTIAL_INTERVAL_MS - 1, minor(), Some(AREA)),
            RefreshAction::None,
            "one ms short of the interval must not refresh"
        );
        assert_eq!(
            d.decide(MIN_PARTIAL_INTERVAL_MS, minor(), Some(AREA)),
            RefreshAction::Partial(AREA),
            "exactly at the interval must refresh"
        );
        // And the clock restarts from the refresh that was actually performed.
        assert_eq!(
            d.decide(MIN_PARTIAL_INTERVAL_MS + 1, minor(), Some(AREA)),
            RefreshAction::None
        );
    }

    /// A change with no drawn pixels pushes nothing: the dirty region is what a
    /// partial refresh *is*, so there is nothing to send.
    #[test]
    fn a_change_that_drew_no_pixels_refreshes_nothing() {
        let (mut d, _) = primed();
        assert_eq!(
            d.decide(MIN_PARTIAL_INTERVAL_MS * 4, minor(), None),
            RefreshAction::None
        );
        assert_eq!(d.partials_since_full(), 0, "and costs no ghosting budget");
    }

    // --- the ghosting bound ---------------------------------------------

    /// Ghosting is bounded: the accumulator never exceeds
    /// [`MAX_PARTIALS_BEFORE_FULL`], modeled ghosting never exceeds 1.0, and a
    /// full refresh resets both. Driven over many cycles so a
    /// reset-that-doesn't-reset would show up.
    #[test]
    fn ghosting_stays_bounded_and_a_full_refresh_clears_it() {
        let (mut d, _) = primed();
        let mut t = 0u64;
        let mut saw_reset = 0;
        for _ in 0..(MAX_PARTIALS_BEFORE_FULL * 5 + 5) {
            t += MIN_PARTIAL_INTERVAL_MS;
            let action = d.decide(t, minor(), Some(AREA));
            assert!(
                d.partials_since_full() <= MAX_PARTIALS_BEFORE_FULL,
                "ghosting accumulator {} exceeded the bound {MAX_PARTIALS_BEFORE_FULL}",
                d.partials_since_full()
            );
            assert!(d.ghosting() <= 1.0);
            if action == RefreshAction::Full {
                saw_reset += 1;
                assert_eq!(
                    d.partials_since_full(),
                    0,
                    "a full refresh must clear ghosting"
                );
                assert_eq!(d.ghosting(), 0.0);
            }
        }
        assert!(
            saw_reset >= 4,
            "expected several ghosting resets, saw {saw_reset}"
        );
    }

    /// The ghosting bound is *derived* from the panel spec, not chosen: 10
    /// partials at the GDEM0397T81P's 0.10 rate. A spec correction must move it.
    #[test]
    fn the_ghosting_bound_comes_from_the_panel_spec() {
        assert!((PANEL.ghosting_rate_partial - 0.10).abs() < 1e-6);
        assert_eq!(MAX_PARTIALS_BEFORE_FULL, 10);
        // A hypothetically worse panel tolerates fewer partials...
        let ghosty = DisplaySpec {
            ghosting_rate_partial: 0.5,
            ..PANEL
        };
        assert_eq!(max_partials_before_full(&ghosty), 2);
        // ...and a degenerate rate degrades to "full every time", not unbounded.
        for rate in [0.0, -1.0, 2.0] {
            let bad = DisplaySpec {
                ghosting_rate_partial: rate,
                ..PANEL
            };
            assert_eq!(
                max_partials_before_full(&bad),
                1,
                "rate {rate} must degrade safely"
            );
        }
    }

    // --- the energy model -----------------------------------------------

    /// The policy's whole justification, in numbers: the old behaviour
    /// (full refresh every 1 s render) is a *saturated* panel — duty cycle
    /// pinned at 1.0, ~34 mA continuous — while the configured policy is an
    /// order of magnitude cheaper. Pins the ratio so a cadence or ghosting
    /// change that quietly gives the win back fails here.
    #[test]
    fn the_policy_is_an_order_of_magnitude_cheaper_than_always_full() {
        // The old loop: a 3000 ms full refresh demanded every 1000 ms.
        let always_full_1s = avg_display_ma(1_000, 0);
        assert!(
            (always_full_1s - ACTIVE_REFRESH_MA).abs() < 1e-3,
            "a 3 s refresh every 1 s saturates the panel: {always_full_1s:.1} mA"
        );

        let configured = configured_avg_display_ma();
        assert!(
            configured * 10.0 < always_full_1s,
            "configured policy {configured:.2} mA should be >10x cheaper than the old \
             {always_full_1s:.1} mA"
        );
        // And the absolute figure is the ~1.9 mA the MIN_PARTIAL_INTERVAL_MS doc
        // tabulates — if this moves, that table (and the battery model) are stale.
        assert!(
            (1.5..2.5).contains(&configured),
            "configured display draw {configured:.2} mA left the documented ~1.9 mA band"
        );
    }

    /// `avg_display_ma` behaves monotonically in the cadence, and
    /// `min_interval_ms_for_budget_ma` really inverts it — so a future change can
    /// pick a cadence from a power budget and trust the answer.
    #[test]
    fn the_cadence_and_budget_functions_are_consistent_inverses() {
        let mut prev = f32::INFINITY;
        for interval in [1_000u64, 5_000, 10_000, 20_000, 40_000, 80_000] {
            let ma = avg_display_ma(interval, MAX_PARTIALS_BEFORE_FULL);
            assert!(
                ma < prev,
                "a longer interval must draw less: {interval} ms -> {ma:.2} mA"
            );
            prev = ma;
        }
        for budget in [0.5f32, 1.0, 2.0, 5.0] {
            let interval = min_interval_ms_for_budget_ma(budget, MAX_PARTIALS_BEFORE_FULL);
            let actual = avg_display_ma(interval, MAX_PARTIALS_BEFORE_FULL);
            assert!(
                actual <= budget,
                "{budget} mA budget -> {interval} ms interval, which actually draws \
                 {actual:.3} mA"
            );
            // And it is the *shortest* such interval (one ms less overruns).
            let shorter = avg_display_ma(interval.saturating_sub(1), MAX_PARTIALS_BEFORE_FULL);
            assert!(
                shorter > budget || interval <= 1,
                "{interval} ms is not the tightest interval for {budget} mA"
            );
        }
        // A zero/negative budget admits no cadence rather than returning 0.
        assert_eq!(min_interval_ms_for_budget_ma(0.0, 10), u64::MAX);
    }
}
