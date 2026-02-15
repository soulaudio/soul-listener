//! Debug manager - central coordinator

use super::state::DebugState;

/// Central coordinator for the debug system
///
/// The DebugManager owns and manages the debug state, providing controlled
/// access to debug features like panels, borders, inspector, and power monitoring.
pub struct DebugManager {
    state: DebugState,
}

impl DebugManager {
    /// Creates a new DebugManager with default state
    ///
    /// All debug features are initially disabled.
    pub fn new() -> Self {
        Self {
            state: DebugState::new(),
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
}
