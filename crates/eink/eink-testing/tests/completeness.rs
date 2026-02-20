//! Completeness tests for eink-testing utilities.
//! These tests verify the full testing API is functional.

// Test file — unwrap/expect/panic acceptable in test code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use eink_testing::TestEmulator;
use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use std::path::Path;

// ── Basic utility tests ───────────────────────────────────────────────────

#[test]
fn test_emulator_dimensions() {
    let t = TestEmulator::new(200, 100);
    // Should expose width and height
    assert_eq!(t.width(), 200);
    assert_eq!(t.height(), 100);
}

#[test]
fn test_emulator_clear_fills_white() {
    let mut t = TestEmulator::new(50, 50);
    // Draw something black
    Rectangle::new(Point::zero(), Size::new(50, 50))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t)
        .unwrap();
    assert_eq!(t.pixel_at(0, 0), Some(Gray4::BLACK));

    // Clear should restore to white
    t.clear_display();
    assert_eq!(t.pixel_at(0, 0), Some(Gray4::WHITE));
    assert_eq!(t.pixel_at(49, 49), Some(Gray4::WHITE));
}

#[test]
fn test_pixel_diff_count_same_image() {
    let mut t1 = TestEmulator::new(20, 20);
    let mut t2 = TestEmulator::new(20, 20);

    // Draw same thing on both
    Rectangle::new(Point::new(5, 5), Size::new(10, 10))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t1)
        .unwrap();
    Rectangle::new(Point::new(5, 5), Size::new(10, 10))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t2)
        .unwrap();

    assert_eq!(t1.pixel_diff_count(&t2), 0);
}

#[test]
fn test_pixel_diff_count_different_images() {
    let mut t1 = TestEmulator::new(20, 20);
    let t2 = TestEmulator::new(20, 20);

    Rectangle::new(Point::zero(), Size::new(20, 20))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t1)
        .unwrap();
    // t2 stays white

    let diff = t1.pixel_diff_count(&t2);
    assert_eq!(diff, 400, "20x20 = 400 pixels should all differ");
}

#[test]
fn test_assert_region_non_uniform_passes_when_mixed() {
    let mut t = TestEmulator::new(50, 50);
    // Draw half black, half white in a region
    Rectangle::new(Point::new(10, 10), Size::new(10, 10))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t)
        .unwrap();
    // The 20x10 region starting at (10,10) has black left half, white right half
    let mixed_region = Rectangle::new(Point::new(10, 10), Size::new(20, 10));
    assert!(
        t.assert_region_non_uniform(mixed_region).is_ok(),
        "Region with mixed pixels should pass non-uniform check"
    );
}

#[test]
fn test_assert_region_non_uniform_fails_when_uniform() {
    let t = TestEmulator::new(50, 50);
    // All-white region
    let uniform_region = Rectangle::new(Point::new(0, 0), Size::new(10, 10));
    assert!(
        t.assert_region_non_uniform(uniform_region).is_err(),
        "All-white region should fail non-uniform check"
    );
}

#[test]
fn test_component_bounds_accessible() {
    let mut t = TestEmulator::new(200, 100);
    t.register_component("header", "Container", (0, 0), (200, 50));
    t.register_component("footer", "Container", (0, 50), (200, 50));

    let header = t.query_by_test_id("header").unwrap();
    let footer = t.query_by_test_id("footer").unwrap();

    // Header and footer should not overlap (they stack vertically)
    assert!(
        !bounds_overlap(header.position, header.size, footer.position, footer.size),
        "Header and footer should not overlap"
    );
}

fn bounds_overlap(
    pos1: (i32, i32),
    size1: (u32, u32),
    pos2: (i32, i32),
    size2: (u32, u32),
) -> bool {
    let r1x2 = pos1.0 + size1.0 as i32;
    let r1y2 = pos1.1 + size1.1 as i32;
    let r2x2 = pos2.0 + size2.0 as i32;
    let r2y2 = pos2.1 + size2.1 as i32;

    pos1.0 < r2x2 && r1x2 > pos2.0 && pos1.1 < r2y2 && r1y2 > pos2.1
}

#[test]
fn test_snapshot_workflow_creates_file() {
    let mut t = TestEmulator::new(100, 50);
    Rectangle::new(Point::new(10, 10), Size::new(30, 20))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t)
        .unwrap();

    let path = "tests/golden/test_snapshot.png";
    // Force-create golden (UPDATE_GOLDEN behavior)
    t.save_golden(path).unwrap();

    // File should now exist
    assert!(Path::new(path).exists(), "Golden file should be created");

    // Subsequent assert_matches_golden should pass with 0 threshold
    t.assert_matches_golden(path, 0).unwrap();

    // Clean up
    std::fs::remove_file(path).ok();
}

#[test]
fn test_golden_fails_when_images_differ() {
    let mut t1 = TestEmulator::new(50, 50);
    Rectangle::new(Point::zero(), Size::new(50, 50))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t1)
        .unwrap();

    let path = "tests/golden/test_diff.png";
    t1.save_golden(path).unwrap();

    // Different image (white)
    let t2 = TestEmulator::new(50, 50); // all white
    let result = t2.assert_matches_golden(path, 10); // threshold 10 pixels
    assert!(
        result.is_err(),
        "Should fail when images differ by more than threshold"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn test_dark_pixel_percentage() {
    let mut t = TestEmulator::new(100, 100);
    // Fill top half with black
    Rectangle::new(Point::zero(), Size::new(100, 50))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut *t)
        .unwrap();

    let total = Rectangle::new(Point::zero(), Size::new(100, 100));
    let black_count = t.pixel_count_of_color(total, Gray4::BLACK);
    assert_eq!(
        black_count, 5000,
        "Top half should be 50*100=5000 black pixels"
    );

    let pct = t.dark_pixel_percentage(total);
    assert!(
        (pct - 50.0).abs() < 0.1,
        "Dark pixel percentage should be ~50%, got {pct}"
    );
}
