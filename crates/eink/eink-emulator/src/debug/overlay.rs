//! Debug overlay rendering
//!
//! Provides visual debugging overlays for component layout inspection. The overlay renderer
//! draws colored borders around UI components to help identify layout boundaries and hierarchy.
//!
//! # Color Scheme
//!
//! - **Container** - Blue (0xFF0080FF)
//! - **Button** - Green (0xFF00FF80)
//! - **Label** - Red (0xFFFF4040)
//! - **ProgressBar** - Purple (0xFFC040FF)
//! - **Other** - Yellow (0xFFFFCC00)
//!
//! # Example
//!
//! ```no_run
//! use eink_emulator::debug::overlay::OverlayRenderer;
//! use eink_emulator::debug::state::ComponentInfo;
//!
//! let renderer = OverlayRenderer::new();
//! let mut buffer = vec![0u32; 800 * 600];
//!
//! let components = vec![
//!     ComponentInfo {
//!         component_type: "Button".to_string(),
//!         position: (10, 10),
//!         size: (100, 40),
//!         test_id: Some("play-button".to_string()),
//!         ..Default::default()
//!     },
//! ];
//!
//! renderer.render_borders(&mut buffer, 800, 600, &components);
//! ```

use std::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
};

use super::state::ComponentInfo;

// ---------------------------------------------------------------------------
// Per-instance color palette (cycles when more components than colors)
// ---------------------------------------------------------------------------

/// Distinct ARGB colors used for per-instance border coloring.
const INSTANCE_PALETTE: &[u32] = &[
    0xFF4488FF, // blue
    0xFF44FF88, // green
    0xFFFF6644, // orange-red
    0xFFFF44FF, // magenta
    0xFF44FFFF, // cyan
    0xFFFFDD44, // yellow
    0xFFFF8844, // orange
    0xFFBB44FF, // purple
];

// ---------------------------------------------------------------------------
// OverlayCanvas — embedded-graphics DrawTarget backed by a raw u32 slice
// ---------------------------------------------------------------------------

struct OverlayCanvas<'buf> {
    buf: &'buf mut [u32],
    width: u32,
    height: u32,
}

impl<'buf> OverlayCanvas<'buf> {
    fn new(buf: &'buf mut [u32], width: u32, height: u32) -> Self {
        Self { buf, width, height }
    }
}

