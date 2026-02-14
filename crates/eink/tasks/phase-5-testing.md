# Phase 5: Testing Infrastructure (eink-testing)

**Duration:** Week 5-6 (40 hours)
**Status:** ⏳ Pending
**Dependencies:** Phase 3 (Components), Phase 4 (Emulator)

## Overview

Build comprehensive testing utilities with Playwright-like API, screenshot comparison, test ID queries, and assertion helpers.

---

## Tasks

### 5.1: Test ID Query System
**Effort:** 5 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement query system to find components by test ID.

**Acceptance Criteria:**
- [ ] `query_by_test_id(id)` returns component reference
- [ ] `query_all_by_test_id(id)` returns all matches
- [ ] `get_by_test_id(id)` panics if not found (strict)
- [ ] Builds accessibility tree from View hierarchy
- [ ] Works with nested components

**Implementation:**
```rust
pub struct AccessibilityTree {
    nodes: Vec<AccessibilityNode>,
}

pub struct AccessibilityNode {
    pub test_id: Option<String>,
    pub bounds: Rectangle,
    pub node_type: NodeType,
    pub children: Vec<usize>,  // Indices
}

impl Emulator {
    pub fn query_by_test_id(&self, id: &str) -> Option<&AccessibilityNode> {
        self.accessibility_tree
            .nodes
            .iter()
            .find(|node| node.test_id.as_deref() == Some(id))
    }

    pub fn get_by_test_id(&self, id: &str) -> &AccessibilityNode {
        self.query_by_test_id(id)
            .expect(&format!("Element with test_id='{}' not found", id))
    }
}
```

**Tests:**
```rust
#[test]
fn test_query_by_test_id() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);

    let button = Button::new("Click").test_id("submit-btn");
    button.draw(&mut emulator, emulator.bounding_box()).unwrap();

    let node = emulator.get_by_test_id("submit-btn");
    assert_eq!(node.node_type, NodeType::Button);
}
```

---

### 5.2: Accessibility Tree Building
**Effort:** 6 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Build accessibility tree from View hierarchy during rendering.

**Acceptance Criteria:**
- [ ] Tree built during `draw()` calls
- [ ] Captures bounds, test_id, node type
- [ ] Hierarchical structure preserved
- [ ] Efficient (O(n) traversal)

**Implementation:**
```rust
pub trait View {
    fn draw<D>(&self, target: &mut D, bounds: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>
    {
        // Build accessibility node
        if let Some(ctx) = target.accessibility_context() {
            ctx.add_node(AccessibilityNode {
                test_id: self.test_id().map(String::from),
                bounds,
                node_type: self.node_type(),
                children: vec![],
            });
        }

        // Actual drawing...
        self.draw_impl(target, bounds)
    }
}
```

---

### 5.3: Screenshot Comparison
**Effort:** 5 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Compare screenshots with reference images (golden testing).

**Acceptance Criteria:**
- [ ] `assert_snapshot(name, emulator)` macro
- [ ] Pixel-perfect comparison
- [ ] Threshold-based comparison (allow minor differences)
- [ ] Diff image generation on failure
- [ ] Integration with `insta` crate

**Implementation:**
```rust
use insta::assert_snapshot;
use image::{GrayImage, Luma};

pub fn assert_screenshot_matches(
    emulator: &Emulator,
    reference_path: &str,
    threshold: f32,
) -> Result<()> {
    let actual = emulator.to_grayscale_image();
    let reference = image::open(reference_path)?.to_luma8();

    let diff = compare_images(&actual, &reference);

    if diff > threshold {
        // Save diff image
        let diff_img = generate_diff_image(&actual, &reference);
        diff_img.save("tests/screenshots/diff.png")?;

        panic!("Screenshot mismatch: diff = {:.2}%", diff * 100.0);
    }

    Ok(())
}

fn compare_images(a: &GrayImage, b: &GrayImage) -> f32 {
    if a.dimensions() != b.dimensions() {
        return 1.0;  // 100% different
    }

    let total_pixels = (a.width() * a.height()) as f32;
    let different_pixels = a.pixels()
        .zip(b.pixels())
        .filter(|(pa, pb)| pa.0[0] != pb.0[0])
        .count() as f32;

    different_pixels / total_pixels
}
```

