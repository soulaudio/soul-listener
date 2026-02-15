//! Debug panel UI rendering
//!
//! Provides a side panel for debug controls and information display. The panel appears on
//! the right side of the screen with a semi-transparent background and contains interactive
//! controls for toggling debug features.
//!
//! # Panel Layout
//!
//! The panel is 200 pixels wide and positioned on the right edge of the screen. It uses a
//! semi-transparent dark gray background (0xDC282828) to overlay the main content while
//! maintaining visibility.
//!
//! # Example
//!
//! ```no_run
//! use eink_emulator::debug::panel::DebugPanel;
//! use eink_emulator::debug::state::DebugState;
//!
//! let panel = DebugPanel::new();
//! let mut buffer = vec![0u32; 800 * 600];
//! let mut state = DebugState::new();
//! state.panel_visible = true;
//!
//! panel.render(&mut buffer, 800, 600, &state);
//! ```

use super::state::DebugState;

const PANEL_WIDTH: u32 = 200;
const PANEL_BG_COLOR: u32 = 0xDC282828; // Semi-transparent dark gray

/// Debug panel renderer
///
/// The debug panel provides an interactive UI for controlling debug features. It renders
/// a 200-pixel wide panel on the right side of the screen with a semi-transparent background.
/// The panel is only visible when `panel_visible` is enabled in the debug state.
pub struct DebugPanel {
    width: u32,
}

impl DebugPanel {
    /// Create a new debug panel renderer
    pub fn new() -> Self {
        Self { width: PANEL_WIDTH }
    }

    /// Get the width of the debug panel in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Render the debug panel
    ///
    /// Draws the debug panel on the right side of the screen if enabled in the state.
    /// The panel consists of a semi-transparent background and interactive controls.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The ARGB framebuffer to draw into
    /// * `screen_width` - Width of the screen in pixels
    /// * `screen_height` - Height of the screen in pixels
    /// * `state` - Current debug state containing panel visibility and settings
    ///
    /// # Pixel Format
    ///
    /// Colors are in ARGB format: 0xAARRGGBB where:
    /// - AA = Alpha channel (0xDC = ~86% opacity for semi-transparent background)
    /// - RR = Red channel
    /// - GG = Green channel
    /// - BB = Blue channel
    pub fn render(
        &self,
        buffer: &mut [u32],
        screen_width: u32,
        screen_height: u32,
        state: &DebugState,
    ) {
        if !state.panel_visible {
            return;
        }

        let panel_x = screen_width - PANEL_WIDTH;

        // Draw background
        self.draw_background(buffer, screen_width, screen_height, panel_x);

        // Draw controls
        self.draw_controls(buffer, screen_width, panel_x, state);
    }

    /// Draw the semi-transparent panel background
    ///
    /// Fills the panel area with a semi-transparent dark gray color.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The framebuffer to draw into
    /// * `screen_width` - Width of the screen
    /// * `screen_height` - Height of the screen
    /// * `panel_x` - X coordinate where the panel starts
    fn draw_background(
        &self,
        buffer: &mut [u32],
        screen_width: u32,
        screen_height: u32,
        panel_x: u32,
    ) {
        for y in 0..screen_height {
            for x in panel_x..screen_width {
                let idx = (y * screen_width + x) as usize;
                if idx < buffer.len() {
                    buffer[idx] = PANEL_BG_COLOR;
                }
            }
        }
    }

    /// Draw interactive controls in the panel
    ///
    /// This method will render checkboxes, buttons, and other UI elements for
    /// controlling debug features. Currently a stub for future implementation.
    ///
    /// # Arguments
    ///
    /// * `_buffer` - The framebuffer to draw into (unused in stub)
    /// * `_screen_width` - Width of the screen (unused in stub)
    /// * `_panel_x` - X coordinate where the panel starts (unused in stub)
    /// * `_state` - Current debug state (unused in stub)
    fn draw_controls(
        &self,
        _buffer: &mut [u32],
        _screen_width: u32,
        _panel_x: u32,
        _state: &DebugState,
    ) {
        // TODO: Implement in next task
    }
}

