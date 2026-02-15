//! Debug manager - central coordinator

use super::power_graph::PowerGraph;
use super::state::DebugState;
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Result of event handling
///
/// Indicates whether the debug system consumed the event or whether
/// it should be passed to other handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// The event was consumed by the debug system
    Consumed,
    /// The event was not handled and should be passed to other handlers
    NotHandled,
}

/// Central coordinator for the debug system
///
/// The DebugManager owns and manages the debug state, providing controlled
/// access to debug features like panels, borders, inspector, and power monitoring.
pub struct DebugManager {
    state: DebugState,
    power_graph: PowerGraph,
}

impl DebugManager {
    /// Creates a new DebugManager with default state
    ///
    /// All debug features are initially disabled.
    pub fn new() -> Self {
        Self {
            state: DebugState::new(),
            power_graph: PowerGraph::new(),
        }
    }

    /// Returns an immutable reference to the debug state
    pub fn state(&self) -> &DebugState {
        &self.state
    }

    /// Returns a mutable reference to the debug state
    ///
    /// Use this to toggle debug features or modify debug state.
    pub fn state_mut(&mut self) -> &mut DebugState {
        &mut self.state
    }

    /// Returns an immutable reference to the power graph
    pub fn power_graph(&self) -> &PowerGraph {
        &self.power_graph
    }

    /// Returns a mutable reference to the power graph
    ///
    /// Use this to add power samples or modify graph settings.
    pub fn power_graph_mut(&mut self) -> &mut PowerGraph {
        &mut self.power_graph
    }

    /// Handles window events for debug system
    ///
    /// Processes keyboard input to toggle debug features:
    /// - F12: Toggle debug panel visibility
    /// - F1: Toggle component border rendering
    /// - F2: Toggle inspector mode
    /// - F3: Toggle power consumption graph
    ///
    /// # Arguments
    ///
    /// * `event` - The window event to process
    ///
    /// # Returns
    ///
    /// * `EventResult::Consumed` - If the event was a debug hotkey
    /// * `EventResult::NotHandled` - If the event should be passed to other handlers
    ///
    /// # Example
    ///
    /// ```ignore
    /// match debug_manager.handle_event(&event) {
    ///     EventResult::Consumed => {
    ///         // Debug system handled the event, don't process further
    ///     }
    ///     EventResult::NotHandled => {
    ///         // Pass to application event handlers
    ///     }
    /// }
    /// ```
    pub fn handle_event(&mut self, event: &WindowEvent) -> EventResult {
        match event {
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    match key_code {
                        KeyCode::F12 => {
                            self.state.toggle_panel();
                            return EventResult::Consumed;
                        }
                        KeyCode::F1 => {
                            self.state.toggle_borders();
                            return EventResult::Consumed;
                        }
                        KeyCode::F2 => {
                            self.state.toggle_inspector();
                            return EventResult::Consumed;
                        }
                        KeyCode::F3 => {
                            self.state.toggle_power_graph();
                            return EventResult::Consumed;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        EventResult::NotHandled
    }
}

impl Default for DebugManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_manager_creation() {
        let manager = DebugManager::new();
        assert!(!manager.state().panel_visible);
        assert!(!manager.state().borders_enabled);
    }

    #[test]
    fn test_toggle_panel() {
        let mut manager = DebugManager::new();
        manager.state_mut().toggle_panel();
        assert!(manager.state().panel_visible);
        manager.state_mut().toggle_panel();
        assert!(!manager.state().panel_visible);
    }

    #[test]
    fn test_default() {
        let manager = DebugManager::default();
        assert!(!manager.state().panel_visible);
    }

    #[test]
    fn test_event_result_enum() {
        // Verify EventResult enum values
        assert_eq!(EventResult::Consumed, EventResult::Consumed);
        assert_eq!(EventResult::NotHandled, EventResult::NotHandled);
        assert_ne!(EventResult::Consumed, EventResult::NotHandled);
    }

    #[test]
    fn test_handle_event_non_keyboard() {
        let mut manager = DebugManager::new();

        // Test that non-keyboard events return NotHandled
        let event = WindowEvent::RedrawRequested;
        let result = manager.handle_event(&event);
        assert_eq!(result, EventResult::NotHandled);
    }

    // Note: Testing actual keyboard events is complex due to winit's KeyEvent
    // having private fields. The event handling logic is straightforward:
    // - F12 toggles panel (tested via toggle_panel)
    // - F1 toggles borders (tested via DebugState tests)
    // - F2 toggles inspector (tested via DebugState tests)
    // - F3 toggles power graph (tested via DebugState tests)
    //
    // Integration tests with actual keyboard input will be performed
    // in the emulator's integration test suite.

    #[test]
    fn test_state_toggles_via_manager() {
        let mut manager = DebugManager::new();

        // Test all toggles through manager interface
        assert!(!manager.state().panel_visible);
        manager.state_mut().toggle_panel();
        assert!(manager.state().panel_visible);

        assert!(!manager.state().borders_enabled);
        manager.state_mut().toggle_borders();
        assert!(manager.state().borders_enabled);

        assert!(!manager.state().inspector_mode);
        manager.state_mut().toggle_inspector();
        assert!(manager.state().inspector_mode);

        assert!(!manager.state().power_graph_enabled);
        manager.state_mut().toggle_power_graph();
        assert!(manager.state().power_graph_enabled);
    }

    #[test]
    fn test_power_graph_accessors() {
        use crate::debug::state::RefreshType;

        let mut manager = DebugManager::new();

        // Test immutable accessor
        assert_eq!(manager.power_graph().current_power(), 10.0);

        // Test mutable accessor
        manager
            .power_graph_mut()
            .add_sample(50.0, Some(RefreshType::Partial));
        assert_eq!(manager.power_graph().current_power(), 50.0);

        // Test average power
        manager
            .power_graph_mut()
            .add_sample(60.0, Some(RefreshType::Partial));
        assert_eq!(manager.power_graph().average_power(), 55.0);
    }
}