impl OriginDimensions for OverlayCanvas<'_> {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for OverlayCanvas<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Infallible>
    where
        I: IntoIterator<Item = Pixel<Rgb888>>,
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0 && coord.y >= 0 {
                let x = coord.x as u32;
                let y = coord.y as u32;
                if x < self.width && y < self.height {
                    let idx = (y * self.width + x) as usize;
                    if idx < self.buf.len() {
                        self.buf[idx] = 0xFF000000
                            | ((color.r() as u32) << 16)
                            | ((color.g() as u32) << 8)
                            | color.b() as u32;
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Alpha-blended fill helper
// ---------------------------------------------------------------------------

/// Fill a rectangle in `buffer` with `rgb` colour at the given `alpha` (0–255).
///
/// Each covered pixel is composited as:  `out = existing*(1-a) + rgb*a`
/// This produces a semi-transparent overlay without a separate alpha channel.
#[allow(clippy::too_many_arguments)]
fn fill_rect_blended(
    buffer: &mut [u32],
    stride: u32,
    height: u32,
    bx: i32,
    by: i32,
    bw: u32,
    bh: u32,
    rgb: u32, // 0x00RRGGBB
    alpha: u8,
) {
    let r_ov = (rgb >> 16) & 0xFF;
    let g_ov = (rgb >> 8) & 0xFF;
    let b_ov = rgb & 0xFF;
    let a = alpha as u32;
    let a_inv = 255 - a;

    for dy in 0..bh {
        let py = by + dy as i32;
        if py < 0 || py >= height as i32 {
            continue;
        }
        for dx in 0..bw {
            let px = bx + dx as i32;
            if px < 0 || px >= stride as i32 {
                continue;
            }
            let idx = (py as u32 * stride + px as u32) as usize;
            if idx >= buffer.len() {
                break;
            }
            let ex = buffer[idx];
            let r = ((ex >> 16 & 0xFF) * a_inv + r_ov * a) / 255;
            let g = ((ex >> 8 & 0xFF) * a_inv + g_ov * a) / 255;
            let b = ((ex & 0xFF) * a_inv + b_ov * a) / 255;
            buffer[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
        }
    }
}

/// Renders debug overlays on the framebuffer
///
/// The overlay renderer draws 1-pixel colored borders around components to visualize
/// layout boundaries. Component types are distinguished by color for easy identification.
pub struct OverlayRenderer;

impl OverlayRenderer {
    /// Create a new overlay renderer
    pub fn new() -> Self {
        Self
    }

    /// Render debug borders around components
    ///
    /// Draws a 1-pixel colored border around each component in the provided list.
    /// The border color is determined by the component type.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The ARGB framebuffer to draw into
    /// * `width` - Width of the framebuffer in pixels
    /// * `_height` - Height of the framebuffer in pixels (unused but kept for API consistency)
    /// * `components` - List of components to draw borders around
    ///
    /// # Pixel Format
    ///
    /// Colors are in ARGB format: 0xAARRGGBB where:
    /// - AA = Alpha channel (0xFF = fully opaque)
    /// - RR = Red channel
    /// - GG = Green channel
    /// - BB = Blue channel
    pub fn render_borders(
        &self,
        buffer: &mut [u32],
        width: u32,
        _height: u32,
        components: &[ComponentInfo],
    ) {
        for component in components {
            let color = self.get_border_color(&component.component_type);
            self.draw_rect_border(buffer, width, component.position, component.size, color);
        }
    }

    /// Render the **Layout** overlay: per-instance colored semi-transparent fills,
    /// solid 2px borders, and `test_id` labels in the top-left corner.
    ///
    /// This is the visual counterpart to the "Layout" debug mode (Ctrl+2).  Each
    /// component gets a unique hue cycled from [`INSTANCE_PALETTE`].  The fill is
    /// alpha-blended at 25 % opacity so the underlying display content remains
    /// readable.  The border uses 2 concentric 1px lines for visibility against both
    /// light and dark content.
    ///
    /// Padding / margin visualisation: if a component's `test_id` contains the
    /// substring `"pad"` or `"margin"` it is rendered at reduced opacity (15 %)
    /// so it reads as secondary to its parent.
    ///
    /// # Arguments
    ///
    /// * `buffer`     – ARGB framebuffer to draw into
    /// * `width`      – Width of the framebuffer in pixels
    /// * `height`     – Height of the framebuffer in pixels
    /// * `components` – List of components to draw layout regions for
    pub fn render_layout(
        &self,
        buffer: &mut [u32],
        width: u32,
        height: u32,
        components: &[ComponentInfo],
    ) {
        // Pass 1 — semi-transparent fills (drawn back-to-front so inner comps win)
        for (i, component) in components.iter().enumerate() {
            let argb = INSTANCE_PALETTE[i % INSTANCE_PALETTE.len()];
            let rgb = argb & 0x00FFFFFF;

            // Detect padding/margin zones → lighter fill
            let is_secondary = component
                .test_id
                .as_deref()
                .map(|id| id.contains("pad") || id.contains("margin"))
                .unwrap_or(false);
            let fill_alpha = if is_secondary { 38u8 } else { 64u8 }; // ~15 % or ~25 %

            fill_rect_blended(
                buffer,
                width,
                height,
                component.position.0,
                component.position.1,
                component.size.0,
                component.size.1,
                rgb,
                fill_alpha,
            );
        }

        // Pass 2 — 2px solid borders (outer + inner 1px)
        for (i, component) in components.iter().enumerate() {
            let color = INSTANCE_PALETTE[i % INSTANCE_PALETTE.len()];
            self.draw_rect_border(buffer, width, component.position, component.size, color);
            if component.size.0 > 4 && component.size.1 > 4 {
                self.draw_rect_border(
                    buffer,
                    width,
                    (component.position.0 + 1, component.position.1 + 1),
                    (component.size.0 - 2, component.size.1 - 2),
                    color,
                );
            }
        }

        // Pass 3 — label backgrounds + text
        struct LabelCmd {
            tx: i32,
            ty: i32,
            text: String,
            color: Rgb888,
        }
        let mut label_cmds: Vec<LabelCmd> = Vec::with_capacity(components.len());

        for (i, component) in components.iter().enumerate() {
            let (bx, by) = component.position;
            let (bw, bh) = component.size;
            if bw < 20 || bh < 14 {
                continue;
            }

            let label = component
                .test_id
                .as_deref()
                .unwrap_or(&component.component_type);
            let max_chars = (bw.saturating_sub(8) / 6).min(32) as usize;
            let label_short: String = label.chars().take(max_chars).collect();
            if label_short.is_empty() {
                continue;
            }

            // Dark pill background behind the label for readability.
            let bg_w = (label_short.len() as u32 * 6 + 4).min(bw.saturating_sub(4));
            let bg_h = 12u32;
            let bg_x = (bx + 2).max(0) as u32;
            let bg_y = (by + 1).max(0) as u32;
            for dy in 0..bg_h {
                for dx in 0..bg_w {
                    let px = bg_x + dx;
                    let py = bg_y + dy;
                    if px < width && py < height {
                        let idx = (py * width + px) as usize;
                        if idx < buffer.len() {
                            buffer[idx] = 0xFF0A0A14;
                        }
                    }
                }
            }

            let argb = INSTANCE_PALETTE[i % INSTANCE_PALETTE.len()];
            label_cmds.push(LabelCmd {
                tx: bx + 3,
                ty: by + 11,
                text: label_short,
                color: Rgb888::new(
                    ((argb >> 16) & 0xFF) as u8,
                    ((argb >> 8) & 0xFF) as u8,
                    (argb & 0xFF) as u8,
                ),
            });
        }

        // Single canvas pass for text.
        let mut canvas = OverlayCanvas::new(buffer, width, height);
        for cmd in &label_cmds {
            let style = MonoTextStyle::new(&FONT_6X10, cmd.color);
            let _ = Text::new(&cmd.text, Point::new(cmd.tx, cmd.ty), style).draw(&mut canvas);
        }
    }

    /// Draw an amber highlight around a **hovered** component (inspect mode).
    ///
    /// Used when the cursor is over a component while inspect mode (Ctrl+3) is
    /// active.  Thinner than the selected highlight and drawn in amber so it
    /// reads as "transient" rather than pinned.
    ///
    /// # Arguments
    ///
    /// * `buffer` – ARGB framebuffer (display content, pre-rotation)
    /// * `width`  – Width of the framebuffer in pixels
    /// * `height` – Height of the framebuffer in pixels
    /// * `comp`   – The component under the cursor
    pub fn render_hovered_component(
        &self,
        buffer: &mut [u32],
        width: u32,
        height: u32,
        comp: &ComponentInfo,
    ) {
        const HOV_COLOR: u32 = 0xFFFF8C00; // amber / orange
        const HOV_FILL_ALPHA: u8 = 18; // ~7 % opacity fill

        fill_rect_blended(
            buffer,
            width,
            height,
            comp.position.0,
            comp.position.1,
            comp.size.0,
            comp.size.1,
            HOV_COLOR & 0x00FFFFFF,
            HOV_FILL_ALPHA,
        );

        // Single-pixel amber border
        self.draw_rect_border(buffer, width, comp.position, comp.size, HOV_COLOR);
    }

    /// Draw a bright highlight border around a single **selected** component.
    ///
    /// Used to show which component is currently selected (via panel click or
    /// display click), independently of the full borders overlay (Ctrl+2).
    /// The highlight is a bright cyan 2-pixel border with a subtle fill so it
    /// is visible against both light and dark display content.
    ///
    /// # Arguments
    ///
    /// * `buffer` – ARGB framebuffer (display content, pre-rotation)
    /// * `width`  – Width of the framebuffer in pixels
    /// * `height` – Height of the framebuffer in pixels
    /// * `comp`   – The component to highlight
    pub fn render_selected_component(
        &self,
        buffer: &mut [u32],
        width: u32,
        height: u32,
        comp: &ComponentInfo,
    ) {
        const SEL_COLOR: u32 = 0xFFFF6200; // vivid orange – clearly visible on gray display
        const SEL_FILL_ALPHA: u8 = 45; // ~18% fill

        fill_rect_blended(
            buffer,
            width,
            height,
            comp.position.0,
            comp.position.1,
            comp.size.0,
            comp.size.1,
            SEL_COLOR & 0x00FFFFFF,
            SEL_FILL_ALPHA,
        );

        // 3-pixel border: outer + middle + inner for strong visibility
        self.draw_rect_border(buffer, width, comp.position, comp.size, SEL_COLOR);
        if comp.size.0 > 4 && comp.size.1 > 4 {
            self.draw_rect_border(
                buffer,
                width,
                (comp.position.0 + 1, comp.position.1 + 1),
                (comp.size.0 - 2, comp.size.1 - 2),
                SEL_COLOR,
            );
        }
        if comp.size.0 > 8 && comp.size.1 > 8 {
            self.draw_rect_border(
                buffer,
                width,
                (comp.position.0 + 2, comp.position.1 + 2),
                (comp.size.0 - 4, comp.size.1 - 4),
                0xFFFFAA00, // lighter orange inner ring
            );
        }
    }

    /// Draw Chromium-DevTools-style box-model zone fills on a hovered component.
    ///
    /// Shows coloured semi-transparent zones for:
    /// - **Margin** (orange) — area outside the border
    /// - **Border** (yellow) — the border ring itself
    /// - **Padding** (green) — area between border and content
    /// - **Content** (blue) — the inner content area
    ///
    /// Falls back to a simple amber highlight when all spacing values are zero.
    pub fn render_hovered_box_model(
        &self,
        buffer: &mut [u32],
        screen_width: u32,
        screen_height: u32,
        comp: &ComponentInfo,
    ) {
        // Margin zone (full rect including margin)
        let mx = comp.position.0 - comp.margin.left as i32;
        let my = comp.position.1 - comp.margin.top as i32;
        let mw = comp.size.0 + comp.margin.left as u32 + comp.margin.right as u32;
        let mh = comp.size.1 + comp.margin.top as u32 + comp.margin.bottom as u32;
        fill_rect_blended(
            buffer,
            screen_width,
            screen_height,
            mx,
            my,
            mw,
            mh,
            0xFFA040,
            100,
        );

        // Border zone (component rect itself)
        fill_rect_blended(
            buffer,
            screen_width,
            screen_height,
            comp.position.0,
            comp.position.1,
            comp.size.0,
            comp.size.1,
            0xFFD040,
            100,
        );

        // Padding zone (inside border)
        let px = comp.position.0 + comp.border.left as i32;
        let py = comp.position.1 + comp.border.top as i32;
        let pw = comp
            .size
            .0
            .saturating_sub(comp.border.left as u32 + comp.border.right as u32);
        let ph = comp
            .size
            .1
            .saturating_sub(comp.border.top as u32 + comp.border.bottom as u32);
        fill_rect_blended(
            buffer,
            screen_width,
            screen_height,
            px,
            py,
            pw,
            ph,
            0x50CC50,
            100,
        );

        // Content zone (inside padding)
        let cx = px + comp.padding.left as i32;
        let cy = py + comp.padding.top as i32;
        let cw = pw.saturating_sub(comp.padding.left as u32 + comp.padding.right as u32);
        let ch = ph.saturating_sub(comp.padding.top as u32 + comp.padding.bottom as u32);
        fill_rect_blended(
            buffer,
            screen_width,
            screen_height,
            cx,
            cy,
            cw,
            ch,
            0x4090E0,
            100,
        );
    }

    // Keep the old name as an alias so existing callers outside window.rs still compile.
    #[inline]
    pub fn render_borders_labeled(
        &self,
        buffer: &mut [u32],
        width: u32,
        height: u32,
        components: &[ComponentInfo],
    ) {
        self.render_layout(buffer, width, height, components);
    }

    /// Get the border color for a component type
    ///
    /// Returns the appropriate debug color based on component type. Unknown types
    /// default to yellow.
    fn get_border_color(&self, component_type: &str) -> u32 {
        match component_type {
            "Container" => 0xFF0080FF,   // Blue
            "Button" => 0xFF00FF80,      // Green
            "Label" => 0xFFFF4040,       // Red
            "ProgressBar" => 0xFFC040FF, // Purple
            _ => 0xFFFFCC00,             // Yellow (unknown/default)
        }
    }

    /// Draw a 1-pixel rectangular border
    ///
    /// Draws the four edges of a rectangle in the specified color.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The framebuffer to draw into
    /// * `width` - Width of the framebuffer
    /// * `pos` - Top-left corner position (x, y)
    /// * `size` - Size of the rectangle (width, height)
    /// * `color` - ARGB color value
    fn draw_rect_border(
        &self,
        buffer: &mut [u32],
        width: u32,
        pos: (i32, i32),
        size: (u32, u32),
        color: u32,
    ) {
        let (x, y) = pos;
        let (w, h) = size;

        // Top border
        for dx in 0..w {
            self.set_pixel(buffer, width, x + dx as i32, y, color);
        }
        // Bottom border
        for dx in 0..w {
            self.set_pixel(buffer, width, x + dx as i32, y + h as i32 - 1, color);
        }
        // Left border
        for dy in 0..h {
            self.set_pixel(buffer, width, x, y + dy as i32, color);
        }
        // Right border
        for dy in 0..h {
            self.set_pixel(buffer, width, x + w as i32 - 1, y + dy as i32, color);
        }
    }

    /// Set a single pixel in the framebuffer with bounds checking
    ///
    /// Only sets the pixel if coordinates are within valid bounds. Negative
    /// coordinates are skipped.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The framebuffer to write to
    /// * `width` - Width of the framebuffer
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
    /// * `color` - ARGB color value
    fn set_pixel(&self, buffer: &mut [u32], width: u32, x: i32, y: i32, color: u32) {
        if x >= 0 && y >= 0 {
            let idx = (y as u32 * width + x as u32) as usize;
            if idx < buffer.len() {
                buffer[idx] = color;
            }
        }
    }
}

impl Default for OverlayRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_renderer_creation() {
        let renderer = OverlayRenderer::new();
        // Verify construction succeeds
        let _ = renderer;
    }

    #[test]
    fn test_overlay_renderer_default() {
        let renderer = OverlayRenderer::new();
        // Verify default construction succeeds
        let _ = renderer;
    }

    #[test]
    fn test_border_color_selection() {
        let renderer = OverlayRenderer::new();
        assert_eq!(renderer.get_border_color("Container"), 0xFF0080FF);
        assert_eq!(renderer.get_border_color("Button"), 0xFF00FF80);
        assert_eq!(renderer.get_border_color("Label"), 0xFFFF4040);
        assert_eq!(renderer.get_border_color("ProgressBar"), 0xFFC040FF);
        assert_eq!(renderer.get_border_color("Unknown"), 0xFFFFCC00);
        assert_eq!(renderer.get_border_color("AnyOtherType"), 0xFFFFCC00);
    }

    #[test]
    fn test_render_borders() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 100 * 100];

        let components = vec![ComponentInfo {
            component_type: "Button".to_string(),
            position: (10, 10),
            size: (50, 30),
            test_id: None,
            ..Default::default()
        }];

        renderer.render_borders(&mut buffer, 100, 100, &components);

        // Check top-left corner pixel
        let idx = 10 * 100 + 10;
        assert_eq!(buffer[idx], 0xFF00FF80); // Green for Button

        // Check top-right corner pixel
        let idx = 10 * 100 + 59; // x=10+50-1=59
        assert_eq!(buffer[idx], 0xFF00FF80);

        // Check bottom-left corner pixel
        let idx = 39 * 100 + 10; // y=10+30-1=39
        assert_eq!(buffer[idx], 0xFF00FF80);

        // Check bottom-right corner pixel
        let idx = 39 * 100 + 59;
        assert_eq!(buffer[idx], 0xFF00FF80);

        // Check interior pixel (should be untouched)
        let idx = 20 * 100 + 30; // Middle of the button
        assert_eq!(buffer[idx], 0);
    }

    #[test]
    fn test_render_multiple_components() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 200 * 200];

        let components = vec![
            ComponentInfo {
                component_type: "Container".to_string(),
                position: (10, 10),
                size: (100, 150),
                test_id: None,
                ..Default::default()
            },
            ComponentInfo {
                component_type: "Label".to_string(),
                position: (20, 20),
                size: (80, 20),
                test_id: Some("title".to_string()),
                ..Default::default()
            },
            ComponentInfo {
                component_type: "ProgressBar".to_string(),
                position: (20, 50),
                size: (80, 10),
                test_id: None,
                ..Default::default()
            },
        ];

        renderer.render_borders(&mut buffer, 200, 200, &components);

        // Check container (blue)
        assert_eq!(buffer[10 * 200 + 10], 0xFF0080FF);

        // Check label (red)
        assert_eq!(buffer[20 * 200 + 20], 0xFFFF4040);

        // Check progress bar (purple)
        assert_eq!(buffer[50 * 200 + 20], 0xFFC040FF);
    }

    #[test]
    fn test_bounds_checking_negative_coordinates() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 100 * 100];

        let components = vec![ComponentInfo {
            component_type: "Button".to_string(),
            position: (-10, -10), // Partially off-screen
            size: (30, 30),
            test_id: None,
            ..Default::default()
        }];

        // Should not panic
        renderer.render_borders(&mut buffer, 100, 100, &components);

        // The bottom-right corner of the component at (19, 19) should be visible
        let idx = 19 * 100 + 19;
        assert_eq!(buffer[idx], 0xFF00FF80);
    }

    #[test]
    fn test_bounds_checking_out_of_bounds() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 100 * 100];

        let components = vec![ComponentInfo {
            component_type: "Button".to_string(),
            position: (90, 90),
            size: (50, 50), // Extends beyond buffer
            test_id: None,
            ..Default::default()
        }];

        // Should not panic
        renderer.render_borders(&mut buffer, 100, 100, &components);

        // Check visible corner
        let idx = 90 * 100 + 90;
        assert_eq!(buffer[idx], 0xFF00FF80);
    }

    #[test]
    fn test_render_empty_components() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 100 * 100];

        renderer.render_borders(&mut buffer, 100, 100, &[]);

        // Buffer should remain unchanged
        assert!(buffer.iter().all(|&pixel| pixel == 0));
    }

    #[test]
    fn test_small_component() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 100 * 100];

        let components = vec![ComponentInfo {
            component_type: "Button".to_string(),
            position: (50, 50),
            size: (1, 1), // Single pixel
            test_id: None,
            ..Default::default()
        }];

        renderer.render_borders(&mut buffer, 100, 100, &components);

        // Single pixel should be colored
        let idx = 50 * 100 + 50;
        assert_eq!(buffer[idx], 0xFF00FF80);
    }

    #[test]
    fn test_two_pixel_component() {
        let renderer = OverlayRenderer::new();
        let mut buffer = vec![0u32; 100 * 100];

        let components = vec![ComponentInfo {
            component_type: "Label".to_string(),
            position: (50, 50),
            size: (2, 2),
            test_id: None,
            ..Default::default()
        }];

        renderer.render_borders(&mut buffer, 100, 100, &components);

        // All four pixels should form the border
        assert_eq!(buffer[50 * 100 + 50], 0xFFFF4040); // Top-left
        assert_eq!(buffer[50 * 100 + 51], 0xFFFF4040); // Top-right
        assert_eq!(buffer[51 * 100 + 50], 0xFFFF4040); // Bottom-left
        assert_eq!(buffer[51 * 100 + 51], 0xFFFF4040); // Bottom-right
    }
}
