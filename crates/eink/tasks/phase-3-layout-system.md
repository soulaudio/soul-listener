# Phase 2: Core System (eink-system)

**Duration:** Week 2-3 (50 hours)
**Status:** ⏳ Pending
**Dependencies:** Phase 1 (Tasks 1.2, 1.5, 1.7)

## Overview

Build the layout and styling engine - the foundation for all UI components. This is CSS-like flexbox layout system optimized for e-ink displays and no_std environments.

---

## Tasks

### 2.1: Style System - EdgeInsets
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement EdgeInsets for padding and margin with convenient constructors.

**Acceptance Criteria:**
- [ ] `EdgeInsets` struct with `top`, `right`, `bottom`, `left`
- [ ] `all(value)` - uniform insets
- [ ] `symmetric(vertical, horizontal)` - symmetric insets
- [ ] `only(top, right, bottom, left)` - specific insets
- [ ] `vertical(value)` and `horizontal(value)` helpers
- [ ] Derives: `Debug`, `Clone`, `Copy`, `PartialEq`
- [ ] Comprehensive unit tests

**Implementation:**
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl EdgeInsets {
    pub const fn all(value: u32) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
    }

    pub const fn symmetric(vertical: u32, horizontal: u32) -> Self {
        Self { top: vertical, right: horizontal, bottom: vertical, left: horizontal }
    }

    pub const fn only(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self { top, right, bottom, left }
    }

    pub const fn horizontal(&self) -> u32 {
        self.left + self.right
    }

    pub const fn vertical(&self) -> u32 {
        self.top + self.bottom
    }
}
```

**Verification:**
```rust
#[test]
fn test_edge_insets() {
    let insets = EdgeInsets::all(16);
    assert_eq!(insets.horizontal(), 32);
    assert_eq!(insets.vertical(), 32);
}
```

---

### 2.2: Style System - Border & Style Struct
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Define Border and main Style struct for component styling.

**Acceptance Criteria:**
- [ ] `Border` struct (width, color, radius)
- [ ] `Style` struct (padding, margin, background, border, etc.)
- [ ] Builder pattern for Style
- [ ] Default implementation
- [ ] Font-related fields (size, weight, alignment)

**Implementation:**
```rust
#[derive(Debug, Clone, Copy)]
pub struct Border {
    pub width: u32,
    pub color: Gray4,
    pub radius: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub padding: EdgeInsets,
    pub margin: EdgeInsets,
    pub background: Option<Gray4>,
    pub border: Option<Border>,
    pub font_size: u32,
    pub font_weight: FontWeight,
    pub text_align: TextAlign,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            padding: EdgeInsets::all(0),
            margin: EdgeInsets::all(0),
            background: None,
            border: None,
            font_size: 14,
            font_weight: FontWeight::Normal,
            text_align: TextAlign::Left,
        }
    }
}
```

---

### 2.3: Theme System - Color Palette
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Define semantic color palette for theming.

**Acceptance Criteria:**
- [ ] `ColorPalette` struct with semantic colors
- [ ] Background, foreground, primary, secondary, accent
- [ ] Predefined palettes (dark, light, high-contrast)
- [ ] Grayscale optimization for 4-level displays

**Implementation:**
```rust
#[derive(Debug, Clone, Copy)]
pub struct ColorPalette {
    pub background: Gray4,
    pub foreground: Gray4,
    pub primary: Gray4,
    pub secondary: Gray4,
    pub accent: Gray4,
}

impl ColorPalette {
    pub const LIGHT: Self = Self {
        background: Gray4::WHITE,
        foreground: Gray4::BLACK,
        primary: Gray4::new(3),
        secondary: Gray4::new(1),
        accent: Gray4::new(2),
    };

