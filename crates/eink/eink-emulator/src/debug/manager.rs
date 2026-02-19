//! Debug manager - central coordinator

use super::inspector::Inspector;
use super::power_graph::PowerGraph;
use super::state::{ComponentInfo, DebugState};
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

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
    /// Persistent inspector instance — retains the active tab across re-renders.
    inspector: Inspector,
    modifiers: ModifiersState,
    /// Last known cursor position in physical window pixels (from CursorMoved events)
    cursor_pos: Option<(f64, f64)>,
    /// Timestamp of the last idle power sample added by `maybe_add_idle_sample`.
    last_idle_sample_time: std::time::Instant,
}

impl DebugManager {
    /// Creates a new DebugManager with default state
    ///
    /// All debug features are initially disabled.  The power graph is pre-seeded
    /// with a handful of idle-power samples so the chart shows a baseline
    /// immediately when the debug panel is first opened.
    pub fn new() -> Self {
        let mut power_graph = PowerGraph::new();
        // Pre-seed with idle samples so the history chart is never completely empty
        let idle_mw = power_graph.estimate_power(None);
        for _ in 0..20 {
            power_graph.add_sample(idle_mw, None);
        }
        Self {
            state: DebugState::new(),
            power_graph,
            inspector: Inspector::new(),
            modifiers: ModifiersState::empty(),
            cursor_pos: None,
            last_idle_sample_time: std::time::Instant::now(),
        }
    }

    /// Returns the last known cursor position in physical window pixels, if any.
    pub fn cursor_pos(&self) -> Option<(f64, f64)> {
        self.cursor_pos
    }

