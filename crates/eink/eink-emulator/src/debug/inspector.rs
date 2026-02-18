//! Component inspector overlay
//!
//! When inspector mode is active (Ctrl+3) the inspector draws a tooltip over
//! whatever component the cursor is hovering.  The tooltip is organised in
//! three tabs that can be cycled with Tab / Shift+Tab:
//!
//! - **Layout** (default): position, size, right/bottom bounds
//! - **BoxModel**: margin / border / padding / content visualisation
//! - **Component**: type name, test ID, key-value attributes
//!
//! # Rendering
//!
//! The inspector renders directly into the shared ARGB pixel buffer using an
//! embedded-graphics `DrawTarget` canvas scoped to the tooltip region.
//!
//! # Example
//!
//! ```rust
//! use eink_emulator::debug::inspector::{Inspector, InspectorTab};
//! use eink_emulator::debug::state::{ComponentInfo, DebugState};
//!
//! let mut inspector = Inspector::new();
//! inspector.set_tab(InspectorTab::BoxModel);
//!
//! let component = ComponentInfo {
//!     component_type: "Button".to_string(),
//!     position: (10, 20),
//!     size: (100, 40),
//!     test_id: Some("play-button".to_string()),
//!     ..Default::default()
//! };
//!
//! let mut buffer = vec![0u32; 800 * 600];
//! inspector.render_details(&mut buffer, 800, 10, 10, &component, &DebugState::default());
//! ```

use std::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
};

use super::state::ComponentInfo;

// ---------------------------------------------------------------------------
// Colours
// ---------------------------------------------------------------------------

const TOOLTIP_BG: u32 = 0xEE192434; // Dark navy, slightly transparent
const TOOLTIP_BORDER: u32 = 0xFF4488CC; // Blue border
const DIVIDER: u32 = 0xFF2A3D55;

const COL_TITLE: Rgb888 = Rgb888::new(0xCC, 0xDD, 0xFF);
const COL_KEY: Rgb888 = Rgb888::new(0x88, 0xAA, 0xBB);
const COL_VALUE: Rgb888 = Rgb888::new(0xFF, 0xFF, 0xFF);
const COL_TAB_ON: Rgb888 = Rgb888::new(0x55, 0xCC, 0xFF);
const COL_TAB_OFF: Rgb888 = Rgb888::new(0x44, 0x55, 0x66);

// Layout constants
pub const TOOLTIP_W: u32 = 160;
pub const TOOLTIP_H: u32 = 120;
const PAD: i32 = 3;
const LH: i32 = 11; // Line height

// Box-model zone colours (ARGB)
const MARGIN_FILL:  u32 = 0x60FFA040;
const BORDER_FILL:  u32 = 0x60FFD040;
const PADDING_FILL: u32 = 0x6050CC50;
const CONTENT_FILL: u32 = 0x604090E0;
const MARGIN_LINE:  u32 = 0xFFDD7020;
const BORDER_LINE:  u32 = 0xFFCCAA20;
const PADDING_LINE: u32 = 0xFF30AA30;
const CONTENT_LINE: u32 = 0xFF2070CC;

// ---------------------------------------------------------------------------
// Canvas that wraps a rectangular region of the main pixel buffer
// ---------------------------------------------------------------------------

struct Canvas<'buf> {
    buf: &'buf mut [u32],
    screen_w: u32,
    x_off: u32,
    y_off: u32,
    width: u32,
    height: u32,
}

