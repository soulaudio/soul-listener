//! E-Ink UI Testing Utilities
//!
//! Playwright-inspired headless testing API for e-ink display UIs.
//!
//! # Quick start
//!
//! ```no_run
//! use eink_testing::TestEmulator;
//! use eink_specs::displays::WAVESHARE_2_13_V4;
//! use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::{PrimitiveStyle, Rectangle}};
//!
//! let mut t = TestEmulator::with_spec(&WAVESHARE_2_13_V4);
//!
//! // Draw UI content
//! Rectangle::new(Point::new(10, 10), Size::new(40, 20))
//!     .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
//!     .draw(&mut *t)
//!     .unwrap();
//!
//! // Register the component so it can be queried by test ID
//! t.register_component("header", "Container", (10, 10), (40, 20));
//!
//! // Assertions
//! t.assert_pixel(20, 15, Gray4::BLACK).unwrap();
//! t.assert_has_component("header").unwrap();
//! ```
//!
//! # Feature flags
//!
//! | Feature | What it unlocks |
//! |---------|-----------------|
//! | `debug` | `query_from_debug_manager()` — read components registered in the emulator's debug overlay |
//! | `keyboard-input` | `simulate_key()`, `simulate_scroll()`, `take_events()` |
//!
//! # Golden screenshot testing
//!
//! ```no_run
//! # use eink_testing::TestEmulator;
//! # let t = TestEmulator::new(100, 100);
//! // First run: set UPDATE_GOLDEN=1 to create/update the reference file.
//! t.assert_matches_golden("tests/golden/my_screen.png", 0).unwrap();
//! ```

#![warn(clippy::all)]
#![warn(clippy::dbg_macro)]
// Testing lib — println is allowed (clippy.toml has allow-print-in-tests = true)
// but we still warn on it in lib code (not test code)
#![warn(clippy::print_stdout)]
#![allow(clippy::module_name_repetitions)]

use std::path::Path;

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

pub use eink_emulator::{EinkColor, Emulator};
pub use eink_specs::DisplaySpec;

// Re-export input types when the feature is active so callers only need
// `eink_testing::Button` / `eink_testing::InputEvent`.
#[cfg(feature = "keyboard-input")]
pub use eink_emulator::input::{Button, InputEvent};

// ─────────────────────────────────────────────────────────────────────────────
// ComponentRef
// ─────────────────────────────────────────────────────────────────────────────

/// A UI component registered in the [`TestEmulator`]'s component registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRef {
    /// The test identifier (analogous to `data-testid` in web testing).
    pub test_id: String,
    /// Broad component category, e.g. `"Button"`, `"Label"`, `"Container"`.
    pub component_type: String,
    /// Top-left corner in display coordinates.
    pub position: (i32, i32),
    /// Width × height in pixels.
    pub size: (u32, u32),
}

