//! Component inspector for viewing detailed information
//!
//! The inspector provides a tabbed interface for viewing component details:
//! - **Layout Tab**: Position, size, and layout properties
//! - **Component Tab**: Component type and hierarchy information
//! - **Stats Tab**: Performance metrics and refresh statistics
//!
//! # Example
//!
//! ```rust
//! use eink_emulator::debug::inspector::{Inspector, InspectorTab};
//! use eink_emulator::debug::state::ComponentInfo;
//!
//! let mut inspector = Inspector::new();
//!
//! // Switch to component tab
//! inspector.set_tab(InspectorTab::Component);
//!
//! // Render component details
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

use super::state::ComponentInfo;

/// Inspector tab types for viewing different aspects of a component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    /// Layout properties (position, size, constraints)
    Layout,
    /// Component metadata (type, hierarchy, test ID)
    Component,
    /// Performance statistics (refresh count, render time)
    Stats,
}

/// Component inspector with tabbed interface
///
/// The inspector displays detailed information about selected components
/// in a tabbed interface. Each tab shows different aspects of the component:
///
/// - **Layout**: Position, size, padding, margins
/// - **Component**: Type, test ID, parent/child relationships
/// - **Stats**: Render count, refresh statistics, timing data
///
/// # Note
///
/// Current implementation uses `println!` stubs for rendering. Full text
/// rendering will be implemented when the text rendering system is ready.
pub struct Inspector {
    current_tab: InspectorTab,
}

impl Inspector {
    /// Create a new inspector with Layout tab selected by default
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_emulator::debug::inspector::{Inspector, InspectorTab};
    ///
    /// let inspector = Inspector::new();
    /// assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    /// ```
    pub fn new() -> Self {
        Self {
            current_tab: InspectorTab::Layout,
        }
    }

    /// Switch to a different inspector tab
    ///
    /// # Arguments
    ///
    /// * `tab` - The tab to switch to
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_emulator::debug::inspector::{Inspector, InspectorTab};
    ///
    /// let mut inspector = Inspector::new();
    /// inspector.set_tab(InspectorTab::Component);
    /// assert_eq!(inspector.current_tab(), InspectorTab::Component);
    /// ```
    pub fn set_tab(&mut self, tab: InspectorTab) {
        self.current_tab = tab;
    }

    /// Get the currently selected tab
    ///
    /// # Returns
    ///
    /// The current inspector tab
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_emulator::debug::inspector::{Inspector, InspectorTab};
    ///
    /// let inspector = Inspector::new();
    /// assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    /// ```
    pub fn current_tab(&self) -> InspectorTab {
        self.current_tab
    }

    /// Render component details based on the current tab
    ///
    /// # Arguments
    ///
    /// * `buffer` - Pixel buffer to render into
    /// * `screen_width` - Width of the screen in pixels
    /// * `x` - X position to start rendering
    /// * `y` - Y position to start rendering
    /// * `component` - Component information to display
    ///
    /// # Note
    ///
    /// Currently uses `println!` for output. Will be replaced with actual
    /// text rendering to the buffer when the text system is implemented.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_emulator::debug::inspector::Inspector;
    /// use eink_emulator::debug::state::ComponentInfo;
    ///
    /// let inspector = Inspector::new();
    /// let component = ComponentInfo {
    ///     component_type: "Button".to_string(),
    ///     position: (10, 20),
    ///     size: (100, 40),
    ///     test_id: None,
    /// };
    ///
    /// let mut buffer = vec![0u32; 800 * 600];
    /// inspector.render_details(&mut buffer, 800, 10, 10, &component);
    /// ```
    pub fn render_details(
        &self,
        _buffer: &mut [u32],
        _screen_width: u32,
        _x: u32,
        _y: u32,
        component: &ComponentInfo,
    ) {
        // Render component details based on current tab
        match self.current_tab {
            InspectorTab::Layout => {
                self.render_layout_tab(component);
            }
            InspectorTab::Component => {
                self.render_component_tab(component);
            }
            InspectorTab::Stats => {
                self.render_stats_tab(component);
            }
        }
    }

    /// Render the Layout tab showing position and size information
    ///
    /// Displays:
    /// - Position (x, y)
    /// - Size (width, height)
    /// - Bounds calculation
    ///
    /// # Note
    ///
    /// Currently outputs to stdout via `println!`. Will be replaced with
    /// actual text rendering to buffer.
    fn render_layout_tab(&self, component: &ComponentInfo) {
        // TODO: Implement text rendering to buffer
        println!(
            "Layout: pos={:?}, size={:?}",
            component.position, component.size
        );
    }

