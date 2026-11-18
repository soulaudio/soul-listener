//! The shared firmware run-loop step — the one piece of scheduling logic
//! both `board-h7a3`'s embassy task loop and a host test (`dap-sim`'s
//! `tests/firmware_step.rs`) drive, so the two never duplicate the loop
//! (see the roadmap's Phase B, "`firmware-core` factoring (same-as-shipped)":
//! "Extract a board-agnostic firmware run-loop so `board-h7a3` (embassy HAL)
//! and the [host/]QEMU test binary share the exact same core + swapped
//! adapters.").
//!
//! [`crate::app::App::tick`] already does the heavy lifting every
//! scheduler period needs unconditionally — drain [`crate::ports::Controls`],
//! poll the [`crate::power::PowerService`], pump the deck one block (see its
//! own doc comment). The one thing `tick` deliberately leaves to its caller
//! is *when* to pay for a [`crate::app::App::render`] call: a full e-ink
//! refresh is comparatively slow (hundreds of ms to a few seconds — see
//! `eink_specs::DisplaySpec::full_refresh_duration`), so running it on every
//! tick unconditionally would be wrong on real hardware. [`step`] is the
//! thin wrapper that adds exactly that one decision on top of `tick`.

use soul_audio_core::{Progress, Sink};

use crate::app::{App, TickOutcome};
use crate::display::{Display, DisplayError};
use crate::ports::{Controls, Power, Storage};

/// How often (in the caller's `now_ms` units) [`step`] forces a display
/// refresh even with no notable [`TickOutcome`] event — keeps an
/// elapsed-time readout from going stale during otherwise-uneventful
/// playback. Deliberately coarse (not a UI-smoothness cadence): see the
/// module docs for why a full refresh is too slow to run every tick.
pub const RENDER_PERIOD_MS: u64 = 1_000;

/// What one [`step`] call did: the wrapped [`App::tick`] outcome, plus
/// whether (and with what result) this step also rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepOutcome {
    /// The wrapped [`App::tick`] call's own outcome.
    pub tick: TickOutcome,
    /// `Some(result)` if this step also called [`App::render`] (see
    /// [`step`]'s render policy below); `None` if it skipped rendering this
    /// period.
    pub render: Option<Result<(), DisplayError>>,
}

/// Drive one firmware scheduler period against `app`: [`App::tick`], then
/// render if either —
/// - this tick did something a user would notice: `play_pause_toggled`,
///   `track_advanced`, a non-zero `volume_delta`, `low_battery`,
///   `critical_shutdown`, or the deck just swapped tracks
///   ([`Progress::TrackChanged`]); or
/// - at least [`RENDER_PERIOD_MS`] has elapsed since `*last_render_ms` (or
///   nothing has been rendered yet, i.e. `*last_render_ms == None`).
///
/// `last_render_ms` is the caller's own plain `Option<u64>` local (seeded
/// `None`), threaded through across calls — this function carries no state
/// of its own, so `board-h7a3`'s embassy task loop and a host test both
/// drive the exact same four lines:
///
/// ```text
/// let mut last_render_ms: Option<u64> = None;
/// loop {
///     let outcome = dap_core::firmware::step(&mut app, now_ms, &mut last_render_ms);
///     // ... log/assert on `outcome` ...
///     now_ms += period_ms;
/// }
/// ```
///
/// This is the ONE run-loop implementation both the board and `dap-sim`'s
/// host tests exercise — see the module docs' "test what you ship"
/// rationale and `dap-sim/tests/firmware_step.rs`.
pub fn step<S, St, Pcm, Pw, C, D, const PRELOAD: usize>(
    app: &mut App<S, St, Pcm, Pw, C, D, PRELOAD>,
    now_ms: u64,
    last_render_ms: &mut Option<u64>,
) -> StepOutcome
where
    S: Sink,
    St: Storage,
    Pcm: AsRef<[i32]> + AsMut<[i32]>,
    Pw: Power,
    C: Controls,
    D: Display,
{
    let tick = app.tick(now_ms);

    let notable = tick.play_pause_toggled
        || tick.track_advanced
        || tick.volume_delta != 0.0
        || tick.low_battery
        || tick.critical_shutdown
        || matches!(tick.pump, Ok(Progress::TrackChanged));
    let due = match *last_render_ms {
        None => true,
        Some(last) => now_ms.saturating_sub(last) >= RENDER_PERIOD_MS,
    };

    let render = if notable || due {
        *last_render_ms = Some(now_ms);
        Some(app.render())
    } else {
        None
    };

    StepOutcome { tick, render }
}
