//! Component inspector overlay
//!
//! When inspector mode is active (Ctrl+3) the inspector draws a tooltip over
//! whatever component the cursor is hovering.  The tooltip is organised in
//! three tabs that can be cycled with Tab / Shift+Tab:
//!
//! - **Layout** (default): position, size, right/bottom bounds
//! - **Component**: type name, test ID
//! - **Stats**: placeholder refresh statistics
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
//! use eink_emulator::debug::state::ComponentInfo;
//!
//! let mut inspector = Inspector::new();
//! inspector.set_tab(InspectorTab::Component);
//!
//! let component = ComponentInfo {
//!     component_type: "Button".to_string(),
//!     position: (10, 20),
//!     size: (100, 40),
//!     test_id: Some("play-button".to_string()),
//! };
//!
//! let mut buffer = vec![0u32; 800 * 600];
//! inspector.render_details(&mut buffer, 800, 10, 10, &component);
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
const TOOLTIP_W: u32 = 120;
const TOOLTIP_H: u32 = 80;
const PAD: i32 = 3;
const LH: i32 = 11; // Line height

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
    /// Component metadata (type, test ID)
    Component,
    /// Performance statistics
    Stats,
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
        let tabs = [("LYT", InspectorTab::Layout), ("CMP", InspectorTab::Component), ("STS", InspectorTab::Stats)];
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
            }

            InspectorTab::Stats => {
                txt(&mut canvas, cy, "STATS", COL_TITLE);
                cy += LH;
                kv(&mut canvas, cy, "full  ", "0");
                cy += LH;
                kv(&mut canvas, cy, "partial ", "0");
                cy += LH;
                txt(&mut canvas, cy, "(live stats TBD)", COL_KEY);
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
        inspector.set_tab(InspectorTab::Stats);
        assert_eq!(inspector.current_tab(), InspectorTab::Stats);
        inspector.set_tab(InspectorTab::Layout);
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    }

    #[test]
    fn test_render_layout_tab() {
        let inspector = Inspector::new();
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component);
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
        inspector.render_details(&mut buffer, 800, 10, 10, &component);
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_render_stats_tab() {
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::Stats);
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 10, 10, &component);
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_render_all_tabs() {
        let mut inspector = Inspector::new();
        let component = make_component();
        let mut buffer = vec![0u32; 800 * 600];
        for tab in [InspectorTab::Layout, InspectorTab::Component, InspectorTab::Stats] {
            inspector.set_tab(tab);
            inspector.render_details(&mut buffer, 800, 10, 10, &component);
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
        };
        let mut buffer = vec![0u32; 800 * 600];
        inspector.render_details(&mut buffer, 800, 0, 0, &component);
    }

    #[test]
    fn test_tooltip_clamped_near_right_edge() {
        let inspector = Inspector::new();
        let component = make_component();
        let screen_w = 200u32;
        let mut buffer = vec![0u32; (screen_w * 200) as usize];
        // Request x near the right edge — tooltip should be clamped
        inspector.render_details(&mut buffer, screen_w, screen_w - 10, 0, &component);
        // Should not panic, pixels should be written
        let written = buffer.iter().any(|&px| px != 0);
        assert!(written);
    }

    #[test]
    fn test_inspector_tab_equality() {
        assert_eq!(InspectorTab::Layout, InspectorTab::Layout);
        assert_ne!(InspectorTab::Layout, InspectorTab::Component);
        assert_ne!(InspectorTab::Component, InspectorTab::Stats);
    }

    #[test]
    fn test_inspector_tab_clone() {
        let tab = InspectorTab::Component;
        assert_eq!(tab, tab.clone());
    }

    #[test]
    fn test_inspector_tab_copy() {
        let tab = InspectorTab::Stats;
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
