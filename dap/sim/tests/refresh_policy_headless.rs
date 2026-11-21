//! Headless, hardware-free proof that the **shipped run loop** refreshes the
//! e-ink panel sparingly — driving the real `dap_core::firmware::step` (the one
//! loop `board-h7a3` runs) over simulated playback and asserting on the refresh
//! calls `SimDisplay` counts.
//!
//! `dap_core::display::refresh`'s own unit tests prove the *policy* in
//! isolation; these prove the *wiring*. The distinction matters here, because
//! the bug this guards against was entirely in the wiring: the policy didn't
//! exist, and `App::render` called `full_refresh()` unconditionally while
//! `firmware::step` called `render` every second. On this panel a full refresh
//! is 3 s at ~34 mA, so the shipped loop asked for a continuous panel flash —
//! and no test noticed, because every test asserted `render().is_ok()` rather
//! than what it cost.

use std::path::{Path, PathBuf};

use dap_core::app::{App, AppConfig};
use dap_core::display::refresh::{
    RefreshAction, MAX_PARTIALS_BEFORE_FULL, MIN_PARTIAL_INTERVAL_MS,
};
use dap_core::firmware;
use dap_core::ports::InputEvent;
use dap_sim::{
    sac_scratch, BatteryKeyframe, SimBattery, SimControls, SimDisplay, SimSink, SimStorage,
};

const PRELOAD: usize = 0;

/// Simulated run length and tick rate. Ten minutes at 20 ms/tick crosses many
/// `RENDER_PERIOD_MS` boundaries, so what's measured is the loop's steady-state
/// refresh appetite rather than a start-up transient.
const RUN_MS: u64 = 10 * 60 * 1_000;
const TICK_MS: u64 = 20;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

type Dap = App<SimSink, SimStorage, Box<[i32]>, SimBattery, SimControls, SimDisplay, PRELOAD>;

fn app_with(controls: SimControls) -> Dap {
    let mut app: Dap = App::new(
        SimSink::new(),
        SimStorage::new([tone_path(), antiphase_path()]),
        SimBattery::new([BatteryKeyframe::new(0, 823, 2_500, false)]),
        controls,
        SimDisplay::new(),
        AppConfig::default(),
        sac_scratch,
    );
    app.set_queue(2, 0);
    app
}

/// Generous worker stack — same reason as every other dap-sim headless test:
/// `Deck`/`Engine` embed `AnyDecoder` by value across several unoptimized debug
/// frames inside `AudioPlayer::pump`.
fn on_worker(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}

#[test]
fn the_shipped_run_loop_never_flashes_the_panel_per_tick() {
    on_worker(run_loop_scenario);
}

fn run_loop_scenario() {
    let mut app = app_with(SimControls::new([(0, InputEvent::Press)]));

    let mut last_render_ms: Option<u64> = None;
    let mut steps = 0u64;
    let mut now_ms = 0;
    while now_ms < RUN_MS {
        let outcome = firmware::step(&mut app, now_ms, &mut last_render_ms);
        if let Some(result) = outcome.render {
            result.expect("SimDisplay must never fail a refresh");
        }
        steps += 1;
        now_ms += TICK_MS;
    }

    let d = app.display();
    let fulls = u64::from(d.full_refresh_count());
    let partials = u64::from(d.partial_refresh_count());
    let minutes = RUN_MS / 60_000;

    println!("{steps} steps over {minutes} min: {fulls} full + {partials} partial refreshes");

    // --- the regression this file exists for -----------------------------
    //
    // The old loop ran one FULL refresh per render, and rendered every second:
    // ~600 full-panel flashes over ten minutes, each a 3 s / ~34 mA waveform.
    // Pin an order-of-magnitude bound rather than an exact count, so the gate
    // survives cadence tuning but not a return to flash-per-tick.
    let old_behaviour_fulls = RUN_MS / 1_000;
    assert!(
        fulls * 20 < old_behaviour_fulls,
        "{fulls} full refreshes over {minutes} min — the old always-full-every-second \
         behaviour would have been ~{old_behaviour_fulls}, and the policy must stay far \
         below it"
    );

    // No refresh may happen more often than the throttle allows, plus slack for
    // content changes (a track change bypasses the throttle by design, and this
    // scenario has two tracks plus the boot frame).
    let throttle_ceiling = RUN_MS / MIN_PARTIAL_INTERVAL_MS + 8;
    let total = fulls + partials;
    assert!(
        total <= throttle_ceiling,
        "{total} refreshes over {minutes} min exceeds the {throttle_ceiling} the \
         {MIN_PARTIAL_INTERVAL_MS} ms throttle should allow (plus content-change slack)"
    );

    // The ghosting contract, observed end-to-end.
    assert!(
        partials <= fulls * u64::from(MAX_PARTIALS_BEFORE_FULL),
        "{partials} partials against {fulls} fulls exceeds the {MAX_PARTIALS_BEFORE_FULL}:1 \
         ghosting bound"
    );

    // The panel really was driven: a policy that never refreshes at all would
    // satisfy every bound above. Only the boot frame and the track changes are
    // guaranteed here — dap-sim's audio fixtures are short clips (a few seconds
    // each), so playback ends early in this ten-minute window and the progress
    // readout stops advancing. That is why the *partial* path is asserted
    // deterministically in the next test rather than inferred from this one.
    assert!(
        fulls >= 1,
        "the first frame must have been committed to a blank panel"
    );
    assert!(
        last_render_ms.is_some(),
        "firmware::step never rendered at all over {steps} steps"
    );
    // (That the redraw cadence never outpaces the commit throttle is a
    // compile-time const gate in dap-arch-tests' `seams.rs`, not an assertion
    // here — both operands are constants.)
}

