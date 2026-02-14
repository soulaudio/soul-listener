# Phase 3: Components (eink-components)

**Duration:** Week 3-4 (60 hours)
**Status:** ⏳ Pending
**Dependencies:** Phase 2 (Tasks 2.1-2.10)

## Overview

Build standard reusable UI components that implement the View trait and compile to embedded-graphics primitives. All components must support test IDs and work in no_std environments.

---

## Tasks

### 3.1: Container - VStack (Vertical Stack)
**Effort:** 4 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Vertical container that stacks children top-to-bottom.

**Acceptance Criteria:**
- [ ] `VStack` struct with children array
- [ ] Spacing property
- [ ] Alignment property (leading, center, trailing)
- [ ] Test ID support
- [ ] no_std compatible (heapless Vec or static arrays)
- [ ] Implements View trait

**Implementation:**
```rust
pub struct VStack<'a> {
    children: &'a [&'a dyn View],
    spacing: u32,
    alignment: HorizontalAlignment,
    style: Style,
    test_id: Option<&'a str>,
}

impl<'a> VStack<'a> {
    pub fn new(children: &'a [&'a dyn View]) -> Self {
        Self {
            children,
            spacing: 0,
            alignment: HorizontalAlignment::Leading,
            style: Style::default(),
            test_id: None,
        }
    }

    pub fn spacing(mut self, spacing: u32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn alignment(mut self, alignment: HorizontalAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn test_id(mut self, id: &'a str) -> Self {
        self.test_id = Some(id);
        self
    }
}

impl<'a> View for VStack<'a> {
    fn layout(&self, constraints: Constraints, theme: &Theme) -> LayoutNode {
        let mut y = self.style.padding.top;
        let mut children_layout = heapless::Vec::<LayoutNode, 32>::new();

        for child in self.children {
            let child_layout = child.layout(constraints, theme);
            children_layout.push(child_layout).ok();
            y += child_layout.bounds.size.height + self.spacing;
        }

        LayoutNode {
            bounds: Rectangle::new(
                Point::zero(),
                Size::new(constraints.max_width, y + self.style.padding.bottom),
            ),
            children: children_layout,
        }
    }

    fn test_id(&self) -> Option<&str> {
        self.test_id
    }
}
```

**Tests:**
```rust
#[test]
fn test_vstack_layout() {
    let children = [&Text::new("A"), &Text::new("B")];
    let stack = VStack::new(&children).spacing(8);
    let layout = stack.layout(Constraints::loose(Size::new(200, 400)), &theme);
    // Assert spacing is correct
}
```

---

### 3.2: Container - HStack (Horizontal Stack)
**Effort:** 4 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Horizontal container that stacks children left-to-right.

**Acceptance Criteria:**
- [ ] `HStack` struct with children array
- [ ] Spacing property
- [ ] Alignment property (top, center, bottom)
- [ ] Test ID support
- [ ] Implements View trait

**Similar to VStack but horizontal axis**

---

### 3.3: Container - ZStack (Layered Stack)
**Effort:** 3 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Layered container where children overlap (z-index stacking).

**Acceptance Criteria:**
- [ ] `ZStack` struct
- [ ] Children rendered back-to-front
- [ ] Alignment property
- [ ] Test ID support

**Use Cases:**
- Overlays
- Badges on icons
- Background images

---

### 3.4: Container - Spacer
**Effort:** 1 hour | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Empty space component for layout spacing.

**Acceptance Criteria:**
- [ ] `Spacer` struct with optional width/height
- [ ] Flexible (expands to fill space)
- [ ] Fixed size option

**Implementation:**
```rust
pub struct Spacer {
    size: Option<Size>,
}

impl Spacer {
    pub fn new(size: u32) -> Self {
        Self { size: Some(Size::new(size, size)) }
    }

    pub fn flexible() -> Self {
        Self { size: None }
    }
}
```

---

### 3.5: Text Component
**Effort:** 6 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Text rendering component with styling support.