impl ComponentRef {
    /// The bounding rectangle as an `embedded-graphics` [`Rectangle`].
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(
            Point::new(self.position.0, self.position.1),
            Size::new(self.size.0, self.size.1),
        )
    }

    /// Centre of the component's bounding box.
    pub fn center(&self) -> Point {
        self.bounds().center()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TestEmulator
// ─────────────────────────────────────────────────────────────────────────────

/// Headless e-ink emulator for UI testing.
///
/// Wraps [`Emulator`] and adds:
/// - A component registry queryable by test ID
/// - Pixel and region assertions
/// - Screenshot capture and golden-file comparison
/// - Input event simulation (with the `keyboard-input` feature)
///
/// Derefs to [`Emulator`], which implements [`DrawTarget`], so you can draw
/// embedded-graphics primitives directly:
///
/// ```no_run
/// use eink_testing::TestEmulator;
/// use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::{PrimitiveStyle, Rectangle}};
///
/// let mut t = TestEmulator::new(100, 100);
/// Rectangle::new(Point::zero(), Size::new(50, 50))
///     .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
///     .draw(&mut *t)
///     .unwrap();
/// ```
pub struct TestEmulator {
    inner: Emulator,
    components: Vec<ComponentRef>,
    #[cfg(feature = "keyboard-input")]
    pending_events: Vec<InputEvent>,
}

impl TestEmulator {
    /// Create a headless emulator with exact pixel dimensions.
    ///
    /// A minimal display spec is synthesised from the given dimensions so
    /// tests work at the size they request (unlike `Emulator::headless` which
    /// ignores its arguments and uses a fixed Waveshare spec).
    pub fn new(width: u32, height: u32) -> Self {
        use eink_specs::{ColorMode, Controller, DisplaySpec, PanelType};
        // Box::leak is intentional: specs must be `&'static`.
        // Memory cost is negligible for testing (<100 bytes per emulator).
        let spec: &'static DisplaySpec = Box::leak(Box::new(DisplaySpec {
            name: "TestDisplay",
            width,
            height,
            controller: Controller::SSD1680,
            panel_type: PanelType::Carta1000,
            color_mode: Some(ColorMode::Grayscale),
            grayscale_levels: 4,
            full_refresh_ms: 100,
            partial_refresh_ms: 50,
            fast_refresh_ms: 30,
            ghosting_rate_partial: 0.1,
            ghosting_rate_fast: 0.2,
            flash_count_full: 1,
            temp_optimal_min: 0,
            temp_optimal_max: 50,
            temp_operating_min: -10,
            temp_operating_max: 60,
            quirks: None,
        }));
        Self {
            inner: Emulator::headless_with_spec(spec),
            components: Vec::new(),
            #[cfg(feature = "keyboard-input")]
            pending_events: Vec::new(),
        }
    }

    /// Create a headless emulator sized to a specific display spec.
    pub fn with_spec(spec: &'static DisplaySpec) -> Self {
        Self {
            inner: Emulator::headless_with_spec(spec),
            components: Vec::new(),
            #[cfg(feature = "keyboard-input")]
            pending_events: Vec::new(),
        }
    }

    // ── Framebuffer access ───────────────────────────────────────────────────

    /// Return the grayscale color at `(x, y)`, or `None` if out of bounds.
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<Gray4> {
        match self.inner.framebuffer.get_pixel(x, y)? {
            EinkColor::Gray(g) => Some(g),
            // Non-gray modes treated as black for testing purposes.
            EinkColor::Spectra6 { bw, .. } => Some(bw),
            EinkColor::Kaleido3 { .. } => Some(Gray4::BLACK),
        }
    }

    // ── Pixel assertions ─────────────────────────────────────────────────────

    /// Assert that pixel `(x, y)` has the expected grayscale luma.
    ///
    /// Returns `Err` with a descriptive message on mismatch.
    pub fn assert_pixel(&self, x: u32, y: u32, expected: Gray4) -> Result<(), String> {
        let actual = self
            .pixel_at(x, y)
            .ok_or_else(|| format!("Pixel ({x}, {y}) is out of bounds"))?;
        if actual.luma() != expected.luma() {
            Err(format!(
                "assert_pixel({x}, {y}): expected luma {}, got luma {}",
                expected.luma(),
                actual.luma()
            ))
        } else {
            Ok(())
        }
    }

    /// Assert that every pixel inside `rect` has the given color.
    pub fn assert_region_uniform(&self, rect: Rectangle, color: Gray4) -> Result<(), String> {
        let tl = rect.top_left;
        for dy in 0..rect.size.height {
            for dx in 0..rect.size.width {
                let x = (tl.x as u32).wrapping_add(dx);
                let y = (tl.y as u32).wrapping_add(dy);
                self.assert_pixel(x, y, color)
                    .map_err(|e| format!("assert_region_uniform failed in {rect:?}: {e}"))?;
            }
        }
        Ok(())
    }

    /// Assert that `rect` contains **at least one** pixel with the given color.
    pub fn assert_region_contains(&self, rect: Rectangle, color: Gray4) -> Result<(), String> {
        if self.pixel_count_of_color(rect, color) > 0 {
            Ok(())
        } else {
            Err(format!(
                "assert_region_contains: no pixel with luma {} found in {rect:?}",
                color.luma()
            ))
        }
    }

    /// Count how many pixels in `rect` match `color`'s luma.
    pub fn pixel_count_of_color(&self, rect: Rectangle, color: Gray4) -> usize {
        let tl = rect.top_left;
        let mut count = 0;
        for dy in 0..rect.size.height {
            for dx in 0..rect.size.width {
                let x = (tl.x as u32).wrapping_add(dx);
                let y = (tl.y as u32).wrapping_add(dy);
                if self.pixel_at(x, y).map(|p| p.luma()) == Some(color.luma()) {
                    count += 1;
                }
            }
        }
        count
    }

    // ── Component registry ───────────────────────────────────────────────────

    /// Register (or update) a component by test ID.
    ///
    /// Call this after rendering a screen to annotate where each logical
    /// component sits, so tests can use [`query_by_test_id`] to look it up.
    pub fn register_component(
        &mut self,
        test_id: &str,
        component_type: &str,
        position: (i32, i32),
        size: (u32, u32),
    ) {
        if let Some(existing) = self.components.iter_mut().find(|c| c.test_id == test_id) {
            existing.component_type = component_type.to_string();
            existing.position = position;
            existing.size = size;
        } else {
            self.components.push(ComponentRef {
                test_id: test_id.to_string(),
                component_type: component_type.to_string(),
                position,
                size,
            });
        }
    }

    /// Remove all registered components.
    pub fn clear_components(&mut self) {
        self.components.clear();
    }

    /// Find a component by test ID in the local registry.
    pub fn query_by_test_id(&self, test_id: &str) -> Option<&ComponentRef> {
        self.components.iter().find(|c| c.test_id == test_id)
    }

    /// Return all registered components.
    pub fn components(&self) -> &[ComponentRef] {
        &self.components
    }

    /// Total number of registered components.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Assert that a component with `test_id` exists in the registry.
    pub fn assert_has_component(&self, test_id: &str) -> Result<(), String> {
        self.query_by_test_id(test_id)
            .ok_or_else(|| format!("Component '{test_id}' not found"))
            .map(|_| ())
    }

    // ── Screenshot utilities ─────────────────────────────────────────────────

    /// Save the current framebuffer as a PNG.
    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.screenshot(path)
    }

    /// Save the current framebuffer as the golden reference PNG.
    ///
    /// Parent directories are created automatically.
    pub fn save_golden(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.inner.screenshot(p)
    }

    /// Assert the current framebuffer matches a golden reference PNG.
    ///
    /// `threshold` is the maximum per-channel absolute difference that is still
    /// considered equal (0 = exact match; useful for font rendering differences).
    ///
    /// Set the `UPDATE_GOLDEN=1` environment variable to **update** the golden
    /// file instead of asserting:
    ///
    /// ```bash
    /// UPDATE_GOLDEN=1 cargo test
    /// ```
    pub fn assert_matches_golden(
        &self,
        golden_path: impl AsRef<Path>,
        threshold: u8,
    ) -> Result<(), String> {
        let golden_path = golden_path.as_ref();

        if std::env::var("UPDATE_GOLDEN").is_ok() {
            return self.save_golden(golden_path).map_err(|e| {
                format!(
                    "Failed to save golden '{p}': {e}",
                    p = golden_path.display()
                )
            });
        }

        use image::GenericImageView;
        let tmp = {
            let mut p = std::env::temp_dir();
            p.push(format!("eink_testing_{}.png", std::process::id()));
            p
        };
        self.inner
            .screenshot(&tmp)
            .map_err(|e| format!("Failed to capture screenshot: {e}"))?;
        let current = image::open(&tmp).map_err(|e| format!("Failed to open screenshot: {e}"))?;
        let _ = std::fs::remove_file(&tmp);

        let golden = image::open(golden_path).map_err(|e| {
            format!(
                "Failed to open golden '{}': {e}\nRun with UPDATE_GOLDEN=1 to create it.",
                golden_path.display()
            )
        })?;

        if current.dimensions() != golden.dimensions() {
            let (cw, ch) = current.dimensions();
            let (gw, gh) = golden.dimensions();
            return Err(format!(
                "Dimension mismatch: screenshot is {cw}×{ch}, golden is {gw}×{gh}"
            ));
        }

        let current_rgba = current.to_rgba8();
        let golden_rgba = golden.to_rgba8();
        let mut diff_pixels: u64 = 0;
        for (cp, gp) in current_rgba.pixels().zip(golden_rgba.pixels()) {
            let differs =
                cp.0.iter()
                    .zip(gp.0.iter())
                    .any(|(&a, &b)| (a as i32 - b as i32).unsigned_abs() as u8 > threshold);
            if differs {
                diff_pixels += 1;
            }
        }

        if diff_pixels > 0 {
            Err(format!(
                "{diff_pixels} pixels differ from golden '{}' (threshold={threshold})",
                golden_path.display()
            ))
        } else {
            Ok(())
        }
    }

    // ── Input simulation (keyboard-input feature) ────────────────────────────

    /// Enqueue a [`ButtonPress`] + [`ButtonRelease`] pair.
    ///
    /// Retrieve the events with [`take_events`].
    #[cfg(feature = "keyboard-input")]
    pub fn simulate_key(&mut self, button: Button) {
        self.pending_events.push(InputEvent::ButtonPress(button));
        self.pending_events.push(InputEvent::ButtonRelease(button));
    }

    /// Enqueue a [`RotaryIncrement`] event.
    ///
    /// Positive `steps` = clockwise; negative = counter-clockwise.
    #[cfg(feature = "keyboard-input")]
    pub fn simulate_scroll(&mut self, steps: i32) {
        self.pending_events.push(InputEvent::RotaryIncrement(steps));
    }

    /// Drain and return all pending input events.
    ///
    /// ```no_run
    /// # #[cfg(feature = "keyboard-input")]
    /// # {
    /// use eink_testing::{TestEmulator, Button, InputEvent};
    /// let mut t = TestEmulator::new(100, 100);
    /// t.simulate_key(Button::Play);
    /// let events = t.take_events();
    /// assert_eq!(events[0], InputEvent::ButtonPress(Button::Play));
    /// # }
    /// ```
    #[cfg(feature = "keyboard-input")]
    pub fn take_events(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.pending_events)
    }

    // ── Emulator access ──────────────────────────────────────────────────────

    pub fn emulator(&self) -> &Emulator {
        &self.inner
    }

    pub fn emulator_mut(&mut self) -> &mut Emulator {
        &mut self.inner
    }
}