/// The partial-refresh path, end to end and deterministically: a **minor**
/// change (the volume readout moving) must commit as a `partial_refresh` of the
/// dirty rectangle — never as a full-panel flash — and must respect the
/// throttle. This is the "no full refresh on a 1-field change" property the
/// enforcement roster asks for, asserted against the real `App` + `SimDisplay`
/// wiring rather than against the policy in isolation.
#[test]
fn a_minor_change_commits_as_a_partial_refresh_not_a_full_one() {
    on_worker(minor_change_scenario);
}

fn minor_change_scenario() {
    let mut app = app_with(SimControls::new([(0, InputEvent::Press)]));

    // Warm up until playback is established. This matters: the FIRST tick
    // auto-loads track 0, which changes the title from "No Track" to "Track 1"
    // — a content change, correctly a full refresh. The interesting case is a
    // frame that differs from the panel's in a minor field ONLY, so the title
    // and transport state have to settle first.
    for i in 0..60u64 {
        app.tick(i);
    }
    let baseline_track = app.player().current();
    assert!(
        baseline_track.is_some(),
        "the scripted Press must have started playback"
    );

    // Baseline: commit the settled frame. (Full because the panel is blank.)
    assert_eq!(
        app.render(0).expect("render must succeed"),
        RefreshAction::Full,
        "the first frame must be committed fully — there is no prior image"
    );
    assert_eq!(app.display().full_refresh_count(), 1);
    assert_eq!(app.display().partial_refresh_count(), 0);

    // An identical frame, past the throttle, must still commit nothing.
    assert_eq!(
        app.render(MIN_PARTIAL_INTERVAL_MS).expect("render"),
        RefreshAction::None,
        "nothing changed, so nothing may be pushed"
    );

    // Now advance playback until the elapsed readout moves — one whole second
    // of decoded audio, the canonical "1-field change". `pump` decodes a fixed
    // block per tick, so this is a bounded loop; the guard keeps the test
    // honest if a fixture ever ends mid-way and swaps tracks (which would be a
    // content change, not the case under test).
    let mut action = RefreshAction::None;
    let mut ticks = 0u64;
    while ticks < 1_000 {
        app.tick(ticks);
        ticks += 1;
        assert_eq!(
            app.player().current(),
            baseline_track,
            "the track must not change during this scenario — that would be a content \
             change, not the 1-field change under test (fixture too short?)"
        );
        action = app
            .render(MIN_PARTIAL_INTERVAL_MS * 2)
            .expect("render must succeed");
        if action != RefreshAction::None {
            break;
        }
    }

    // **The roster's property**: that 1-field change committed as a partial
    // refresh, and cost no full-panel flash.
    assert!(
        matches!(action, RefreshAction::Partial(_)),
        "the elapsed readout advancing must commit as a partial refresh, got {action:?} \
         after {ticks} ticks"
    );
    assert_eq!(
        app.display().full_refresh_count(),
        1,
        "a 1-field change must NOT cost a full-panel flash"
    );
    assert_eq!(app.display().partial_refresh_count(), 1);

    // And the driver was handed a real area to push.
    let area = app
        .display()
        .last_partial_refresh_area()
        .expect("partial_refresh must have recorded its area");
    assert!(
        area.size.width > 0 && area.size.height > 0,
        "the partial-refresh area must be non-empty, got {area:?}"
    );

    // Re-rendering an unchanged frame costs nothing at all, however long we wait.
    assert_eq!(
        app.render(MIN_PARTIAL_INTERVAL_MS * 10)
            .expect("render must succeed"),
        RefreshAction::None,
        "an identical frame must never be re-committed"
    );
    assert_eq!(app.display().full_refresh_count(), 1);
    assert_eq!(app.display().partial_refresh_count(), 1);
}