    /// Render the Component tab showing component metadata
    ///
    /// Displays:
    /// - Component type
    /// - Test ID (if present)
    /// - Hierarchy information (future)
    ///
    /// # Note
    ///
    /// Currently outputs to stdout via `println!`. Will be replaced with
    /// actual text rendering to buffer.
    fn render_component_tab(&self, component: &ComponentInfo) {
        // TODO: Implement text rendering to buffer
        println!("Component: type={}", component.component_type);
        if let Some(test_id) = &component.test_id {
            println!("  test_id={}", test_id);
        }
    }

    /// Render the Stats tab showing performance metrics
    ///
    /// Displays:
    /// - Refresh count (future)
    /// - Render time (future)
    /// - Memory usage (future)
    ///
    /// # Note
    ///
    /// Currently outputs to stdout via `println!`. Will be replaced with
    /// actual text rendering and real metrics.
    fn render_stats_tab(&self, _component: &ComponentInfo) {
        // TODO: Implement text rendering to buffer and real metrics
        println!("Stats: refreshes=0");
    }
}

impl Default for Inspector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to create a test component
    fn create_test_component() -> ComponentInfo {
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

        // Default is Layout
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);

        // Switch to Component
        inspector.set_tab(InspectorTab::Component);
        assert_eq!(inspector.current_tab(), InspectorTab::Component);

        // Switch to Stats
        inspector.set_tab(InspectorTab::Stats);
        assert_eq!(inspector.current_tab(), InspectorTab::Stats);

        // Switch back to Layout
        inspector.set_tab(InspectorTab::Layout);
        assert_eq!(inspector.current_tab(), InspectorTab::Layout);
    }

    #[test]
    fn test_render_layout_tab() {
        let inspector = Inspector::new();
        let component = create_test_component();
        let mut buffer = vec![0u32; 800 * 600];

        // Should not panic
        inspector.render_details(&mut buffer, 800, 10, 10, &component);
    }

    #[test]
    fn test_render_component_tab() {
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::Component);

        let component = create_test_component();
        let mut buffer = vec![0u32; 800 * 600];

        // Should not panic
        inspector.render_details(&mut buffer, 800, 10, 10, &component);
    }

    #[test]
    fn test_render_stats_tab() {
        let mut inspector = Inspector::new();
        inspector.set_tab(InspectorTab::Stats);

        let component = create_test_component();
        let mut buffer = vec![0u32; 800 * 600];

        // Should not panic
        inspector.render_details(&mut buffer, 800, 10, 10, &component);
    }

    #[test]
    fn test_render_all_tabs() {
        let mut inspector = Inspector::new();
        let component = create_test_component();
        let mut buffer = vec![0u32; 800 * 600];

        // Test all tabs can render without panicking
        for tab in [
            InspectorTab::Layout,
            InspectorTab::Component,
            InspectorTab::Stats,
        ] {
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

        // Should handle missing test_id gracefully
        inspector.render_details(&mut buffer, 800, 0, 0, &component);
    }

    #[test]
    fn test_inspector_tab_equality() {
        assert_eq!(InspectorTab::Layout, InspectorTab::Layout);
        assert_eq!(InspectorTab::Component, InspectorTab::Component);
        assert_eq!(InspectorTab::Stats, InspectorTab::Stats);

        assert_ne!(InspectorTab::Layout, InspectorTab::Component);
        assert_ne!(InspectorTab::Component, InspectorTab::Stats);
        assert_ne!(InspectorTab::Stats, InspectorTab::Layout);
    }

    #[test]
    fn test_inspector_tab_clone() {
        let tab = InspectorTab::Component;
        let cloned = tab.clone();
        assert_eq!(tab, cloned);
    }

    #[test]
    fn test_inspector_tab_copy() {
        let tab = InspectorTab::Stats;
        let copied = tab; // Copy happens implicitly
        assert_eq!(tab, copied);
    }

    #[test]
    fn test_inspector_tab_debug() {
        // Just ensure Debug is implemented
        let tab = InspectorTab::Layout;
        let debug_str = format!("{:?}", tab);
        assert!(debug_str.contains("Layout"));
    }
}
