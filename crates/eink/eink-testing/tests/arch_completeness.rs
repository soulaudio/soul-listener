//! Architecture-enforcement tests for the eink-testing API surface.
//!
//! These tests confirm that the required methods exist and behave correctly.
//! Compilation of this file is itself an enforcement: if a method is removed
//! or signature-changed, this test binary fails to build.

use eink_testing::TestEmulator;

/// Verify that `TestEmulator` exposes `width()` and `height()` accessors.
///
/// These are necessary for test code that needs to reason about display bounds
/// without reaching into the internal `Emulator` struct.
#[test]
fn eink_testing_has_width_height_api() {
    let t = TestEmulator::new(800, 480);
    assert_eq!(t.width(), 800);
    assert_eq!(t.height(), 480);
}

/// Verify that `pixel_diff_count` returns 0 when comparing identical (default) displays.
///
/// This confirms the method exists and behaves correctly for the zero-diff case.
#[test]
fn eink_testing_pixel_diff_same_image_is_zero() {
    let t1 = TestEmulator::new(10, 10);
    let t2 = TestEmulator::new(10, 10);
    assert_eq!(t1.pixel_diff_count(&t2), 0);
}

/// Verify `clear_display` resets all pixels to white.
///
/// This is the baseline behavior: after a clear, every sampled pixel must be white.
#[test]
fn eink_testing_clear_display_resets_to_white() {
    use embedded_graphics::{
        pixelcolor::Gray4,
        prelude::*,
        primitives::{PrimitiveStyle, Rectangle},
    };

    let mut t = TestEmulator::new(32, 32);

    // Dirty the display
    Rectangle::new(Point::zero(), Size::new(32, 32))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t)
        .unwrap();
    assert_eq!(t.pixel_at(0, 0), Some(Gray4::BLACK));

    t.clear_display();

    // All corners must be white
    assert_eq!(t.pixel_at(0, 0), Some(Gray4::WHITE));
    assert_eq!(t.pixel_at(31, 31), Some(Gray4::WHITE));
    assert_eq!(t.pixel_at(0, 31), Some(Gray4::WHITE));
    assert_eq!(t.pixel_at(31, 0), Some(Gray4::WHITE));
}

/// Verify `assert_region_non_uniform` returns `Err` on a uniform region.
///
/// A uniform region should always fail this check — it is the caller's job
/// to ensure the region contains meaningful variation.
#[test]
fn eink_testing_non_uniform_rejects_uniform_region() {
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::Rectangle;

    let t = TestEmulator::new(20, 20); // all white by default
    let region = Rectangle::new(Point::zero(), Size::new(20, 20));
    assert!(
        t.assert_region_non_uniform(region).is_err(),
        "A uniform region must fail the non-uniform check"
    );
}

/// Verify `dark_pixel_percentage` returns 0.0 on a brand-new (all-white) display.
#[test]
fn eink_testing_dark_pixel_pct_zero_on_white() {
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::Rectangle;

    let t = TestEmulator::new(50, 50);
    let region = Rectangle::new(Point::zero(), Size::new(50, 50));
    let pct = t.dark_pixel_percentage(region);
    assert!(
        pct == 0.0,
        "All-white display must have 0% dark pixels, got {pct}"
    );
}
