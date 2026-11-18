//! Proves `dap_core::firmware::step` — the ONE run-loop function this
//! repo's "test what you ship" principle depends on — is driven here on the
//! host exactly as `board-h7a3`'s embassy task drives it (see
//! `board-h7a3/src/main.rs`'s `loop { firmware::step(app, now_ms, &mut
//! last_render_ms); ... }`), not a re-derived/duplicated loop.
//!
//! Two scenarios:
//! - [`firmware_step_runs_the_boards_stub_wiring_without_panicking`] builds
//!   the exact `App<StubSink, StubStorage, _, StubPower, StubControls,
//!   StubDisplay, 0>` instantiation `board-h7a3` wires up (see
//!   `dap-core/src/stub.rs`) and drives it through `step` for a few hundred
//!   iterations: no hardware, no real media, no real input — this is the
//!   literal code the board links and runs, proving it doesn't panic and
//!   that the render cadence fires.
//! - [`firmware_step_pumps_real_audio_and_refreshes_the_display`] swaps in
//!   `SimSink`/`SimStorage`/`SimControls`/`SimDisplay` (real fixtures, real
//!   decode) to prove `step`'s render policy and tick-draining are correct
//!   against genuine playback, not just against a wiring that never
//!   produces anything — the stronger "audio pumps, display refreshes"
//!   proof the stub-only scenario above can't give on its own (`StubStorage`
//!   never successfully opens anything, by design — see its doc comment).

use std::path::{Path, PathBuf};

use dap_core::app::{App, AppConfig};
use dap_core::firmware;
use dap_core::ports::InputEvent;
use dap_core::stub::{StubControls, StubDisplay, StubPower, StubSink, StubStorage};
use dap_sim::{sac_scratch, SimControls, SimDisplay, SimSink, SimStorage};
use soul_audio_core::DeckState;

// 0 = single resident decoder; gapless via the output ring at track reopen
// (Rockbox model) -- matches every other fixture-backed test in this crate
// and the real value `board-h7a3` wires up.
const PRELOAD: usize = 0;

/// The literal type `board-h7a3/src/main.rs` constructs (modulo `Pcm`: the
/// board uses a `StaticCell`-backed `&'static mut [i32]`; this test uses a
/// plain heap `Box<[i32]>` via `dap_sim::sac_scratch`, which is never
/// actually called anyway -- see `StubStorage`'s doc comment for why).
type StubDap =
    App<StubSink, StubStorage, Box<[i32]>, StubPower, StubControls, StubDisplay, PRELOAD>;

/// Real audio/display/input (`Sim*`), paired with `StubPower`'s fixed
/// healthy reading so this scenario isolates "does audio pump and does the
/// display refresh through `step`" from power-event behavior entirely
/// (already covered by `tests/power_headless.rs`/`tests/device_headless.rs`).
type SimDap = App<SimSink, SimStorage, Box<[i32]>, StubPower, SimControls, SimDisplay, PRELOAD>;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

/// Same big-stack-worker convention as this crate's other `AudioPlayer`/
/// `App`-driving tests (`audio_headless.rs`, `device_headless.rs`,
/// `ui_headless.rs`, `static_placement.rs`): `Deck`/`Engine` embed
/// `AnyDecoder` by value across several unoptimized (debug) call frames,
/// which `App::tick` (called by `firmware::step`) drives every call.
fn run_on_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}

#[test]
fn firmware_step_runs_the_boards_stub_wiring_without_panicking() {
    run_on_big_stack(|| {
        let mut app: StubDap = App::new(
            StubSink,
            StubStorage,
            StubPower,
            StubControls,
            StubDisplay,
            AppConfig::default(),
            sac_scratch,
        );
        // Mirrors `board-h7a3/src/main.rs`'s `app.set_queue(1, 0)` -- one
        // "track" queued against storage that always reports `NoMedia`.
        app.set_queue(1, 0);

        const STEPS: u64 = 300;
        const PERIOD_MS: u64 = 20;
        let mut last_render_ms: Option<u64> = None;
        let mut renders = 0u32;

        for i in 0..STEPS {
            let now_ms = i * PERIOD_MS;
            let outcome = firmware::step(&mut app, now_ms, &mut last_render_ms);

            if i == 0 {
                // The very first pump attempts to open track 0, which
                // `StubStorage` always fails -- `App::tick` must surface
                // that as an error, not fabricate progress or panic.
                assert!(
                    outcome.tick.pump.is_err(),
                    "the first pump must surface StubStorage's NoMedia error"
                );
            }

            if let Some(render) = outcome.render {
                renders += 1;
                assert!(render.is_ok(), "StubDisplay must never fail a refresh");
            }
        }

        assert!(
            renders >= 1,
            "the periodic render cadence (dap_core::firmware::RENDER_PERIOD_MS) \
             must have fired at least once over {STEPS} steps at {PERIOD_MS} ms/step"
        );
        assert_ne!(
            app.state(),
            DeckState::Playing,
            "nothing can ever load from a storage that always reports NoMedia -- \
             the deck must never report Playing"
        );
    });
}

#[test]
fn firmware_step_pumps_real_audio_and_refreshes_the_display() {
    run_on_big_stack(|| {
        let sink = SimSink::new();
        let storage = SimStorage::new([tone_path(), antiphase_path()]);
        // Press at t=0 -- starts playback on the very first drained event,
        // same convention as `tests/ui_headless.rs`.
        let controls = SimControls::new([(0, InputEvent::Press)]);
        let display = SimDisplay::new();

        let mut app: SimDap = App::new(
            sink,
            storage,
            StubPower,
            controls,
            display,
            AppConfig::default(),
            sac_scratch,
        );
        app.set_queue(2, 0);

        const STEPS: u64 = 60;
        const PERIOD_MS: u64 = 20;
        let mut last_render_ms: Option<u64> = None;
        let mut renders = 0u32;

        for i in 0..STEPS {
            let now_ms = i * PERIOD_MS;
            let outcome = firmware::step(&mut app, now_ms, &mut last_render_ms);
            assert!(
                outcome.tick.pump.is_ok(),
                "pump must succeed against real fixtures at t={now_ms}"
            );
            if let Some(render) = outcome.render {
                renders += 1;
                assert!(render.is_ok(), "SimDisplay must not fail a refresh");
            }
        }

        assert_eq!(
            app.state(),
            DeckState::Playing,
            "the scripted Press must have started playback"
        );
        assert!(
            !app.player().sink().recorded().is_empty(),
            "real audio must have reached SimSink via firmware::step"
        );
        assert!(
            renders >= 1,
            "firmware::step must have rendered at least once over {STEPS} steps"
        );
        assert!(
            app.display().full_refresh_count() >= 1,
            "at least one of firmware::step's renders must have committed a real \
             full_refresh to the display"
        );
    });
}