**Usage:**
```rust
#[test]
fn test_now_playing_screen() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);
    NowPlayingScreen::render(&mut emulator, &mock_state()).unwrap();

    assert_screenshot_matches(&emulator, "tests/screenshots/now_playing.png", 0.01).unwrap();
}
```

---

### 5.4: Assertion Helpers
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Assertion helpers for common test scenarios.

**Acceptance Criteria:**
- [ ] `assert_visible(test_id)` - element is visible
- [ ] `assert_hidden(test_id)` - element is not visible
- [ ] `assert_text_contains(test_id, text)` - text content check
- [ ] `assert_bounds(test_id, expected_bounds)` - position check
- [ ] `assert_color(test_id, expected_color)` - color check

**Implementation:**
```rust
pub trait EmulatorAssertions {
    fn assert_visible(&self, test_id: &str);
    fn assert_hidden(&self, test_id: &str);
    fn assert_text_contains(&self, test_id: &str, text: &str);
    fn assert_bounds(&self, test_id: &str, expected: Rectangle);
}

impl EmulatorAssertions for Emulator {
    fn assert_visible(&self, test_id: &str) {
        let node = self.get_by_test_id(test_id);
        assert!(
            node.bounds.size.width > 0 && node.bounds.size.height > 0,
            "Element '{}' is not visible", test_id
        );
    }

    fn assert_text_contains(&self, test_id: &str, text: &str) {
        let node = self.get_by_test_id(test_id);
        if let NodeType::Text(content) = &node.node_type {
            assert!(
                content.contains(text),
                "Element '{}' does not contain text '{}'", test_id, text
            );
        } else {
            panic!("Element '{}' is not a Text node", test_id);
        }
    }
}
```

---

### 5.5: Mock Input System
**Effort:** 5 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Simulate user inputs for interaction testing.

**Acceptance Criteria:**
- [ ] `tap(point)` - simulate tap at coordinates
- [ ] `tap_by_test_id(test_id)` - tap element by ID
- [ ] `key_press(key)` - simulate key press
- [ ] `swipe(from, to)` - simulate swipe gesture
- [ ] Input queue for async execution

**Implementation:**
```rust
impl Emulator {
    pub fn tap(&mut self, point: Point) -> Result<()> {
        // Find element at point
        if let Some(node) = self.node_at_point(point) {
            // Trigger on_press callback if button
            if let NodeType::Button { on_press, .. } = &node.node_type {
                if let Some(callback) = on_press {
                    callback();
                }
            }
        }
        Ok(())
    }

    pub fn tap_by_test_id(&mut self, test_id: &str) -> Result<()> {
        let node = self.get_by_test_id(test_id);
        let center = node.bounds.center();
        self.tap(center)
    }
}
```

**Usage:**
```rust
#[test]
fn test_button_interaction() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);
    let mut clicked = false;

    let button = Button::new("Click")
        .test_id("btn")
        .on_press(|| clicked = true);

    button.draw(&mut emulator, emulator.bounding_box()).unwrap();
    emulator.tap_by_test_id("btn").unwrap();

    assert!(clicked);
}
```

---

### 5.6: Visual Regression Test Framework
**Effort:** 6 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Framework for visual regression testing with `insta`.

**Acceptance Criteria:**
- [ ] Integration with `insta` crate
- [ ] `assert_snapshot!` macro wrapper
- [ ] Automatic screenshot naming
- [ ] Review workflow (`cargo insta review`)
- [ ] CI integration

**Setup:**
```toml
# Cargo.toml
[dev-dependencies]
insta = { version = "1.34", features = ["redactions"] }
```

**Usage:**
```rust
use eink_testing::assert_ui_snapshot;

#[test]
fn test_ui_regression() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);
    render_my_ui(&mut emulator);

    assert_ui_snapshot!("my_ui", emulator);
    // First run: creates snapshot
    // Subsequent runs: compares against snapshot
    // If different: cargo insta review
}
```

---

### 5.7: E-Ink Behavior Tests
**Effort:** 4 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Test helpers for e-ink specific behaviors.

**Acceptance Criteria:**
- [ ] `assert_ghosting_level(min, max)` - ghosting in range
- [ ] `assert_refresh_count(full, partial)` - refresh counts
- [ ] `assert_last_refresh_time(expected)` - timing verification
- [ ] Temperature effect tests