impl std::ops::Deref for TestEmulator {
    type Target = Emulator;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for TestEmulator {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::{
        pixelcolor::Gray4,
        prelude::{Point, Size},
        primitives::{PrimitiveStyle, Rectangle},
    };

    #[test]
    fn pixel_at_default_is_white() {
        let t = TestEmulator::new(50, 50);
        assert_eq!(t.pixel_at(0, 0), Some(Gray4::WHITE));
        assert_eq!(t.pixel_at(49, 49), Some(Gray4::WHITE));
    }

    #[test]
    fn pixel_at_out_of_bounds_is_none() {
        let t = TestEmulator::new(50, 50);
        assert_eq!(t.pixel_at(50, 0), None);
        assert_eq!(t.pixel_at(0, 50), None);
    }

    #[test]
    fn assert_pixel_after_draw() {
        let mut t = TestEmulator::new(50, 50);
        Rectangle::new(Point::new(10, 10), Size::new(10, 10))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut *t)
            .unwrap();
        assert!(t.assert_pixel(15, 15, Gray4::BLACK).is_ok());
        assert!(t.assert_pixel(0, 0, Gray4::WHITE).is_ok());
        assert!(t.assert_pixel(15, 15, Gray4::WHITE).is_err());
    }

    #[test]
    fn register_and_query_component() {
        let mut t = TestEmulator::new(100, 100);
        t.register_component("play-btn", "Button", (10, 20), (80, 30));

        assert!(t.assert_has_component("play-btn").is_ok());
        assert!(t.assert_has_component("missing").is_err());

        let c = t.query_by_test_id("play-btn").unwrap();
        assert_eq!(c.position, (10, 20));
        assert_eq!(c.size, (80, 30));
        assert_eq!(t.component_count(), 1);
    }

