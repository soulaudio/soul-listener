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
//!     },
//! ];
//!
//! renderer.render_borders(&mut buffer, 800, 600, &components);
//! ```

use super::state::ComponentInfo;

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
        let renderer = OverlayRenderer::default();
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
            },
            ComponentInfo {
                component_type: "Label".to_string(),
                position: (20, 20),
                size: (80, 20),
                test_id: Some("title".to_string()),
            },
            ComponentInfo {
                component_type: "ProgressBar".to_string(),
                position: (20, 50),
                size: (80, 10),
                test_id: None,
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
        }];

        renderer.render_borders(&mut buffer, 100, 100, &components);

        // All four pixels should form the border
        assert_eq!(buffer[50 * 100 + 50], 0xFFFF4040); // Top-left
        assert_eq!(buffer[50 * 100 + 51], 0xFFFF4040); // Top-right
        assert_eq!(buffer[51 * 100 + 50], 0xFFFF4040); // Bottom-left
        assert_eq!(buffer[51 * 100 + 51], 0xFFFF4040); // Bottom-right
    }
}
