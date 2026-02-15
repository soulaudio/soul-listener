//! Debug state management

use std::time::Instant;

/// Component information for debugging
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub component_type: String,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub test_id: Option<String>,
}

/// Power consumption sample
#[derive(Debug, Clone, Copy)]
pub struct PowerSample {
    pub timestamp: Instant,
    pub power_mw: f32,
    pub refresh_type: Option<RefreshType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshType {
    Full,
    Partial,
    Fast,
}

/// Debug system state
pub struct DebugState {
    pub panel_visible: bool,
    pub borders_enabled: bool,
    pub inspector_mode: bool,
    pub power_graph_enabled: bool,
    pub hovered_component: Option<ComponentInfo>,
    pub selected_component: Option<ComponentInfo>,
    pub power_history: Vec<PowerSample>,  // Will be ring buffer later
}

impl Default for DebugState {
    fn default() -> Self {
        Self {
            panel_visible: false,
            borders_enabled: false,
            inspector_mode: false,
            power_graph_enabled: false,
            hovered_component: None,
            selected_component: None,
            power_history: Vec::new(),
        }
    }
}

impl DebugState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    pub fn toggle_borders(&mut self) {
        self.borders_enabled = !self.borders_enabled;
    }

    pub fn toggle_inspector(&mut self) {
        self.inspector_mode = !self.inspector_mode;
    }

    pub fn toggle_power_graph(&mut self) {
        self.power_graph_enabled = !self.power_graph_enabled;
    }
}