**Implementation:**
```rust
pub trait EinkAssertions {
    fn assert_ghosting_level(&self, min: f32, max: f32);
    fn assert_full_refresh_count(&self, expected: usize);
    fn assert_partial_refresh_count(&self, expected: usize);
}

impl EinkAssertions for Emulator {
    fn assert_ghosting_level(&self, min: f32, max: f32) {
        let level = self.ghosting_level();
        assert!(
            level >= min && level <= max,
            "Ghosting level {} not in range [{}, {}]", level, min, max
        );
    }

    fn assert_full_refresh_count(&self, expected: usize) {
        assert_eq!(
            self.stats.full_refresh_count,
            expected,
            "Expected {} full refreshes, got {}",
            expected,
            self.stats.full_refresh_count
        );
    }
}
```

**Usage:**
```rust
#[tokio::test]
async fn test_ghosting_accumulation() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);

    for _ in 0..5 {
        emulator.refresh_partial().await.unwrap();
    }

    emulator.assert_ghosting_level(0.6, 0.8);
    emulator.assert_partial_refresh_count(5);

    emulator.refresh_full().await.unwrap();
    emulator.assert_ghosting_level(0.0, 0.0);
}
```

---

### 5.8: Test Fixtures
**Effort:** 3 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Reusable test fixtures for common scenarios.

**Acceptance Criteria:**
- [ ] `mock_playback_state()` - sample playback data
- [ ] `mock_track()` - sample track info
- [ ] `mock_library()` - sample music library
- [ ] `default_theme()` - test theme

---

### 5.9: Headless CI Integration
**Effort:** 3 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Ensure tests run in CI without display server.

**Acceptance Criteria:**
- [ ] All tests pass in headless mode
- [ ] GitHub Actions workflow
- [ ] Screenshot artifacts uploaded on failure
- [ ] Fast execution (< 5 min for full suite)

**GitHub Actions:**
```yaml
# .github/workflows/visual-tests.yml
name: Visual Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Run visual tests
        run: cargo test -p eink-testing --all-features

      - name: Upload screenshot diffs on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: screenshot-diffs
          path: tests/screenshots/diff/
```

---

### 5.10: Performance Benchmarks
**Effort:** 4 hours | **Priority:** Low | **Status:** ⏳ Pending

**Description:**
Benchmarks for rendering and layout performance.

**Acceptance Criteria:**
- [ ] Layout computation benchmarks
- [ ] Rendering benchmarks
- [ ] Memory usage profiling
- [ ] Regression detection

**Using criterion:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_layout(c: &mut Criterion) {
    c.bench_function("vstack_layout", |b| {
        let children = [&Text::new("A"), &Text::new("B")];
        let stack = VStack::new(&children);

        b.iter(|| {
            stack.layout(
                black_box(Constraints::loose(Size::new(200, 400))),
                black_box(&theme),
            )
        });
    });
}

criterion_group!(benches, bench_layout);
criterion_main!(benches);
```

---

### 5.11: Documentation & Testing Guide
**Effort:** 5 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Comprehensive testing guide and examples.

**Acceptance Criteria:**
- [ ] Testing guide (markdown)
- [ ] Best practices documented
- [ ] Example test suite
- [ ] CI setup instructions

**Guide Sections:**
1. Introduction to eink-testing
2. Test ID conventions
3. Screenshot testing workflow
4. Visual regression with insta
5. E-ink behavior testing
6. CI integration
7. Troubleshooting

---

### 5.12: Example Test Suite
**Effort:** 4 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Complete example test suite demonstrating all features.

**Test Files:**
1. `unit_tests.rs` - Component unit tests
2. `integration_tests.rs` - Full screen tests
3. `visual_regression_tests.rs` - Screenshot tests
4. `eink_behavior_tests.rs` - Ghosting, refresh tests
5. `interaction_tests.rs` - Input simulation tests

---

## Phase Completion Checklist

- [ ] All tasks completed
- [ ] Test ID system working
- [ ] Screenshot comparison reliable
- [ ] All assertion helpers implemented
- [ ] Headless CI passing
- [ ] Documentation complete
- [ ] Example test suite provided
- [ ] Code review complete

---

**Dependencies for Next Phase:**
- Phase 6 (Polish) can begin after core testing infrastructure is ready

**Estimated Completion:** End of Week 6