    pub const HIGH_CONTRAST: Self = Self {
        background: Gray4::WHITE,
        foreground: Gray4::BLACK,
        primary: Gray4::BLACK,
        secondary: Gray4::new(2),
        accent: Gray4::BLACK,
    };
}
```

---

### 2.4: Theme System - Typography
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Define typography scale for consistent text styling.

**Acceptance Criteria:**
- [ ] `Typography` struct with heading1, heading2, body, caption
- [ ] `TextStyle` struct (size, weight, color, line_height)
- [ ] Predefined typography scales

---

### 2.5: Theme System - Spacing Scale
**Effort:** 1 hour | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Define consistent spacing scale (xs, sm, md, lg, xl).

**Acceptance Criteria:**
- [ ] `Spacing` struct with standard increments
- [ ] Typical scale: 4, 8, 16, 24, 32

---

### 2.6: Layout System - Constraints
**Effort:** 3 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement constraint-based layout system (min/max width/height).

**Acceptance Criteria:**
- [ ] `Constraints` struct (min_width, max_width, min_height, max_height)
- [ ] `tight(size)` - exact size constraint
- [ ] `loose(size)` - max size constraint
- [ ] `unbounded()` - no constraints
- [ ] `constrain(size)` - clamp size to constraints
- [ ] `enforce(constraints)` - compose constraints

**Implementation:**
```rust
#[derive(Debug, Clone, Copy)]
pub struct Constraints {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl Constraints {
    pub const fn tight(size: Size) -> Self {
        Self {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }

    pub const fn loose(size: Size) -> Self {
        Self {
            min_width: 0,
            max_width: size.width,
            min_height: 0,
            max_height: size.height,
        }
    }

    pub fn constrain(&self, size: Size) -> Size {
        Size::new(
            size.width.max(self.min_width).min(self.max_width),
            size.height.max(self.min_height).min(self.max_height),
        )
    }
}
```

---

### 2.7: Layout System - LayoutNode
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Define LayoutNode - result of layout computation with position and size.

**Acceptance Criteria:**
- [ ] `LayoutNode` struct (bounds, children)
- [ ] Tree structure for nested layouts
- [ ] Efficient allocation (heapless for no_std)

---

### 2.8: Layout Algorithm - Flexbox (Axis)
**Effort:** 6 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement single-axis flexbox layout algorithm.

**Acceptance Criteria:**
- [ ] `FlexDirection` enum (Row, Column)
- [ ] Main axis layout
- [ ] Cross axis alignment
- [ ] Flex sizing (grow, shrink)
- [ ] Gap support
- [ ] no_std compatible (heapless collections)

**Implementation:**
```rust
pub struct FlexLayout {
    pub direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub gap: u32,
}

impl Layout for FlexLayout {
    fn compute(&self, constraints: Constraints) -> Size {
        // Main axis layout algorithm
        // 1. Calculate intrinsic sizes
        // 2. Distribute remaining space
        // 3. Position children
    }
}
```

**Verification:**
- [ ] Test: 3 equal children in row
- [ ] Test: 3 children with flex weights
- [ ] Test: Overflow handling
- [ ] Test: Cross-axis alignment

---

### 2.9: Layout Algorithm - Flexbox (Cross-Axis)
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Implement cross-axis alignment (align-items, align-self).

**Acceptance Criteria:**
- [ ] `AlignItems` enum (Start, Center, End, Stretch)
- [ ] Cross-axis sizing
- [ ] Stretch behavior

---

### 2.10: Text Measurement (Basic)
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Implement basic text measurement for layout calculations.

**Acceptance Criteria:**
- [ ] Measure text width given font size
- [ ] Measure text height (line height)
- [ ] Support for embedded-graphics fonts
- [ ] Monospace fallback for no_std

**Implementation:**
```rust
pub fn measure_text(text: &str, font_size: u32) -> Size {
    // Simplified: assume monospace
    let char_width = font_size / 2;
    let width = text.len() as u32 * char_width;
    let height = font_size;
    Size::new(width, height)
}
```

**Future Enhancement:**
- TrueType font metrics (Phase 6)

---

### 2.11: Integration Tests
**Effort:** 4 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Comprehensive integration tests for layout system.

**Acceptance Criteria:**
- [ ] Test: Nested containers
- [ ] Test: Margin collapsing
- [ ] Test: Constraint propagation
- [ ] Test: Flexbox edge cases
- [ ] 80%+ code coverage

---

### 2.12: Documentation & Examples
**Effort:** 6 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Complete rustdoc and example programs.

**Acceptance Criteria:**
- [ ] Every public API documented
- [ ] Layout algorithm explained
- [ ] 3+ example programs (basic layout, flexbox, nested)
- [ ] Performance characteristics documented

**Examples:**
1. `basic_layout.rs` - Simple VStack with Text
2. `flexbox.rs` - Demonstrate flex properties
3. `nested.rs` - Complex nested layouts

---

## Phase Completion Checklist

- [ ] All tasks completed
- [ ] Tests pass (unit + integration)
- [ ] Coverage > 80%
- [ ] Documentation complete
- [ ] Examples working
- [ ] no_std build verified
- [ ] Code review complete

---

**Dependencies for Next Phase:**
- Phase 3 (Components) depends on:
  - Task 2.1-2.5 (Style and Theme)
  - Task 2.6-2.9 (Layout system)
  - Task 2.10 (Text measurement)

**Estimated Completion:** End of Week 3
