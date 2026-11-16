//! `SimControls` -- a host test double for `dap_core::ports::Controls`,
//! driven by a scriptable timeline of `(at_ms, InputEvent)` keyframes that
//! `poll()` drains in order as the simulated clock reaches each one.
//!
//! Mirrors `SimBattery`'s shape (a scripted timeline + a settable simulated
//! clock), for the same reason: `Controls::poll` takes no arguments, so a
//! scripted double needs its own clock a test can advance -- there is no
//! other way to tell it "it is now `t`" between `App::tick` calls.

use dap_core::ports::{Controls, InputEvent};

/// A scriptable [`Controls`] test double: a timeline of `(at_ms,
/// InputEvent)` keyframes, sorted ascending by `at_ms` on construction, that
/// [`poll`](Controls::poll) drains in order -- each event is returned
/// exactly once, the first time `poll` is called at or after its `at_ms`
/// (tracked via an internal cursor, so already-returned events are never
/// re-polled even if the clock moves backwards or stalls).
#[derive(Debug, Clone)]
pub struct SimControls {
    /// Sorted ascending by `at_ms`.
    timeline: Vec<(u64, InputEvent)>,
    /// Index of the next not-yet-returned event in `timeline`.
    cursor: usize,
    now_ms: u64,
}

impl SimControls {
    /// Build controls over `timeline` (need not be pre-sorted -- sorted by
    /// `at_ms` on construction). Starts at simulated clock `0`.
    #[must_use]
    pub fn new(timeline: impl IntoIterator<Item = (u64, InputEvent)>) -> Self {
        let mut timeline: Vec<(u64, InputEvent)> = timeline.into_iter().collect();
        timeline.sort_by_key(|(at_ms, _)| *at_ms);
        Self {
            timeline,
            cursor: 0,
            now_ms: 0,
        }
    }

    /// Set the controls' simulated clock. Takes effect on the next
    /// [`poll`](Controls::poll).
    pub fn set_now_ms(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
    }

    /// Advance the controls' simulated clock by `delta_ms`.
    pub fn advance_ms(&mut self, delta_ms: u64) {
        self.now_ms += delta_ms;
    }
}

impl Controls for SimControls {
    fn poll(&mut self) -> Option<InputEvent> {
        let (at_ms, event) = *self.timeline.get(self.cursor)?;
        if at_ms > self.now_ms {
            return None;
        }
        self.cursor += 1;
        Some(event)
    }
}