    #[test]
    fn register_component_overwrites_existing() {
        let mut t = TestEmulator::new(100, 100);
        t.register_component("btn", "Button", (0, 0), (50, 20));
        t.register_component("btn", "Button", (5, 5), (60, 25));
        assert_eq!(t.component_count(), 1);
        assert_eq!(t.query_by_test_id("btn").unwrap().position, (5, 5));
    }

    #[test]
    fn region_assertions() {
        let mut t = TestEmulator::new(50, 50);
        Rectangle::new(Point::new(5, 5), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut *t)
            .unwrap();

        let filled = Rectangle::new(Point::new(5, 5), Size::new(20, 20));
        let empty = Rectangle::new(Point::new(30, 30), Size::new(10, 10));

        assert!(t.assert_region_contains(filled, Gray4::BLACK).is_ok());
        assert!(t.assert_region_uniform(empty, Gray4::WHITE).is_ok());
        assert!(t.assert_region_contains(empty, Gray4::BLACK).is_err());
    }

    #[test]
    fn pixel_count_of_color() {
        let mut t = TestEmulator::new(10, 10);
        Rectangle::new(Point::zero(), Size::new(5, 5))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut *t)
            .unwrap();
        assert_eq!(
            t.pixel_count_of_color(
                Rectangle::new(Point::zero(), Size::new(10, 10)),
                Gray4::BLACK
            ),
            25
        );
    }

    #[cfg(feature = "keyboard-input")]
    #[test]
    fn simulate_key_produces_press_release() {
        let mut t = TestEmulator::new(50, 50);
        t.simulate_key(Button::Play);
        let events = t.take_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], InputEvent::ButtonPress(Button::Play));
        assert_eq!(events[1], InputEvent::ButtonRelease(Button::Play));
    }

    #[cfg(feature = "keyboard-input")]
    #[test]
    fn simulate_scroll() {
        let mut t = TestEmulator::new(50, 50);
        t.simulate_scroll(3);
        t.simulate_scroll(-1);
        let events = t.take_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], InputEvent::RotaryIncrement(3));
        assert_eq!(events[1], InputEvent::RotaryIncrement(-1));
    }

    #[cfg(feature = "keyboard-input")]
    #[test]
    fn take_events_drains_queue() {
        let mut t = TestEmulator::new(50, 50);
        t.simulate_key(Button::Menu);
        let _ = t.take_events();
        assert!(t.take_events().is_empty());
    }
}