    /// Add an idle power sample if ≥500 ms have elapsed since the last one.
    ///
    /// Call this once per rendered frame to keep the power graph advancing
    /// continuously even when no display refreshes occur.
    pub fn maybe_add_idle_sample(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_idle_sample_time).as_millis() >= 500 {
            let idle_mw = self.power_graph.estimate_power(None);
            self.power_graph.add_sample(idle_mw, None);
            self.last_idle_sample_time = now;
        }
    }

    /// Update `state.hovered_component` based on the current cursor position.
    ///
    /// Maps physical window cursor coords → display coords (dividing by `scale`),
    /// then finds the smallest registered component (or auto-zone) that contains
    /// the cursor.  Sets `state.hovered_component` to `None` when the cursor is
    /// outside the display area or no components are registered.
    ///
    /// # Arguments
    /// * `disp_w` / `disp_h` – display dimensions in physical pixels (pre-rotation)
    /// * `scale`              – integer pixel-scale factor applied to the display
    pub fn update_hovered_component(&mut self, disp_w: u32, disp_h: u32, scale: u32) {
        let (cx, cy) = match self.cursor_pos {
            Some(pos) => pos,
            None => {
                self.state.hovered_component = None;
                return;
            }
        };

        // Map physical window coords to display coords (naive — ignores rotation).
        let scale_f = scale.max(1) as f64;
        let dx = (cx / scale_f) as i32;
        let dy = (cy / scale_f) as i32;

        // Cursor outside display bounds?
        if dx < 0 || dy < 0 || dx >= disp_w as i32 || dy >= disp_h as i32 {
            self.state.hovered_component = None;
            return;
        }

        // Use registered components or fall back to auto-zones.
        let components: Vec<ComponentInfo> = if !self.state.registered_components.is_empty() {
            self.state.registered_components.clone()
        } else {
            let header_h = (disp_h / 7).max(1);
            let footer_h = (disp_h / 7).max(1);
            let content_h = disp_h.saturating_sub(header_h + footer_h);
            let content_y = header_h as i32;
            let footer_y = (header_h + content_h) as i32;
            vec![
                ComponentInfo {
                    component_type: "Container".to_string(),
                    position: (0, 0),
                    size: (disp_w, disp_h),
                    test_id: Some("display-root".to_string()),
                    ..Default::default()
                },
                ComponentInfo {
                    component_type: "Label".to_string(),
                    position: (0, 0),
                    size: (disp_w, header_h),
                    test_id: Some("header".to_string()),
                    ..Default::default()
                },
                ComponentInfo {
                    component_type: "Button".to_string(),
                    position: (0, content_y),
                    size: (disp_w, content_h),
                    test_id: Some("content".to_string()),
                    ..Default::default()
                },
                ComponentInfo {
                    component_type: "ProgressBar".to_string(),
                    position: (0, footer_y),
                    size: (disp_w, footer_h),
                    test_id: Some("footer".to_string()),
                    ..Default::default()
                },
            ]
        };

        // Find the smallest (innermost) component that contains the cursor.
        let mut best: Option<ComponentInfo> = None;
        let mut best_area: u64 = u64::MAX;
        for comp in &components {
            let x0 = comp.position.0;
            let y0 = comp.position.1;
            let x1 = x0 + comp.size.0 as i32;
            let y1 = y0 + comp.size.1 as i32;
            if dx >= x0 && dx < x1 && dy >= y0 && dy < y1 {
                let area = comp.size.0 as u64 * comp.size.1 as u64;
                if area < best_area {
                    best_area = area;
                    best = Some(comp.clone());
                }
            }
        }
        self.state.hovered_component = best;

        // When inspect mode is active, sync the hovered component to the scene
        // tree selection so it highlights in the SCENE tab and the metadata pane
        // updates immediately on hover.
        if self.state.inspector_mode && self.state.panel_visible {
            if let Some(ref hov) = self.state.hovered_component.clone() {
                let rows = self.state.build_scene_rows();
                if let Some(row_idx) = rows.iter().position(|r| {
                    let comp = &self.state.registered_components[r.comp_idx];
                    comp.position == hov.position && comp.size == hov.size
                }) {
                    self.state.scene_selected = Some(row_idx);
                    const VISIBLE: usize = 10;
                    if row_idx < self.state.scene_scroll {
                        self.state.scene_scroll = row_idx;
                    } else if row_idx >= self.state.scene_scroll + VISIBLE {
                        self.state.scene_scroll = row_idx + 1 - VISIBLE;
                    }
                }
            }
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

    /// Returns an immutable reference to the persistent inspector.
    pub fn inspector(&self) -> &Inspector {
        &self.inspector
    }

    /// Returns a mutable reference to the persistent inspector.
    pub fn inspector_mut(&mut self) -> &mut Inspector {
        &mut self.inspector
    }

    /// Handle a mouse click at panel-local coordinates `(px, py)`.
    ///
    /// Hit-test priority (highest first):
    /// 1. **Scene tree rows** — select component; second click toggles collapse.
    /// 2. **Inspect-mode toggle zone** — toggle inspector on/off (Scene tab only).
    /// 3. **Tab bar zone** — switch active tab.
    ///
    /// Returns `true` when the panel consumed the click and the caller should
    /// re-render; `false` when the click was outside any interactive area.
    pub fn handle_panel_click(&mut self, px: f64, py: f64, panel_w: u32, panel_h: u32) -> bool {
        use crate::debug::panel::{
            compute_scene_hits, FONT_ASCENT, INSPECT_HIT_Y_END, INSPECT_HIT_Y_START, TAB_HIT_Y_END,
            TAB_HIT_Y_START, TREE_BASELINE_CY,
        };
        use crate::debug::state::DebugTab;

        if !self.state.panel_visible {
            return false;
        }

        let y = py as u32;
        let x = px as u32;

        // ── Priority 1: tree rows (specific per-row rects) ────────────────
        if self.state.active_tab == DebugTab::Scene {
            let hits = compute_scene_hits(panel_w, panel_h, &self.state);
            for hit in &hits {
                if y >= hit.y && y < hit.y + hit.h {
                    let already_selected = self.state.scene_selected == Some(hit.abs_idx);
                    self.state.scene_selected = Some(hit.abs_idx);

                    let rows = self.state.build_scene_rows();
                    if let Some(row) = rows.get(hit.abs_idx) {
                        let comp = self.state.registered_components[row.comp_idx].clone();
                        self.state.selected_component = Some(comp.clone());

                        // Second click on a collapsible node → toggle.
                        if already_selected && (row.has_children || row.is_label_group) {
                            self.state.toggle_node_collapsed(&comp);
                        }
                    }
                    return true;
                }
            }

            // ── Priority 2: inspect-mode toggle ───────────────────────────
            if (INSPECT_HIT_Y_START..INSPECT_HIT_Y_END).contains(&y) {
                self.state.toggle_inspector();
                return true;
            }
        }

        // ── Priority 3: tab bar ───────────────────────────────────────────
        if (TAB_HIT_Y_START..TAB_HIT_Y_END).contains(&y) {
            let tab_w = (panel_w.saturating_sub(1)) / 3;
            if tab_w > 0 {
                let tab_x = x.saturating_sub(1);
                let tab_idx = (tab_x / tab_w).min(2) as usize;
                self.state.active_tab =
                    [DebugTab::Scene, DebugTab::Display, DebugTab::Power][tab_idx];
            }
            return true;
        }

        // Silence unused-variable warnings from the constants we imported.
        let _ = (FONT_ASCENT, TREE_BASELINE_CY);

        false
    }

    /// Handles window events for debug system
    ///
    /// Processes keyboard input to toggle debug features:
    /// - Ctrl+1: Toggle debug panel visibility
    /// - Ctrl+2: Toggle component border rendering
    /// - Ctrl+3: Toggle inspector mode
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
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = Some((position.x, position.y));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    // Ctrl+1-4 hotkeys (always active)
                    if self.modifiers.control_key() {
                        match key_code {
                            KeyCode::Digit1 => {
                                self.state.toggle_panel();
                                return EventResult::Consumed;
                            }
                            KeyCode::Digit2 => {
                                self.state.toggle_borders();
                                return EventResult::Consumed;
                            }
                            KeyCode::Digit3 => {
                                self.state.toggle_inspector();
                                return EventResult::Consumed;
                            }
                            _ => {}
                        }
                    }

                    // Inspector tab cycling (Tab key when inspector mode is active)
                    if self.state.inspector_mode && key_code == KeyCode::Tab {
                        self.inspector.next_tab();
                        return EventResult::Consumed;
                    }

                    // Panel tab cycling (Tab key when panel is open)
                    if self.state.panel_visible && key_code == KeyCode::Tab {
                        self.state.cycle_tab();
                        return EventResult::Consumed;
                    }

                    // Scene tree navigation (arrow keys active when panel is open)
                    if self.state.panel_visible {
                        const VISIBLE: usize = 10;
                        match key_code {
                            KeyCode::ArrowDown | KeyCode::ArrowUp => {
                                let rows = self.state.build_scene_rows();
                                if !rows.is_empty() {
                                    let n = rows.len();
                                    let cur = self.state.scene_selected.unwrap_or(0);
                                    let next = if key_code == KeyCode::ArrowDown {
                                        (cur + 1).min(n.saturating_sub(1))
                                    } else {
                                        cur.saturating_sub(1)
                                    };
                                    self.state.scene_selected = Some(next);
                                    // Auto-scroll to keep selection visible.
                                    if next < self.state.scene_scroll {
                                        self.state.scene_scroll = next;
                                    } else if next >= self.state.scene_scroll + VISIBLE {
                                        self.state.scene_scroll = next + 1 - VISIBLE;
                                    }
                                    return EventResult::Consumed;
                                }
                            }
                            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter => {
                                let rows = self.state.build_scene_rows();
                                if let Some(sel) = self.state.scene_selected {
                                    if sel < rows.len() {
                                        let row = &rows[sel];
                                        if row.has_children || row.is_label_group {
                                            let comp = self.state.registered_components
                                                [row.comp_idx]
                                                .clone();
                                            self.state.toggle_node_collapsed(&comp);
                                        }
                                    }
                                }
                                return EventResult::Consumed;
                            }
                            _ => {}
                        }
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

        // Test that another sample is correctly stored (average is skewed by
        // the baseline idle samples pre-seeded in new(), so check current instead).
        manager
            .power_graph_mut()
            .add_sample(60.0, Some(RefreshType::Partial));
        assert_eq!(manager.power_graph().current_power(), 60.0);
    }
}
