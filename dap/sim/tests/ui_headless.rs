//! Headless, hardware-free proof that `dap_core::app::App` renders a real
//! Now-Playing frame: a scripted `Press` starts playback through the real
//! `soul-audio-core` decode path (same as `device_headless.rs`'s own
//! scenario), `App::render` builds a `NowPlayingView` from that live
//! player/power state and draws it onto a [`SimDisplay`], and this test
//! asserts directly on the resulting framebuffer — the "UI renders
//! headless" half of Phase 2a (`audio_headless.rs`/`power_headless.rs`/
//! `device_headless.rs` already cover the audio/power/input half).

use std::path::{Path, PathBuf};

use dap_core::app::{App, AppConfig};
use dap_core::ports::InputEvent;
use dap_sim::{BatteryKeyframe, SimBattery, SimControls, SimDisplay, SimSink, SimStorage};
use soul_audio_core::DeckState;

const PRELOAD: usize = 2;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

type Dap = App<SimSink, SimStorage, SimBattery, SimControls, SimDisplay, PRELOAD>;

#[test]
fn now_playing_renders_a_non_blank_frame_headless() {
    // Generous worker stack -- same reason as `audio_headless.rs`/
    // `power_headless.rs`/`device_headless.rs`/`dap_core::app`'s own
    // tests: `Deck`/`Engine` embed `AnyDecoder` by value across several
    // unoptimized (debug) call frames inside `AudioPlayer::pump`, which
    // every `App::tick` call drives.
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
    // Flat, healthy reading -- this test exercises rendering, not power
    // behavior (that's `power_headless.rs`/`device_headless.rs`'s job), so
    // a single keyframe (no discharge curve) is enough.
    let battery = SimBattery::new([BatteryKeyframe::new(0, 823, 2_500, false)]);
    // Press is scheduled at `at_ms=0`, and `SimControls`'s own simulated
    // clock starts at `0` too, so the very first `poll()` already drains it
    // -- no need to advance its clock from outside (unlike
    // `device_headless.rs`'s longer scripted scenario).
    let controls = SimControls::new([(0, InputEvent::Press)]);
    let display = SimDisplay::new();

    let mut app: Dap = App::new(
        sink,
        storage,
        battery,
        controls,
        display,
        AppConfig::default(),
    );
    app.set_queue(2, 0);

    // Drain the scripted Press and pump real audio through soul-audio-core:
    // 30 ticks * 256 frames/tick is comfortably past `tone.wav`'s fade-in,
    // same budget `device_headless.rs` uses before it measures anything.
    for i in 0..30u64 {
        app.tick(i * 20);
    }
    assert_eq!(
        app.state(),
        DeckState::Playing,
        "the scripted Press must have started playback"
    );
    assert!(
        !app.player().sink().recorded().is_empty(),
        "real audio must have reached SimSink before rendering"
    );

    app.render()
        .expect("App::render must succeed against a healthy SimDisplay");

    let fb = app.display();
    assert!(
        fb.non_blank_pixel_count() > 0,
        "a rendered Now-Playing frame must not be blank"
    );
    assert_eq!(
        fb.full_refresh_count(),
        1,
        "one App::render call must commit exactly one full_refresh"
    );

    // Structural check: the status-icon/title band (where `App::render`
    // draws the play/pause icon and the "Track 1" title) must differ from
    // an untouched blank baseline -- not just "some pixel somewhere in the
    // 800x480 frame changed".
    let blank = SimDisplay::new();
    let title_band_changed = (0..800)
        .flat_map(|x| (20..90).map(move |y| (x, y)))
        .any(|(x, y)| fb.pixel(x, y) != blank.pixel(x, y));
    assert!(
        title_band_changed,
        "the title/icon band must differ from a blank display's framebuffer"
    );

    // Dump the frame for eyeballing -- `dap/target/ui_now_playing.png`.
    let out_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/ui_now_playing.png");
    fb.save_png(&out_path).expect("save_png must succeed");
    assert!(out_path.exists(), "save_png must have written a file");
}
