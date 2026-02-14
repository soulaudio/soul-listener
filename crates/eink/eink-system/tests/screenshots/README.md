# Screenshot Test Reference Images

This directory contains reference screenshots for visual regression testing of the eink-system layout engine.

## Directory Structure

```
screenshots/
├── README.md          # This file
├── reference/         # Reference (expected) screenshots
└── actual/           # Generated screenshots during test runs
```

## Screenshot Tests

Each screenshot validates a specific layout scenario:

### Basic Layouts

| Screenshot | Description | Tests |
|------------|-------------|-------|
| `vstack_basic.png` | Simple VStack with 3 children | Basic vertical stacking, gap spacing, child rendering |
| `hstack_space_between.png` | HStack with SpaceBetween justification | Horizontal layout, space distribution |
| `nested_vstack_hstack.png` | Nested VStack inside HStack | Layout composition, nesting behavior |

### Complex Layouts

| Screenshot | Description | Tests |
|------------|-------------|-------|
| `dap_layout.png` | DAP-style UI (header, content, footer) | Real-world layout, multiple containers, typical e-ink UI |
| `responsive_layout.png` | Responsive two-column layout | Percentage-based sizing, adaptive layouts |

### Layout Properties

| Screenshot | Description | Tests |
|------------|-------------|-------|
| `justify_content_modes.png` | All JustifyContent modes | Start, Center, End, SpaceBetween, SpaceAround, SpaceEvenly |
| `align_items_modes.png` | All AlignItems modes | Start, Center, End, Stretch |
| `gap_spacing.png` | Different gap values | Gap: 0, 4, 8, 16 pixels |
| `margin_padding.png` | Margin and padding examples | None, margin-only, padding-only, both |

## Regenerating Reference Screenshots

When you make intentional changes to the layout engine that affect rendering, you need to update the reference screenshots:

```bash
# Regenerate all reference screenshots
UPDATE_SCREENSHOTS=1 cargo test --test e2e_layout

# This will:
# 1. Run all tests
# 2. Save screenshots to reference/ instead of actual/
# 3. Skip comparison (since references are being updated)
```

**⚠️ Warning:** Only regenerate references after verifying that:
1. The visual changes are intentional
2. The new rendering is correct
3. You've reviewed the actual screenshots before promoting them to references

## Running Screenshot Tests

Normal test runs compare generated screenshots against references:

```bash
# Run all E2E layout tests
cargo test --test e2e_layout

# Run specific test
cargo test --test e2e_layout test_simple_vstack_three_children

# Run with output
cargo test --test e2e_layout -- --nocapture
```

## Difference Threshold

The tests use a **1% pixel difference threshold** (`PIXEL_DIFF_THRESHOLD = 0.01`).

This allows for:
- Minor anti-aliasing variations
- Font rendering differences across platforms
- Floating-point rounding in layout calculations

If a test fails with >1% difference:
1. Check `actual/` directory for the generated screenshot
2. Compare visually with `reference/` screenshot
3. Determine if the difference is:
   - **Intentional** → Regenerate references with `UPDATE_SCREENSHOTS=1`
   - **Bug** → Fix the layout engine and re-run tests
   - **Platform variation** → Consider increasing threshold or platform-specific references

## Viewing Screenshots

All screenshots are grayscale PNG files at 250x122 pixels (Waveshare 2.13" V4 display resolution).

To view:
```bash
# Using system image viewer
xdg-open tests/screenshots/reference/vstack_basic.png

# Or on Windows
start tests/screenshots/reference/vstack_basic.png

# Or on macOS
open tests/screenshots/reference/vstack_basic.png
```

## CI/CD Integration

In continuous integration:
1. Tests run in headless mode (no window)
2. Screenshots are generated to `actual/`
3. Compared against committed `reference/` images
4. Build fails if difference > 1%

To skip screenshot tests in CI (if needed):
```bash
cargo test --test e2e_layout -- --skip screenshot
```

## Troubleshooting

### Test fails with "Reference screenshot not found"

**Solution:** Run `UPDATE_SCREENSHOTS=1 cargo test --test e2e_layout` to create initial references.

### Test fails with "X% difference (threshold: 1%)"

**Diagnosis:**
1. Check `actual/` directory for the failed screenshot
2. Compare visually with `reference/`
3. Identify the visual difference

**Solutions:**
- If expected: `UPDATE_SCREENSHOTS=1 cargo test --test e2e_layout`
- If bug: Fix layout engine code
- If platform-specific: Consider platform-specific reference directories

### Screenshots look wrong/corrupted

**Possible causes:**
- Emulator framebuffer not initialized properly
- Drawing operations executed in wrong order
- Coordinate system mismatch

**Debug:**
1. Run test with `--nocapture` to see console output
2. Check emulator initialization in test
3. Verify drawing primitives are correct

### Difference is exactly at threshold (0.99% vs 1.00%)

This is a **flaky test** - minor platform variations causing inconsistency.

**Solutions:**
- Slightly increase threshold (e.g., to 1.5%)
- Make test more deterministic (avoid anti-aliasing, use exact pixel sizes)
- Use platform-specific references

## Best Practices

### When Adding New Tests

1. **Write the test first** with visual output
2. **Run with UPDATE_SCREENSHOTS=1** to create reference
3. **Inspect reference screenshot** visually
4. **Commit reference** to git
5. **Run test normally** to verify it passes

### When Modifying Layout Engine

1. **Run tests first** to establish baseline
2. **Make changes** to layout code
3. **Run tests again** - expect failures
4. **Inspect actual/ screenshots** - are changes correct?
5. **If correct**: Regenerate references
6. **If incorrect**: Fix code and repeat

### Screenshot Naming Convention

- Use snake_case: `feature_variant.png`
- Be descriptive: `vstack_gap_8px.png` not `test1.png`
- Group related tests: `justify_content_*.png`

## Related Documentation

- [eink-system Layout Engine](../README.md)
- [Testing Guide](../../../../docs/TESTING.md)
- [Visual Regression Testing](https://github.com/embedded-graphics/embedded-graphics-simulator#screenshot-testing)

---

**Last Updated:** 2026-02-14
**Maintained by:** eink-system developers
