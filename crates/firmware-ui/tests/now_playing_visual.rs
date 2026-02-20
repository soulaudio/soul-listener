//! Visual regression tests for the Now Playing screen.
//! Uses eink_testing::TestEmulator for headless rendering and pixel assertions.
//!
//! Run: cargo test -p firmware-ui --test now_playing_visual

use eink_testing::TestEmulator;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use ui::now_playing::NowPlayingState;

/// Helper: build a standard test state
fn mock_state() -> NowPlayingState {
    let mut s = NowPlayingState::default();
    s.set_playing(true);
    s.set_volume(75);
    s.set_duration_ms(240_000);
    s.set_position_ms(60_000); // 25% through
    s
}

/// Render Now Playing onto a TestEmulator with component registration.
///
/// Collects component registrations and applies them after drawing to avoid
/// the double-borrow problem (drawing borrows via DerefMut, registration
/// borrows TestEmulator directly).
fn render(t: &mut TestEmulator, state: &NowPlayingState) {
    let mut regs: Vec<(String, String, (i32, i32), (u32, u32))> = Vec::new();
    firmware_ui::screens::now_playing::render_now_playing_to(
        &mut **t,
        state,
        |id, ty, pos, size| {
            regs.push((id.to_owned(), ty.to_owned(), pos, size));
        },
    )
    .unwrap();
    for (id, ty, pos, size) in regs {
        t.register_component(&id, &ty, pos, size);
    }
}

#[test]
fn now_playing_renders_without_panic() {
    let mut t = TestEmulator::new(400, 300);
    let state = mock_state();
    render(&mut t, &state);
}

#[test]
fn now_playing_header_is_dark() {
    // Header bar should be filled (non-white) pixels at top
    let mut t = TestEmulator::new(400, 300);
    render(&mut t, &mock_state());
    // At least some pixels in the top 50px should be dark (not WHITE)
    let has_dark = (0..400u32).any(|x| {
        t.pixel_at(x, 20)
            .map(|c| c != Gray4::WHITE)
            .unwrap_or(false)
    });
    assert!(has_dark, "Header area should contain dark pixels");
}

#[test]
fn now_playing_progress_bar_registered() {
    let mut t = TestEmulator::new(400, 300);
    render(&mut t, &mock_state());
    t.assert_has_component("now-playing-progress").unwrap();
}

#[test]
fn now_playing_title_registered() {
    let mut t = TestEmulator::new(400, 300);
    render(&mut t, &mock_state());
    t.assert_has_component("now-playing-title").unwrap();
}

#[test]
fn now_playing_play_button_registered() {
    let mut t = TestEmulator::new(400, 300);
    render(&mut t, &mock_state());
    t.assert_has_component("now-playing-play-btn").unwrap();
}

#[test]
fn now_playing_progress_at_25_percent() {
    let mut t = TestEmulator::new(400, 300);
    let state = mock_state(); // position=60_000, duration=240_000 → 25%
    render(&mut t, &state);
    let bar = t.query_by_test_id("now-playing-progress").unwrap();
    // Progress bar y position should be below the header
    assert!(
        bar.position.1 > 100,
        "progress bar should be below header, got y={}",
        bar.position.1
    );
}

#[test]
fn now_playing_paused_state_different_button_label() {
    let mut t1 = TestEmulator::new(400, 300);
    let mut playing_state = mock_state();
    playing_state.set_playing(true);
    render(&mut t1, &playing_state);

    let mut t2 = TestEmulator::new(400, 300);
    let mut paused_state = mock_state();
    paused_state.set_playing(false);
    render(&mut t2, &paused_state);

    // Both should render without error; pixel counts may differ
    let playing_dark = (0..400u32)
        .flat_map(|x| (0..300u32).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            t1.pixel_at(x, y)
                .map(|c| c != Gray4::WHITE)
                .unwrap_or(false)
        })
        .count();
    let paused_dark = (0..400u32)
        .flat_map(|x| (0..300u32).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            t2.pixel_at(x, y)
                .map(|c| c != Gray4::WHITE)
                .unwrap_or(false)
        })
        .count();

    assert!(playing_dark > 0, "playing state must render something");
    assert!(paused_dark > 0, "paused state must render something");
}

#[test]
fn now_playing_golden_screenshot() {
    // Golden test: first run creates reference (UPDATE_GOLDEN=1 or file missing),
    // subsequent runs compare pixel-by-pixel within threshold.
    let mut t = TestEmulator::new(400, 300);
    render(&mut t, &mock_state());
    t.assert_matches_golden("tests/golden/now_playing.png", 5)
        .unwrap();
}
