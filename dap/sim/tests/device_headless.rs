//! Headless, hardware-free proof that the whole virtual DAP -- scripted
//! wheel/button input driving transport and volume, soul-audio-core doing
//! the real decode/resample/gapless, and a scripted battery governing power
//! behavior -- all run correctly together through `dap_core::app::App`,
//! zero hardware.
//!
//! One scripted scenario, driven by a single simulated clock `t`:
//! 1. `Press` -> playback starts, real audio reaches `SimSink`.
//! 2. `Rotate` down (App starts at full volume, so there's first room to
//!    make before a rise is observable) then `Rotate` up -> the recorded
//!    level measurably rises.
//! 3. `LongPress` -> the queue advances to track 2, gaplessly (no long
//!    silent run in the recorded stream around the swap).
//! 4. The scripted `SimBattery` discharges: crossing the low-battery
//!    threshold auto-pauses playback; crossing critical stops it and
//!    records exactly one `request_power_off` call.
//!
//! `App` owns its `Power`/`Controls` by value with no mutable accessor back
//! out, so both `SimBattery` and `SimControls` are driven through a small
//! `Arc<Mutex<_>>` wrapper (`Shared`) -- the same role `dap_core::app`'s own
//! tests give their `Arc<Mutex<_>>`-wrapped `MockPower`/`MockControls`
//! doubles, and for the same two reasons: the test needs to keep setting
//! each double's simulated clock between `App::tick` calls, and (since this
//! test runs its body on a dedicated big-stack thread, same as
//! `audio_headless.rs`/`power_headless.rs`) the fixture must stay `Send`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use dap_core::app::{App, AppConfig, TickOutcome};
use dap_core::ports::{BatteryReading, Controls, InputEvent, Power};
use dap_core::power::PowerProfile;
use dap_sim::{BatteryKeyframe, SimBattery, SimControls, SimDisplay, SimSink, SimStorage};
use soul_audio_core::{DeckState, Progress};

// 1 = gapless (one preloaded next track, all gapless playback needs);
// PRELOAD = 2 was a leftover desktop value.
const PRELOAD: usize = 1;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

/// Shared handle to a value moved into `App` by value -- see the module
/// docs for why this exists.
struct Shared<T>(Arc<Mutex<T>>);

