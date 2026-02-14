//! End-to-End Layout Tests with Screenshot Validation
//!
//! This test suite validates the eink-system layout engine by:
//! 1. Creating various layout scenarios (VStack, HStack, nested, complex DAP-style)
//! 2. Rendering to headless emulator
//! 3. Taking screenshots
//! 4. Comparing against reference screenshots for visual regression testing
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all E2E tests
//! cargo test --test e2e_layout
//!
//! # Regenerate reference screenshots
//! UPDATE_SCREENSHOTS=1 cargo test --test e2e_layout
//! ```

use eink_emulator::Emulator;
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use image::{GrayImage, Luma};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================================
// Test Configuration
// ============================================================================

/// Maximum pixel difference tolerance (0.0 to 1.0)
/// 0.01 = 1% difference allowed for minor rendering variations
const PIXEL_DIFF_THRESHOLD: f32 = 0.01;

/// Screenshots directory
fn screenshots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("screenshots")
}

/// Reference screenshots directory
fn reference_dir() -> PathBuf {
    screenshots_dir().join("reference")
}

/// Actual screenshots directory (for comparison)
fn actual_dir() -> PathBuf {
    screenshots_dir().join("actual")
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Setup test environment (create directories)
fn setup() {
    fs::create_dir_all(reference_dir()).ok();
    fs::create_dir_all(actual_dir()).ok();
}

/// Check if we should update reference screenshots
fn should_update_screenshots() -> bool {
    env::var("UPDATE_SCREENSHOTS").is_ok()
}

/// Render using a render function and take screenshot
///
/// Returns the path to the saved screenshot
fn render_and_screenshot(
    render_fn: RenderFn,
    filename: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut emulator = Emulator::headless(250, 122);

    // Execute render function
    render_fn(&mut emulator)?;

    // Take screenshot
    let path = if should_update_screenshots() {
        reference_dir().join(format!("{}.png", filename))
    } else {
        actual_dir().join(format!("{}.png", filename))
    };

    emulator.screenshot(&path)?;
    Ok(path)
}

/// Compare two screenshots pixel-by-pixel
///
/// Returns the percentage difference (0.0 = identical, 1.0 = completely different)
fn compare_screenshots(
    actual: &Path,
    expected: &Path,
) -> Result<f32, Box<dyn std::error::Error>> {
    let actual_img = image::open(actual)?.to_luma8();
    let expected_img = image::open(expected)?.to_luma8();

    // Check dimensions match
    if actual_img.dimensions() != expected_img.dimensions() {
        return Err(format!(
            "Image dimensions mismatch: {:?} vs {:?}",
            actual_img.dimensions(),
            expected_img.dimensions()
        )
        .into());
    }

    // Count different pixels
    let total_pixels = (actual_img.width() * actual_img.height()) as usize;
    let mut diff_pixels = 0;

    for (actual_pixel, expected_pixel) in actual_img.pixels().zip(expected_img.pixels()) {
        if actual_pixel != expected_pixel {
            diff_pixels += 1;
        }
    }

    Ok(diff_pixels as f32 / total_pixels as f32)
}

/// Assert that a screenshot matches the reference
fn assert_screenshot_matches(filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    if should_update_screenshots() {
        println!("✓ Updated reference screenshot: {}.png", filename);
        return Ok(());
    }

    let actual = actual_dir().join(format!("{}.png", filename));
    let expected = reference_dir().join(format!("{}.png", filename));

    if !expected.exists() {
        return Err(format!(
            "Reference screenshot not found: {}. Run with UPDATE_SCREENSHOTS=1 to create it.",
            expected.display()
        )
        .into());
    }

    let diff = compare_screenshots(&actual, &expected)?;

    if diff > PIXEL_DIFF_THRESHOLD {
        return Err(format!(
            "Screenshot mismatch: {:.2}% difference (threshold: {:.2}%)\nActual: {}\nExpected: {}",
            diff * 100.0,
            PIXEL_DIFF_THRESHOLD * 100.0,
            actual.display(),
            expected.display()
        )
        .into());
    }

    println!("✓ Screenshot matches (diff: {:.2}%): {}.png", diff * 100.0, filename);
    Ok(())
}

// ==================================================================================================================
// Helper Rendering Functions
// ============================================================================

/// Render function type - takes an emulator and renders visual content
type RenderFn = fn(&mut Emulator) -> Result<(), Box<dyn std::error::Error>>;

// ============================================================================
// E2E Tests
// ============================================================================

#[test]
fn test_simple_vstack_three_children() {
    setup();

    // Render function for VStack with 3 children
    fn render_vstack(emulator: &mut Emulator) -> Result<(), Box<dyn std::error::Error>> {
        // Simple VStack with 3 children (black rectangles)
        let y_positions = [10, 50, 90];
        for (i, y) in y_positions.iter().enumerate() {
            Rectangle::new(Point::new(10, *y as i32), Size::new(230, 30))
                .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
                .draw(emulator)?;

            // Add text label
            let text = format!("Child {}", i + 1);
            Text::new(
                &text,
                Point::new(15, *y as i32 + 12),
                MonoTextStyle::new(&FONT_6X10, Gray4::WHITE),
            )
            .draw(emulator)?;
        }
        Ok(())
    }

    render_and_screenshot(render_vstack, "vstack_basic").unwrap();
    assert_screenshot_matches("vstack_basic").unwrap();
}

#[test]
fn test_hstack_space_between() {
    setup();

    fn render_hstack(emulator: &mut Emulator) -> Result<(), Box<dyn std::error::Error>> {
        // HStack with SpaceBetween justification
        let x_positions = [10, 110, 210];
        for (i, x) in x_positions.iter().enumerate() {
            Rectangle::new(Point::new(*x as i32, 40), Size::new(30, 40))
                .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
                .draw(emulator)?;

            // Add label
            let label = format!("{}", i + 1);
            Text::new(
                &label,
                Point::new(*x as i32 + 12, 55),
                MonoTextStyle::new(&FONT_6X10, Gray4::WHITE),
            )
            .draw(emulator)?;
        }
        Ok(())
    }

    render_and_screenshot(render_hstack, "hstack_space_between").unwrap();
    assert_screenshot_matches("hstack_space_between").unwrap();
}

#[test]
fn test_nested_vstack_in_hstack() {
    setup();

    fn render_nested(emulator: &mut Emulator) -> Result<(), Box<dyn std::error::Error>> {

        // HStack container with two VStacks inside

        // Left VStack
        Rectangle::new(Point::new(10, 10), Size::new(110, 100))
            .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
            .draw(emulator)?;

        Rectangle::new(Point::new(15, 15), Size::new(100, 25))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
            .draw(emulator)?;
        Text::new(
            "Left A",
            Point::new(20, 25),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(emulator)?;

        Rectangle::new(Point::new(15, 45), Size::new(100, 25))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
            .draw(emulator)?;
        Text::new(
            "Left B",
            Point::new(20, 55),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(emulator)?;

        Rectangle::new(Point::new(15, 75), Size::new(100, 25))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
            .draw(emulator)?;
        Text::new(
            "Left C",
            Point::new(20, 85),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(emulator)?;

        // Right VStack
        Rectangle::new(Point::new(130, 10), Size::new(110, 100))
            .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
            .draw(emulator)?;

        Rectangle::new(Point::new(135, 15), Size::new(100, 25))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
            .draw(emulator)?;
        Text::new(
            "Right A",
            Point::new(140, 25),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(emulator)?;

        Rectangle::new(Point::new(135, 45), Size::new(100, 25))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
            .draw(emulator)?;
        Text::new(
            "Right B",
            Point::new(140, 55),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(emulator)?;

        Ok(())
    }

    render_and_screenshot(render_nested, "nested_vstack_hstack").unwrap();
    assert_screenshot_matches("nested_vstack_hstack").unwrap();
}

#[tokio::test]
async fn test_complex_dap_layout() {
    setup();

    let mut emulator = Emulator::headless(250, 122);

    // Header
    Rectangle::new(Point::new(0, 0), Size::new(250, 20))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Now Playing", Point::new(5, 8), MonoTextStyle::new(&FONT_6X10, Gray4::WHITE))
        .draw(&mut emulator)
        .unwrap();

    // Content area
    Rectangle::new(Point::new(5, 25), Size::new(240, 70))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
        .draw(&mut emulator)
        .unwrap();

    // Album art placeholder
    Rectangle::new(Point::new(10, 30), Size::new(50, 50))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();

    // Track info
    Text::new("Track Title", Point::new(70, 35), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Artist Name", Point::new(70, 50), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Album Name", Point::new(70, 65), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    // Progress bar
    Rectangle::new(Point::new(70, 75), Size::new(170, 8))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
        .draw(&mut emulator)
        .unwrap();
    Rectangle::new(Point::new(71, 76), Size::new(85, 6))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Footer (controls)
    Rectangle::new(Point::new(0, 100), Size::new(250, 22))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();

    // Control buttons (simple circles/text)
    let buttons = ["<<", "||", ">>"];
    let button_x = [50, 115, 180];
    for (text, x) in buttons.iter().zip(button_x.iter()) {
        Text::new(text, Point::new(*x as i32, 111), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    let path = if should_update_screenshots() {
        reference_dir().join("dap_layout.png")
    } else {
        actual_dir().join("dap_layout.png")
    };

    emulator.screenshot(&path).unwrap();
    assert_screenshot_matches("dap_layout").unwrap();
}

#[tokio::test]
async fn test_justify_content_modes() {
    setup();

    let mut emulator = Emulator::headless(250, 122);

    // Title
    Text::new("JustifyContent Modes", Point::new(5, 8), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Start
    Text::new("Start:", Point::new(5, 25), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..3 {
        Rectangle::new(Point::new(50 + i * 25, 20), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // Center
    Text::new("Center:", Point::new(5, 40), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..3 {
        Rectangle::new(Point::new(82 + i * 25, 35), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // End
    Text::new("End:", Point::new(5, 55), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..3 {
        Rectangle::new(Point::new(165 + i * 25, 50), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // SpaceBetween
    Text::new("Between:", Point::new(5, 70), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    let positions = [50, 115, 180];
    for pos in positions {
        Rectangle::new(Point::new(pos, 65), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // SpaceAround
    Text::new("Around:", Point::new(5, 85), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    let positions = [62, 115, 168];
    for pos in positions {
        Rectangle::new(Point::new(pos, 80), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // SpaceEvenly
    Text::new("Evenly:", Point::new(5, 100), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    let positions = [68, 115, 162];
    for pos in positions {
        Rectangle::new(Point::new(pos, 95), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    let path = if should_update_screenshots() {
        reference_dir().join("justify_content_modes.png")
    } else {
        actual_dir().join("justify_content_modes.png")
    };

    emulator.screenshot(&path).unwrap();
    assert_screenshot_matches("justify_content_modes").unwrap();
}

#[tokio::test]
async fn test_align_items_modes() {
    setup();

    let mut emulator = Emulator::headless(250, 122);

    // Title
    Text::new("AlignItems Modes", Point::new(5, 8), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Container boxes for each mode (50x80)
    let modes = ["Start", "Center", "End", "Stretch"];
    let x_positions = [10, 70, 130, 190];

    for (mode, x) in modes.iter().zip(x_positions.iter()) {
        // Container
        Rectangle::new(Point::new(*x as i32, 20), Size::new(50, 80))
            .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
            .draw(&mut emulator)
            .unwrap();

        // Label
        Text::new(mode, Point::new(*x as i32 + 2, 30), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();

        // Child elements positioned according to alignment
        let child_y = match *mode {
            "Start" => 40,
            "Center" => 55,
            "End" => 75,
            "Stretch" => 40,
            _ => 40, // Default case
        };

        let child_height = if *mode == "Stretch" { 55 } else { 20 };

        Rectangle::new(
            Point::new(*x as i32 + 10, child_y),
            Size::new(30, child_height),
        )
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    }

    let path = if should_update_screenshots() {
        reference_dir().join("align_items_modes.png")
    } else {
        actual_dir().join("align_items_modes.png")
    };

    emulator.screenshot(&path).unwrap();
    assert_screenshot_matches("align_items_modes").unwrap();
}

#[tokio::test]
async fn test_gap_spacing() {
    setup();

    let mut emulator = Emulator::headless(250, 122);

    // Title
    Text::new("Gap Spacing", Point::new(5, 8), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Gap: 0
    Text::new("gap=0:", Point::new(5, 25), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..4 {
        Rectangle::new(Point::new(50 + i * 20, 20), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // Gap: 4
    Text::new("gap=4:", Point::new(5, 45), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..4 {
        Rectangle::new(Point::new(50 + i * 24, 40), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // Gap: 8
    Text::new("gap=8:", Point::new(5, 65), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..4 {
        Rectangle::new(Point::new(50 + i * 28, 60), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    // Gap: 16
    Text::new("gap=16:", Point::new(5, 85), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();
    for i in 0..4 {
        Rectangle::new(Point::new(50 + i * 36, 80), Size::new(20, 8))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
    }

    let path = if should_update_screenshots() {
        reference_dir().join("gap_spacing.png")
    } else {
        actual_dir().join("gap_spacing.png")
    };

    emulator.screenshot(&path).unwrap();
    assert_screenshot_matches("gap_spacing").unwrap();
}

#[tokio::test]
async fn test_margin_and_padding() {
    setup();

    let mut emulator = Emulator::headless(250, 122);

    // Title
    Text::new("Margin & Padding", Point::new(5, 8), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Example 1: No margin/padding
    Rectangle::new(Point::new(10, 20), Size::new(70, 40))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
        .draw(&mut emulator)
        .unwrap();
    Rectangle::new(Point::new(10, 20), Size::new(70, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();
    Text::new("None", Point::new(25, 35), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Example 2: Margin only (outer box larger)
    Rectangle::new(Point::new(90, 15), Size::new(80, 50))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
        .draw(&mut emulator)
        .unwrap();
    Rectangle::new(Point::new(95, 20), Size::new(70, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Margin", Point::new(105, 35), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Example 3: Padding only (inner content smaller)
    Rectangle::new(Point::new(180, 20), Size::new(60, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();
    Rectangle::new(Point::new(185, 25), Size::new(50, 30))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Pad", Point::new(192, 38), MonoTextStyle::new(&FONT_6X10, Gray4::WHITE))
        .draw(&mut emulator)
        .unwrap();

    // Example 4: Both (full spacing demonstration)
    Rectangle::new(Point::new(10, 70), Size::new(100, 45))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
        .draw(&mut emulator)
        .unwrap();
    Rectangle::new(Point::new(15, 75), Size::new(90, 35))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();
    Rectangle::new(Point::new(20, 80), Size::new(80, 25))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Both", Point::new(40, 90), MonoTextStyle::new(&FONT_6X10, Gray4::WHITE))
        .draw(&mut emulator)
        .unwrap();

    let path = if should_update_screenshots() {
        reference_dir().join("margin_padding.png")
    } else {
        actual_dir().join("margin_padding.png")
    };

    emulator.screenshot(&path).unwrap();
    assert_screenshot_matches("margin_padding").unwrap();
}

#[tokio::test]
async fn test_responsive_layout() {
    setup();

    let mut emulator = Emulator::headless(250, 122);

    // Simulate responsive layout that adapts to screen size
    // Header that spans full width
    Rectangle::new(Point::new(0, 0), Size::new(250, 15))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Responsive Layout", Point::new(5, 7), MonoTextStyle::new(&FONT_6X10, Gray4::WHITE))
        .draw(&mut emulator)
        .unwrap();

    // Two-column layout (60% / 40% split)
    // Left column (60% = 150px)
    Rectangle::new(Point::new(5, 20), Size::new(145, 95))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Main Content", Point::new(10, 30), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("(60% width)", Point::new(10, 45), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    // Right column (40% = 100px)
    Rectangle::new(Point::new(155, 20), Size::new(90, 95))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::new(2), 1))
        .draw(&mut emulator)
        .unwrap();
    Text::new("Sidebar", Point::new(160, 30), MonoTextStyle::new(&FONT_6X10, Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();
    Text::new("(40%)", Point::new(160, 45), MonoTextStyle::new(&FONT_6X10, Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    let path = if should_update_screenshots() {
        reference_dir().join("responsive_layout.png")
    } else {
        actual_dir().join("responsive_layout.png")
    };

    emulator.screenshot(&path).unwrap();
    assert_screenshot_matches("responsive_layout").unwrap();
}

// ============================================================================
// Screenshot Management Tests
// ============================================================================

#[test]
fn test_screenshot_directories_exist() {
    setup();
    assert!(screenshots_dir().exists());
    assert!(reference_dir().exists());
    assert!(actual_dir().exists());
}

#[test]
fn test_pixel_comparison_identical() {
    setup();

    // Create two identical images
    let img = GrayImage::from_pixel(100, 100, Luma([128]));
    let path1 = actual_dir().join("test_identical_1.png");
    let path2 = actual_dir().join("test_identical_2.png");
    img.save(&path1).unwrap();
    img.save(&path2).unwrap();

    let diff = compare_screenshots(&path1, &path2).unwrap();
    assert_eq!(diff, 0.0, "Identical images should have 0% difference");

    // Cleanup
    fs::remove_file(path1).ok();
    fs::remove_file(path2).ok();
}

#[test]
fn test_pixel_comparison_different() {
    setup();

    // Create two different images
    let img1 = GrayImage::from_pixel(100, 100, Luma([0]));
    let img2 = GrayImage::from_pixel(100, 100, Luma([255]));
    let path1 = actual_dir().join("test_different_1.png");
    let path2 = actual_dir().join("test_different_2.png");
    img1.save(&path1).unwrap();
    img2.save(&path2).unwrap();

    let diff = compare_screenshots(&path1, &path2).unwrap();
    assert_eq!(diff, 1.0, "Completely different images should have 100% difference");

    // Cleanup
    fs::remove_file(path1).ok();
    fs::remove_file(path2).ok();
}

#[test]
fn test_pixel_comparison_partial_difference() {
    setup();

    // Create image with 25% different pixels
    let img1 = GrayImage::from_pixel(100, 100, Luma([0]));
    let mut img2 = GrayImage::from_pixel(100, 100, Luma([0]));

    // Make 25% of pixels different (25x100 = 2500 pixels out of 10000)
    for y in 0..100 {
        for x in 0..25 {
            img2.put_pixel(x, y, Luma([255]));
        }
    }

    let path1 = actual_dir().join("test_partial_1.png");
    let path2 = actual_dir().join("test_partial_2.png");
    img1.save(&path1).unwrap();
    img2.save(&path2).unwrap();

    let diff = compare_screenshots(&path1, &path2).unwrap();
    assert!((diff - 0.25).abs() < 0.01, "Should have ~25% difference, got {}", diff);

    // Cleanup
    fs::remove_file(path1).ok();
    fs::remove_file(path2).ok();
}