**Acceptance Criteria:**
- [ ] `Text` struct with content string
- [ ] Font size, weight, color properties
- [ ] Text alignment (left, center, right)
- [ ] Line wrapping support
- [ ] Truncation with ellipsis
- [ ] Test ID support
- [ ] Embedded-graphics font rendering

**Implementation:**
```rust
pub struct Text<'a> {
    content: &'a str,
    font_size: u32,
    font_weight: FontWeight,
    color: Gray4,
    alignment: TextAlign,
    max_lines: Option<usize>,
    test_id: Option<&'a str>,
}

impl<'a> Text<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            font_size: 14,
            font_weight: FontWeight::Normal,
            color: Gray4::BLACK,
            alignment: TextAlign::Left,
            max_lines: None,
            test_id: None,
        }
    }

    pub fn font_size(mut self, size: u32) -> Self {
        self.font_size = size;
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    pub fn test_id(mut self, id: &'a str) -> Self {
        self.test_id = Some(id);
        self
    }
}

impl<'a> View for Text<'a> {
    fn draw<D>(&self, target: &mut D, bounds: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        use embedded_graphics::text::Text as EgText;
        use embedded_graphics::mono_font::MonoTextStyle;

        let style = MonoTextStyle::new(&FONT_6X10, self.color);
        EgText::new(self.content, bounds.top_left(), style)
            .draw(target)?;

        Ok(())
    }
}
```

---

### 3.6: Button Component
**Effort:** 5 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Interactive button with label and optional icon.

**Acceptance Criteria:**
- [ ] `Button` struct with label
- [ ] Optional icon
- [ ] Press state visualization
- [ ] Disabled state
- [ ] Callback support (function pointer)
- [ ] Test ID support
- [ ] Consistent padding and sizing

**Implementation:**
```rust
pub struct Button<'a> {
    label: &'a str,
    icon: Option<Icon>,
    on_press: Option<fn()>,
    disabled: bool,
    style: Style,
    test_id: Option<&'a str>,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            on_press: None,
            disabled: false,
            style: Style::default(),
            test_id: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn on_press(mut self, callback: fn()) -> Self {
        self.on_press = Some(callback);
        self
    }

    pub fn test_id(mut self, id: &'a str) -> Self {
        self.test_id = Some(id);
        self
    }
}
```

**Tests:**
```rust
#[test]
fn test_button_with_test_id() {
    let button = Button::new("Click").test_id("submit-btn");
    assert_eq!(button.test_id(), Some("submit-btn"));
}
```

---

### 3.7: Icon Component
**Effort:** 4 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Icon rendering from embedded bitmaps or font glyphs.

**Acceptance Criteria:**
- [ ] `Icon` enum with common icons (Play, Pause, Next, Prev, Settings, etc.)
- [ ] Size property
- [ ] Color property
- [ ] Test ID support
- [ ] Bitmap rendering

**Icons to include:**
- Play, Pause, Stop
- Next, Prev, FastForward, Rewind
- VolumeUp, VolumeDown, VolumeMute
- Settings, Menu, Back, Close
- Battery, Bluetooth, Folder, File

---

### 3.8: ProgressBar Component
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Linear progress bar with value (0.0 to 1.0).

**Acceptance Criteria:**
- [ ] `ProgressBar` struct with value (f32)
- [ ] Height property
- [ ] Foreground/background colors
- [ ] Test ID support
- [ ] Smooth rendering with dithering

**Implementation:**
```rust
pub struct ProgressBar {
    value: f32,  // 0.0 to 1.0
    height: u32,
    foreground: Gray4,
    background: Gray4,
    test_id: Option<&'static str>,
}

impl View for ProgressBar {
    fn draw<D>(&self, target: &mut D, bounds: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw background
        Rectangle::new(bounds.top_left(), bounds.size())
            .into_styled(PrimitiveStyle::with_fill(self.background))
            .draw(target)?;

        // Draw foreground (progress)
        let progress_width = (bounds.size().width as f32 * self.value) as u32;
        Rectangle::new(
            bounds.top_left(),
            Size::new(progress_width, self.height),
        )
        .into_styled(PrimitiveStyle::with_fill(self.foreground))
        .draw(target)?;

        Ok(())
    }
}
```