impl<T> Shared<T> {
    fn new(inner: T) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    fn lock(&self) -> MutexGuard<'_, T> {
        self.0.lock().expect("lock poisoned")
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl Controls for Shared<SimControls> {
    fn poll(&mut self) -> Option<InputEvent> {
        self.lock().poll()
    }
}

impl Power for Shared<SimBattery> {
    fn read(&mut self) -> BatteryReading {
        self.lock().read()
    }
    fn set_charge_current_ma(&mut self, ma: u16) {
        self.lock().set_charge_current_ma(ma);
    }
    fn set_clock_profile(&mut self, profile: PowerProfile) {
        self.lock().set_clock_profile(profile);
    }
    fn request_power_off(&mut self) {
        self.lock().request_power_off();
    }
}

type Dap = App<SimSink, SimStorage, Shared<SimBattery>, Shared<SimControls>, SimDisplay, PRELOAD>;

/// Mean absolute sample value -- a level proxy, same helper as
/// `audio_headless.rs`'s volume check.
fn mean_abs(samples: &[i32]) -> f64 {
    samples.iter().map(|&s| (s as f64).abs()).sum::<f64>() / samples.len() as f64
}

/// Longest run of frames (across the whole recorded stream so far) whose
/// samples on every channel are exactly zero -- same helper and rationale as
/// `audio_headless.rs`: a real gapless-boundary bug (silence inserted at a
/// track swap) shows up as a long run; the fixtures' own fades only ever
/// touch exact zero at a handful of edge frames.
fn longest_silent_run(samples: &[i32], channels: usize) -> usize {
    let mut longest = 0usize;
    let mut current = 0usize;
    for frame in samples.chunks_exact(channels) {
        if frame.iter().all(|&s| s == 0) {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// Drive both scripted doubles' simulated clocks to `*t`, pump one `App`
/// tick, keep the queue looping on natural end so a long battery-discharge
/// phase always has something to pause/stop (mirrors `power_headless.rs`'s
/// `pump_keep_playing` -- safe unconditionally here, since `App::tick`
/// always pauses/stops *before* it pumps, so a paused/stopped deck's pump
/// never itself reports `Ended`), then advance `*t` by `step_ms`.
fn tick(
    app: &mut Dap,
    controls: &Shared<SimControls>,
    battery: &Shared<SimBattery>,
    t: &mut u64,
    step_ms: u64,
) -> TickOutcome {
    controls.lock().set_now_ms(*t);
    battery.lock().set_now_ms(*t);
    let outcome = app.tick(*t);
    if let Ok(Progress::Ended) = outcome.pump {
        app.player_mut().goto(0).expect("goto 0");
        app.player_mut().play();
    }
    *t += step_ms;
    outcome
}

#[test]
fn virtual_dap_headless() {
    // Generous worker stack -- same reason as `audio_headless.rs`/
    // `power_headless.rs`/`dap_core::app`'s own tests: `Deck`/`Engine` embed
    // `AnyDecoder` by value across several unoptimized (debug) call frames
    // inside `AudioPlayer::pump`, which every `App::tick` call drives.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}

fn run() {
    let sink = SimSink::new();
    let storage = SimStorage::new([tone_path(), antiphase_path()]);

    // Scripted input: Press at t=0 (start playback), then lower volume
    // (App starts at full scale -- see `AppConfig`/`App::new` docs -- so
    // there is no room to show a *rise* without first making room for one),
    // then raise it back up, then skip to track 2. Times line up exactly
    // with the tick-count phase boundaries below (step_ms=20, 30/40/40
    // ticks/phase -> t=0, 600, 1_400, 2_200).
    let controls = Shared::new(SimControls::new([
        (0, InputEvent::Press),
        (600, InputEvent::Rotate(-16)), // 1.0 -> 0.2 (-16 * 0.05 volume_step)
        (1_400, InputEvent::Rotate(8)), // 0.2 -> 0.6
        (2_200, InputEvent::LongPress),
    ]));

    // Healthy and flat through every scripted-input phase above (that phase
    // finishes by t=2_800, comfortably under this hold's 5_000ms), then
    // discharges to empty over the next 200s -- long enough for
    // `PowerService`'s per-poll EMA to settle past both the low-battery
    // (15%) and critical (3%) thresholds, exactly like `power_headless.rs`'s
    // own discharge scenario.
    let battery = Shared::new(SimBattery::new([
        BatteryKeyframe::new(0, 900, 2_500, false),
        BatteryKeyframe::new(5_000, 900, 2_500, false),
        BatteryKeyframe::new(205_000, 0, 2_500, false),
    ]));

    let mut app: Dap = App::new(
        sink,
        storage,
        battery.clone(),
        controls.clone(),
        SimDisplay::new(),
        AppConfig::default(),
    );
    app.set_queue(2, 0);

    let mut t = 0u64;
    let step_ms = 20u64;

    // --- Phase 1: Press starts playback -> real audio reaches SimSink --
    // 30 ticks * 256 frames/tick = 7_680 device-rate frames -- clear of
    // tone.wav's ~50ms (2_400-frame @48kHz) fade-in before anything below
    // measures level.
    for _ in 0..30 {
        tick(&mut app, &controls, &battery, &mut t, step_ms);
    }
    assert_eq!(
        app.player().current(),
        Some(0),
        "queue must start on track 0"
    );
    assert_eq!(app.state(), DeckState::Playing, "Press must start playback");
    assert!(
        !app.player().sink().recorded().is_empty(),
        "Press must result in real audio reaching SimSink"
    );

    // --- Phase 2: Rotate(-16) lowers volume -> record a quiet baseline ---
    let quiet_start = app.player().sink().recorded().len();
    for _ in 0..40 {
        tick(&mut app, &controls, &battery, &mut t, step_ms);
    }
    let quiet_end = app.player().sink().recorded().len();
    assert!(
        (0.15f32..0.25f32).contains(&app.volume()),
        "Rotate(-16) must have lowered volume to ~0.2, got {}",
        app.volume()
    );

    // --- Phase 3: Rotate(8) raises volume -> recorded level must rise ---
    for _ in 0..40 {
        tick(&mut app, &controls, &battery, &mut t, step_ms);
    }
    let raised_end = app.player().sink().recorded().len();
    assert!(
        (0.55f32..0.65f32).contains(&app.volume()),
        "Rotate(8) must have raised volume to ~0.6, got {}",
        app.volume()
    );

    let channels = app.player().sink().channels();
    {
        let recorded = app.player().sink().recorded();
        let quiet_level = mean_abs(&recorded[quiet_start..quiet_end]);
        let raised_level = mean_abs(&recorded[quiet_end..raised_end]);
        assert!(quiet_level > 0.0, "sanity: quiet window isn't silent");
        assert!(
            raised_level > quiet_level * 2.0,
            "Rotate(+) must raise the recorded level: quiet={quiet_level}, \
             raised={raised_level} (volume 0.2 -> 0.6, expect ~3x)"
        );
    }

    // --- Phase 4: LongPress advances to track 2, gaplessly -------------
    for _ in 0..30 {
        tick(&mut app, &controls, &battery, &mut t, step_ms);
    }
    assert_eq!(
        app.player().current(),
        Some(1),
        "LongPress must advance the queue to track 2"
    );
    let gap = longest_silent_run(app.player().sink().recorded(), channels);
    assert!(
        gap <= 1_024,
        "found a {gap}-frame all-zero run in the recorded stream -- a gapless \
         track-2 swap must not insert an audible silence"
    );

    // --- Phase 5/6: discharge to low (auto-pause) then critical (stop) -
    let mut low_fired_at: Option<u64> = None;
    let mut critical_fired_at: Option<u64> = None;
    let mut frames_at_pause: Option<u64> = None;
    while t <= 205_000 {
        let outcome = tick(&mut app, &controls, &battery, &mut t, 250);

        if outcome.low_battery && low_fired_at.is_none() {
            low_fired_at = Some(t);
            assert_eq!(
                app.state(),
                DeckState::Paused,
                "LowBattery must pause an already-playing deck"
            );
            frames_at_pause = Some(app.player().position_frames());
        }

        if let Some(pos) = frames_at_pause {
            if critical_fired_at.is_none() && !outcome.critical_shutdown {
                assert_eq!(
                    app.player().position_frames(),
                    pos,
                    "player must not advance while paused by LowBattery"
                );
            }
        }

        if outcome.critical_shutdown && critical_fired_at.is_none() {
            critical_fired_at = Some(t);
            assert_eq!(
                app.state(),
                DeckState::Stopped,
                "CriticalShutdown must stop the deck"
            );
            break;
        }
    }

    let low_fired_at = low_fired_at.expect("LowBattery must fire within the discharge loop budget");
    let critical_fired_at =
        critical_fired_at.expect("CriticalShutdown must fire within the discharge loop budget");
    assert!(
        low_fired_at < critical_fired_at,
        "warn must precede critical"
    );
    assert_eq!(
        battery.lock().power_off_calls(),
        1,
        "SimBattery must have recorded exactly one request_power_off call"
    );
}