impl Default for DebugPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_creation() {
        let panel = DebugPanel::new();
        assert_eq!(panel.width(), 200);
    }

    #[test]
    fn test_panel_default() {
        let panel = DebugPanel::default();
        assert_eq!(panel.width(), 200);
    }

    #[test]
    fn test_panel_not_visible_when_disabled() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFF; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = false;

        panel.render(&mut buffer, 800, 600, &state);

        // Buffer should be unchanged (all white)
        assert_eq!(buffer[0], 0xFFFFFFFF);
        assert_eq!(buffer[buffer.len() - 1], 0xFFFFFFFF);
    }

    #[test]
    fn test_panel_renders_background_when_visible() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFF; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 800, 600, &state);

        // Check pixel in panel area (right side)
        let panel_pixel_idx = (0 * 800 + 700) as usize; // Top row, panel area (x=700 > 800-200)
        assert_eq!(buffer[panel_pixel_idx], PANEL_BG_COLOR);

        // Check pixel outside panel area (left side)
        let outside_pixel_idx = (0 * 800 + 400) as usize; // Top row, main area
        assert_eq!(buffer[outside_pixel_idx], 0xFFFFFFFF); // Should be unchanged
    }

    #[test]
    fn test_panel_background_fills_entire_height() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFF; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 800, 600, &state);

        // Check top of panel
        let top_pixel_idx = (0 * 800 + 700) as usize;
        assert_eq!(buffer[top_pixel_idx], PANEL_BG_COLOR);

        // Check middle of panel
        let middle_pixel_idx = (300 * 800 + 700) as usize;
        assert_eq!(buffer[middle_pixel_idx], PANEL_BG_COLOR);

        // Check bottom of panel
        let bottom_pixel_idx = (599 * 800 + 700) as usize;
        assert_eq!(buffer[bottom_pixel_idx], PANEL_BG_COLOR);
    }

    #[test]
    fn test_panel_background_fills_entire_width() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFF; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 800, 600, &state);

        // Check leftmost edge of panel (x = 800 - 200 = 600)
        let left_edge_idx = (100 * 800 + 600) as usize;
        assert_eq!(buffer[left_edge_idx], PANEL_BG_COLOR);

        // Check rightmost edge of panel (x = 799)
        let right_edge_idx = (100 * 800 + 799) as usize;
        assert_eq!(buffer[right_edge_idx], PANEL_BG_COLOR);
    }

    #[test]
    fn test_panel_boundary_is_correct() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFF; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 800, 600, &state);

        // Pixel just before panel (x=599) should be unchanged
        let before_panel_idx = (100 * 800 + 599) as usize;
        assert_eq!(buffer[before_panel_idx], 0xFFFFFFFF);

        // Pixel at panel start (x=600) should be panel color
        let panel_start_idx = (100 * 800 + 600) as usize;
        assert_eq!(buffer[panel_start_idx], PANEL_BG_COLOR);
    }

    #[test]
    fn test_panel_with_different_screen_sizes() {
        let panel = DebugPanel::new();

        // Test with 1024x768 screen
        let mut buffer = vec![0xFFFFFFFF; 1024 * 768];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 1024, 768, &state);

        // Panel should start at x = 1024 - 200 = 824
        let panel_pixel_idx = (0 * 1024 + 900) as usize;
        assert_eq!(buffer[panel_pixel_idx], PANEL_BG_COLOR);

        // Pixel before panel should be unchanged
        let before_panel_idx = (0 * 1024 + 700) as usize;
        assert_eq!(buffer[before_panel_idx], 0xFFFFFFFF);
    }

    #[test]
    fn test_panel_with_small_screen() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFF; 400 * 300];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 400, 300, &state);

        // Panel should start at x = 400 - 200 = 200
        let panel_pixel_idx = (0 * 400 + 300) as usize;
        assert_eq!(buffer[panel_pixel_idx], PANEL_BG_COLOR);

        // Panel covers half the screen
        let left_half_idx = (0 * 400 + 100) as usize;
        assert_eq!(buffer[left_half_idx], 0xFFFFFFFF); // Unchanged

        let right_half_idx = (0 * 400 + 250) as usize;
        assert_eq!(buffer[right_half_idx], PANEL_BG_COLOR); // Panel color
    }
}