---

### 3.9: ListView Component
**Effort:** 8 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Scrollable list of items with virtualization.

**Acceptance Criteria:**
- [ ] `ListView` struct with generic item type
- [ ] Render function: `fn(&T, usize) -> Box<dyn View>`
- [ ] Virtualization (only render visible items)
- [ ] Scroll offset tracking
- [ ] Dividers between items
- [ ] Test ID support
- [ ] no_std compatible

**Implementation:**
```rust
pub struct ListView<'a, T> {
    items: &'a [T],
    render_item: fn(&T, usize, &Theme) -> Box<dyn View + 'a>,
    scroll_offset: u32,
    item_height: u32,
    test_id: Option<&'a str>,
}

impl<'a, T> ListView<'a, T> {
    pub fn new(
        items: &'a [T],
        render_item: fn(&T, usize, &Theme) -> Box<dyn View + 'a>,
    ) -> Self {
        Self {
            items,
            render_item,
            scroll_offset: 0,
            item_height: 40,
            test_id: None,
        }
    }
}
```

**Challenges:**
- Virtualization in no_std (need bounded allocation)
- Scroll physics

---

### 3.10: Image Component
**Effort:** 5 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Display embedded images (bitmaps).

**Acceptance Criteria:**
- [ ] `Image` struct with embedded bitmap
- [ ] Scaling modes (fit, fill, stretch)
- [ ] Grayscale dithering
- [ ] Test ID support

---

### 3.11: Divider Component
**Effort:** 2 hours | **Priority:** Low | **Status:** ⏳ Pending

**Description:**
Horizontal or vertical divider line.

**Acceptance Criteria:**
- [ ] `Divider` struct
- [ ] Horizontal/vertical orientation
- [ ] Thickness property
- [ ] Color property

---

### 3.12: Badge/Label Component
**Effort:** 3 hours | **Priority:** Low | **Status:** ⏳ Pending

**Description:**
Small label with background (for tags, counts, etc.).

**Acceptance Criteria:**
- [ ] `Badge` struct with text
- [ ] Background color
- [ ] Rounded corners
- [ ] Compact sizing

---

### 3.13: Component Unit Tests
**Effort:** 6 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Comprehensive unit tests for all components.

**Acceptance Criteria:**
- [ ] Test each component's layout logic
- [ ] Test test_id() implementation
- [ ] Test builder pattern methods
- [ ] Test rendering (with mock DrawTarget)
- [ ] 80%+ coverage

---

### 3.14: Integration Tests
**Effort:** 4 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Integration tests with complex layouts.

**Test Scenarios:**
- [ ] Nested containers (VStack inside HStack)
- [ ] ListView with complex items
- [ ] Button grid
- [ ] Form layout (labels + inputs)

---

### 3.15: Documentation & Examples
**Effort:** 6 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Complete documentation and example programs.

**Acceptance Criteria:**
- [ ] Rustdoc on all public APIs
- [ ] Component gallery example
- [ ] Music player UI example
- [ ] Settings screen example
- [ ] README with screenshots

**Examples:**
1. `component_gallery.rs` - Show all components
2. `music_player.rs` - Now Playing screen
3. `list_demo.rs` - Scrollable list
4. `forms.rs` - Form layout

---

## Phase Completion Checklist

- [ ] All 12+ components implemented
- [ ] All components support test IDs
- [ ] Tests pass (unit + integration)
- [ ] Coverage > 80%
- [ ] Documentation complete
- [ ] Examples working
- [ ] no_std build verified
- [ ] Code review complete

---

**Dependencies for Next Phase:**
- Phase 5 (Testing) depends on all components having test ID support

**Estimated Completion:** End of Week 4