impl<'buf> Canvas<'buf> {
    fn new(
        buf: &'buf mut [u32],
        screen_w: u32,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Self {
        Self {
            buf,
            screen_w,
            x_off: x,
            y_off: y,
            width: w,
            height: h,
        }
    }

    fn set_px(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            let idx = ((self.y_off + y) * self.screen_w + self.x_off + x) as usize;
            if idx < self.buf.len() {
                // Simple alpha blend for semi-transparent backgrounds
                let a = (color >> 24) & 0xFF;
                if a == 0xFF {
                    self.buf[idx] = color;
                } else {
                    let src = color & 0x00FF_FFFF;
                    let dst = self.buf[idx] & 0x00FF_FFFF;
                    let alpha = a;
                    let inv_alpha = 255 - alpha;
                    let r = ((src >> 16 & 0xFF) * alpha + (dst >> 16 & 0xFF) * inv_alpha) / 255;
                    let g = ((src >> 8 & 0xFF) * alpha + (dst >> 8 & 0xFF) * inv_alpha) / 255;
                    let b = ((src & 0xFF) * alpha + (dst & 0xFF) * inv_alpha) / 255;
                    self.buf[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for dy in 0..h {
            for dx in 0..w {
                self.set_px(x + dx, y + dy, color);
            }
        }
    }

    fn hline(&mut self, y: u32, color: u32) {
        for dx in 0..self.width {
            self.set_px(dx, y, color);
        }
    }

    fn border(&mut self, color: u32) {
        let w = self.width;
        let h = self.height;
        for x in 0..w {
            self.set_px(x, 0, color);
            self.set_px(x, h - 1, color);
        }
        for y in 0..h {
            self.set_px(0, y, color);
            self.set_px(w - 1, y, color);
        }
    }

    fn rect_outline(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        for i in 0..w {
            self.set_px(x + i, y, color);
            self.set_px(x + i, y + h.saturating_sub(1), color);
        }
        for i in 0..h {
            self.set_px(x, y + i, color);
            self.set_px(x + w.saturating_sub(1), y + i, color);
        }
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.y >= 0 {
                let (r, g, b) = (c.r() as u32, c.g() as u32, c.b() as u32);
                self.set_px(p.x as u32, p.y as u32, 0xFF000000 | (r << 16) | (g << 8) | b);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Canvas<'_> {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

/// Draw a line of text into the canvas at (PAD, y)
fn txt(canvas: &mut Canvas, y: i32, text: &str, color: Rgb888) {
    let style = MonoTextStyle::new(&FONT_6X10, color);
    Text::new(text, Point::new(PAD, y), style).draw(canvas).ok();
}

/// Draw a key-value pair: key in dim colour, value in bright colour
fn kv(canvas: &mut Canvas, y: i32, key: &str, value: &str) {
    let key_style = MonoTextStyle::new(&FONT_6X10, COL_KEY);
    let val_style = MonoTextStyle::new(&FONT_6X10, COL_VALUE);
    let key_px = (key.len() as i32) * 6; // 6px per char in FONT_6X10
    Text::new(key, Point::new(PAD, y), key_style).draw(canvas).ok();
    Text::new(value, Point::new(PAD + key_px, y), val_style)
        .draw(canvas)
        .ok();
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Inspector tab types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    /// Layout properties (position, size, bounds)
    Layout,
    /// Box model (margin / border / padding / content visualisation)
    BoxModel,
    /// Component metadata (type, test ID, attributes)
    Component,
}

/// Component inspector with tooltip rendering
pub struct Inspector {
    current_tab: InspectorTab,
}

impl Inspector {
    /// Create a new inspector defaulting to the Layout tab.
    pub fn new() -> Self {
        Self {
            current_tab: InspectorTab::Layout,
        }
    }

    /// Switch to a different tab.
    pub fn set_tab(&mut self, tab: InspectorTab) {
        self.current_tab = tab;
    }

    /// Return the currently active tab.
    pub fn current_tab(&self) -> InspectorTab {
        self.current_tab
    }

    /// Cycle to the next tab: Layout → BoxModel → Component → Layout.
    pub fn next_tab(&mut self) {
        self.current_tab = match self.current_tab {
            InspectorTab::Layout   => InspectorTab::BoxModel,
            InspectorTab::BoxModel => InspectorTab::Component,
            InspectorTab::Component => InspectorTab::Layout,
        };
    }

    /// Render a component-inspector tooltip into `buffer` at position (`x`, `y`).
    ///
    /// The tooltip is `TOOLTIP_W × TOOLTIP_H` pixels and is automatically
    /// clamped to the screen bounds.  The content depends on [`current_tab`].
    ///
    /// # Arguments
    ///
    /// * `buffer`       – ARGB pixel buffer (one pixel per display pixel)
    /// * `screen_width` – Row stride of `buffer`
    /// * `x`, `y`       – Top-left corner of the tooltip in display pixel coords
    /// * `component`    – Component information to show
    pub fn render_details(
        &self,
        buffer: &mut [u32],
        screen_width: u32,
        x: u32,
        y: u32,
        component: &ComponentInfo,
        _state: &super::state::DebugState,
    ) {
        let w = TOOLTIP_W;
        let h = TOOLTIP_H;

        // Clamp so the tooltip stays within the buffer width
        let tx = x.min(screen_width.saturating_sub(w));
        let ty = y;

        let mut canvas = Canvas::new(buffer, screen_width, tx, ty, w, h);

        // ── background & border ──────────────────────────────────────────
        canvas.fill_rect(0, 0, w, h, TOOLTIP_BG);
        canvas.border(TOOLTIP_BORDER);

        // ── tab bar ──────────────────────────────────────────────────────
        let tabs = [("LYT", InspectorTab::Layout), ("BOX", InspectorTab::BoxModel), ("CMP", InspectorTab::Component)];
        let tab_w = (w - 2) / 3;
        for (i, (label, tab)) in tabs.iter().enumerate() {
            let tab_x = 1 + i as u32 * tab_w;
            let is_active = *tab == self.current_tab;
            let bg = if is_active { 0xFF1B3E6A } else { 0xFF111820 };
            canvas.fill_rect(tab_x, 1, tab_w, 9, bg);
            let color = if is_active { COL_TAB_ON } else { COL_TAB_OFF };
            let style = MonoTextStyle::new(&FONT_6X10, color);
            Text::new(label, Point::new(tab_x as i32 + 2, 9), style)
                .draw(&mut canvas)
                .ok();
        }
        canvas.hline(10, DIVIDER);

        // ── content (starts at y = 12) ───────────────────────────────────
        let mut cy = 12i32;

        match self.current_tab {
            InspectorTab::Layout => {
                txt(&mut canvas, cy, "LAYOUT", COL_TITLE);
                cy += LH;
                kv(
                    &mut canvas,
                    cy,
                    "pos ",
                    &format!("({}, {})", component.position.0, component.position.1),
                );
                cy += LH;
                kv(
                    &mut canvas,
                    cy,
                    "sz  ",
                    &format!("{}×{}", component.size.0, component.size.1),
                );
                cy += LH;
                let right = component.position.0 + component.size.0 as i32;
                let bottom = component.position.1 + component.size.1 as i32;
                kv(
                    &mut canvas,
                    cy,
                    "br  ",
                    &format!("({}, {})", right, bottom),
                );
            }

            InspectorTab::BoxModel => {
                // Nested box diagram in the upper portion (y=12..84)
                // Layer order: margin → border → padding → content (back to front)
                canvas.fill_rect(2,  12, 156, 72, MARGIN_FILL);
                canvas.fill_rect(14, 24, 132, 48, BORDER_FILL);
                canvas.fill_rect(16, 26, 128, 44, PADDING_FILL);
                canvas.fill_rect(26, 36, 108, 24, CONTENT_FILL);

                canvas.rect_outline(2,  12, 156, 72, MARGIN_LINE);
                canvas.rect_outline(14, 24, 132, 48, BORDER_LINE);
                canvas.rect_outline(16, 26, 128, 44, PADDING_LINE);
                canvas.rect_outline(26, 36, 108, 24, CONTENT_LINE);

                // Zone labels
                let mar_style  = MonoTextStyle::new(&FONT_6X10, Rgb888::new(0xDD, 0x70, 0x20));
                let pad_style  = MonoTextStyle::new(&FONT_6X10, Rgb888::new(0x30, 0xAA, 0x30));
                let cont_style = MonoTextStyle::new(&FONT_6X10, Rgb888::new(0x40, 0x90, 0xE0));
                Text::new("margin",  Point::new(4, 21), mar_style).draw(&mut canvas).ok();
                Text::new("padding", Point::new(18, 35), pad_style).draw(&mut canvas).ok();

                // Content size centred in content box
                let cont_label = if component.size.0 > 0 && component.size.1 > 0 {
                    format!("{}×{}", component.size.0, component.size.1)
                } else {
                    "- × -".to_string()
                };
                let cont_x = (26 + (108i32 - cont_label.len() as i32 * 6) / 2).max(26);
                Text::new(&cont_label, Point::new(cont_x, 50), cont_style).draw(&mut canvas).ok();

                // Compact value table (y=88..119, 3 rows)
                let dim_style = MonoTextStyle::new(&FONT_6X10, COL_KEY);
                let val_style = MonoTextStyle::new(&FONT_6X10, COL_VALUE);

                let table_y = [88i32, 99, 110];
                let labels  = ["mar", "brd", "pad"];
                let spacings = [component.margin, component.border, component.padding];

                for ((row_y, lbl), sp) in table_y.iter().zip(labels.iter()).zip(spacings.iter()) {
                    Text::new(lbl, Point::new(PAD, *row_y), dim_style).draw(&mut canvas).ok();
                    let row = format!("{:>3}{:>3}{:>3}{:>3}", sp.top, sp.right, sp.bottom, sp.left);
                    Text::new(&row, Point::new(PAD + 18, *row_y), val_style).draw(&mut canvas).ok();
                }
            }

            InspectorTab::Component => {
                txt(&mut canvas, cy, "COMPONENT", COL_TITLE);
                cy += LH;
                txt(&mut canvas, cy, &component.component_type, COL_VALUE);
                cy += LH;
                let id_str = component
                    .test_id
                    .as_deref()
                    .unwrap_or("(none)");
                // Truncate long IDs to fit within tooltip width
                let max_chars = ((w as i32 - PAD * 2) / 6).max(1) as usize;
                let truncated: String = if id_str.len() > max_chars {
                    format!("{}..", &id_str[..max_chars.saturating_sub(2)])
                } else {
                    id_str.to_string()
                };
                kv(&mut canvas, cy, "id  ", &truncated);
                cy += LH;

                // Attributes
                if !component.attributes.is_empty() {
                    let badge_y = h as i32 - 12;
                    canvas.hline(cy as u32, DIVIDER);
                    cy += 2;
                    let max_rows = ((badge_y - cy) / LH).max(0) as usize;
                    for (k, v) in component.attributes.iter().take(max_rows) {
                        kv(&mut canvas, cy, &format!("{}: ", k), v);
                        cy += LH;
                    }
                }
            }
        }

        // ── bottom type badge ─────────────────────────────────────────────
        let badge_y = h - 12;
        canvas.hline(badge_y, DIVIDER);
        let abbr = match component.component_type.as_str() {
            "Container" => "CTN",
            "Button" => "BTN",
            "Label" => "LBL",
            "ProgressBar" => "PBR",
            other => &other[..other.len().min(3)],
        };
        let abbr_style = MonoTextStyle::new(&FONT_6X10, COL_TAB_ON);
        let rest_style = MonoTextStyle::new(&FONT_6X10, COL_KEY);
        Text::new(abbr, Point::new(PAD, (badge_y + 10) as i32), abbr_style)
            .draw(&mut canvas)
            .ok();
        Text::new(
            &format!("  {}", component.component_type),
            Point::new(PAD, (badge_y + 10) as i32),
            rest_style,
        )
        .draw(&mut canvas)
        .ok();

        // Re-stamp border so it is always on top
        canvas.border(TOOLTIP_BORDER);
    }
}

impl Default for Inspector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_component() -> ComponentInfo {
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (10, 20),
            size: (100, 40),
            test_id: Some("test-button".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_inspector_creation() {
        let inspector = Inspector::new();
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    }

    #[test]
    fn test_inspector_default() {
        let inspector = Inspector::default();
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    }

    #[test]
    fn test_tab_switching() {
        let mut inspector = Inspector::new();
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
        inspector.set_tab(InspectorTab::Component);
        assert_eq!(inspector.current_tab(), InspectorTab::Component);
        inspector.set_tab(InspectorTab::BoxModel);
        assert_eq!(inspector.current_tab(), InspectorTab::BoxModel);
        inspector.set_tab(InspectorTab::Layout);
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    }

    #[test]
    fn test_next_tab_cycling() {
        let mut inspector = Inspector::new();
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
        inspector.next_tab();
        assert_eq!(inspector.current_tab(), InspectorTab::BoxModel);
        inspector.next_tab();
        assert_eq!(inspector.current_tab(), InspectorTab::Component);
        inspector.next_tab();
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    }

    #[test]
    fn test_render_layout_tab() {
        let inspector = Inspector::new();
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component, &crate::debug::state::DebugState::default());
        // Tooltip background pixels should have been written
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written, "render_details should write pixels to the buffer");
    }

    #[test]
    fn test_render_component_tab() {
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::Component);
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component, &crate::debug::state::DebugState::default());
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_render_box_model_tab() {
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::BoxModel);
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component, &crate::debug::state::DebugState::default());
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_render_box_model_with_spacing() {
        use crate::debug::state::Spacing;
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::BoxModel);
        let component = ComponentInfo {
            component_type: "Button".to_string(),
            position: (10, 20),
            size: (100, 40),
            test_id: Some("test-btn".to_string()),
            margin:  Spacing::all(8),
            padding: Spacing::axes(4, 12),
            border:  Spacing::all(1),
            ..Default::default()
        };
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component, &crate::debug::state::DebugState::default());
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_render_attrs_in_component_tab() {
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::Component);
        let component = ComponentInfo {
            component_type: "Button".to_string(),
            position: (10, 20),
            size: (100, 40),
            test_id: Some("test-btn".to_string()),
            attributes: vec![
                ("index".to_string(), "0".to_string()),
                ("enabled".to_string(), "true".to_string()),
            ],
            ..Default::default()
        };
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component, &crate::debug::state::DebugState::default());
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_render_all_tabs() {
        let mut inspector = Inspector::new();
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        for tab in [InspectorTab::Layout, InspectorTab::BoxModel, InspectorTab::Component] {
            inspector.set_tab(tab);
            inspector.render_details(&mut buffer, 800, 10, 10, &component, &crate::debug::state::DebugState::default());
        }
    }

    #[test]
    fn test_component_without_test_id() {
        let inspector = Inspector::new();
        let component = ComponentInfo {
            component_type: "Text".to_string(),
            position: (0, 0),
            size: (50, 20),
            test_id: None,
            ..Default::default()
        };
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 0, 0, &component, &crate::debug::state::DebugState::default());
    }

    #[test]
    fn test_tooltip_clamped_near_right_edge() {
        let inspector = Inspector::new();
        let component = make_component();
        let screen_w = 200u32;
        let mut buffer = vec![0u32; (screen_w * 200) as usize];
        // Request x near the right edge — tooltip should be clamped
        inspector.render_details(&mut buffer, screen_w, screen_w - 10, 0, &component, &crate::debug::state::DebugState::default());
        // Should not panic, pixels should be written
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_inspector_tab_equality() {
        assert_eq!(InspectorTab::Layout, InspectorTab::Layout);
        assert_ne!(InspectorTab::Layout, InspectorTab::Component);
        assert_ne!(InspectorTab::Component, InspectorTab::BoxModel);
    }

    #[test]
    fn test_inspector_tab_clone() {
        let tab = InspectorTab::Component;
        assert_eq!(tab, tab.clone());
    }

    #[test]
    fn test_inspector_tab_copy() {
        let tab = InspectorTab::BoxModel;
        let copied = tab;
        assert_eq!(tab, copied);
    }

    #[test]
    fn test_inspector_tab_debug() {
        let tab = InspectorTab::Layout;
        let s = format!("{:?}", tab);
        assert!(s.contains("Layout"));
    }
}
