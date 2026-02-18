//! Integration tests for the debug system
//!
//! These tests verify the integration between debug system components and their
//! interaction with the emulator.

#![cfg(feature = "debug")]

use eink_emulator::debug::*;
use eink_emulator::Emulator;

#[test]
fn test_debug_manager_not_in_headless() {
    let emulator = Emulator::headless(480, 800);

    // Headless mode should not have debug manager (security/performance)
    assert!(emulator.debug_manager().is_none());
}

// Note: Cannot test windowed emulator in test suite on Windows due to winit's
// requirement that EventLoop must be created on the main thread. The windowed
// emulator with debug manager is tested in examples instead.

#[test]
fn test_debug_state_toggles() {
    let mut state = DebugState::new();

    // Initial state - all disabled
    assert!(!state.panel_visible);
    assert!(!state.borders_enabled);
    assert!(!state.inspector_mode);
    assert!(!state.power_graph_enabled);

    // Test panel toggle
    state.toggle_panel();
    assert!(state.panel_visible);
    state.toggle_panel();
    assert!(!state.panel_visible);

    // Test borders toggle
    state.toggle_borders();
    assert!(state.borders_enabled);
    state.toggle_borders();
    assert!(!state.borders_enabled);

    // Test inspector toggle
    state.toggle_inspector();
    assert!(state.inspector_mode);
    state.toggle_inspector();
    assert!(!state.inspector_mode);

    // Test power graph toggle
    state.toggle_power_graph();
    assert!(state.power_graph_enabled);
    state.toggle_power_graph();
    assert!(!state.power_graph_enabled);
}

#[test]
fn test_overlay_renderer_basic() {
    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0u32; 800 * 600];

    let components = vec![ComponentInfo {
        component_type: "Button".to_string(),
        position: (100, 100),
        size: (120, 40),
        test_id: None,
    }];

    renderer.render_borders(&mut buffer, 800, 600, &components);

    // Verify border was drawn (check top-left corner)
    let idx = (100 * 800 + 100) as usize;
    assert_ne!(buffer[idx], 0, "Border pixel should be non-zero");

    // Verify it's the correct color (green for Button)
    assert_eq!(buffer[idx], 0xFF00FF80);
}

#[test]
fn test_overlay_renderer_multiple_components() {
    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0u32; 800 * 600];

    let components = vec![
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (10, 10),
            size: (200, 150),
            test_id: None,
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (50, 50),
            size: (100, 40),
            test_id: Some("play-button".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (50, 100),
            size: (100, 20),
            test_id: None,
        },
    ];

    renderer.render_borders(&mut buffer, 800, 600, &components);

    // Check Container (blue)
    let container_idx = (10 * 800 + 10) as usize;
    assert_eq!(buffer[container_idx], 0xFF0080FF);

    // Check Button (green)
    let button_idx = (50 * 800 + 50) as usize;
    assert_eq!(buffer[button_idx], 0xFF00FF80);

    // Check Label (red)
    let label_idx = (100 * 800 + 50) as usize;
    assert_eq!(buffer[label_idx], 0xFFFF4040);
}

#[test]
fn test_power_graph_samples() {
    let mut graph = PowerGraph::new();

    // Initial state - no samples
    assert_eq!(graph.current_power(), 10.0); // Baseline

    // Add idle sample
    graph.add_sample(15.0, None);
    assert_eq!(graph.current_power(), 15.0);

    // Add partial refresh sample
    graph.add_sample(60.0, Some(RefreshType::Partial));
    assert_eq!(graph.current_power(), 60.0);

    // Add full refresh sample
    graph.add_sample(210.0, Some(RefreshType::Full));
    assert_eq!(graph.current_power(), 210.0);

    // Check average
    let avg = graph.average_power();
    assert_eq!(avg, (15.0 + 60.0 + 210.0) / 3.0);
}

#[test]
fn test_power_graph_estimate() {
    let graph = PowerGraph::new();

    // Test power estimates
    assert_eq!(graph.estimate_power(None), 10.0);
    assert_eq!(graph.estimate_power(Some(RefreshType::Full)), 210.0);
    assert_eq!(graph.estimate_power(Some(RefreshType::Partial)), 60.0);
    assert_eq!(graph.estimate_power(Some(RefreshType::Fast)), 60.0);
}

#[test]
fn test_debug_panel_rendering() {
    let panel = DebugPanel::new();
    let mut buffer = vec![0xFFFFFFFF; 800 * 600];
    let mut state = DebugState::new();

    // Panel hidden - buffer unchanged
    panel.render(&mut buffer, 800, 600, &state);
    assert_eq!(buffer[0], 0xFFFFFFFF);
    assert_eq!(buffer[(0 * 800 + 700) as usize], 0xFFFFFFFF);

    // Panel visible - background rendered
    state.panel_visible = true;
    panel.render(&mut buffer, 800, 600, &state);

    // Check pixel in panel area (right 200px, starting at x=600)
    let panel_pixel = buffer[(0 * 800 + 700) as usize];
    assert_ne!(panel_pixel, 0xFFFFFFFF, "Panel should render background");
    assert_eq!(panel_pixel, 0xDC282828); // Panel background color

    // Check pixel outside panel area
    let outside_pixel = buffer[(0 * 800 + 400) as usize];
    assert_eq!(
        outside_pixel, 0xFFFFFFFF,
        "Outside panel should be unchanged"
    );
}

#[test]
fn test_debug_panel_boundary() {
    let panel = DebugPanel::new();
    let mut buffer = vec![0xFFFFFFFF; 800 * 600];
    let mut state = DebugState::new();
    state.panel_visible = true;

    panel.render(&mut buffer, 800, 600, &state);

    // Panel should start at x = 800 - 200 = 600
    let before_panel = buffer[(100 * 800 + 599) as usize];
    assert_eq!(
        before_panel, 0xFFFFFFFF,
        "Just before panel should be unchanged"
    );

    let at_panel_start = buffer[(100 * 800 + 600) as usize];
    assert_eq!(
        at_panel_start, 0xDC282828,
        "Panel start should have background"
    );

    let at_panel_end = buffer[(100 * 800 + 799) as usize];
    assert_eq!(at_panel_end, 0xDC282828, "Panel end should have background");
}

#[test]
fn test_component_info_creation() {
    let info = ComponentInfo {
        component_type: "ProgressBar".to_string(),
        position: (50, 100),
        size: (200, 20),
        test_id: Some("progress-1".to_string()),
    };

    assert_eq!(info.component_type, "ProgressBar");
    assert_eq!(info.position, (50, 100));
    assert_eq!(info.size, (200, 20));
    assert_eq!(info.test_id, Some("progress-1".to_string()));
}

#[test]
fn test_refresh_type_enum() {
    // Verify enum discriminants
    assert_ne!(RefreshType::Full, RefreshType::Partial);
    assert_ne!(RefreshType::Full, RefreshType::Fast);
    assert_ne!(RefreshType::Partial, RefreshType::Fast);
    assert_eq!(RefreshType::Full, RefreshType::Full);
    assert_eq!(RefreshType::Partial, RefreshType::Partial);
    assert_eq!(RefreshType::Fast, RefreshType::Fast);
}

#[test]
fn test_debug_manager_integration() {
    let mut manager = DebugManager::new();

    // Initial state
    assert!(!manager.state().panel_visible);
    assert!(!manager.state().borders_enabled);

    // Toggle panel
    manager.state_mut().toggle_panel();
    assert!(manager.state().panel_visible);

    // Toggle borders
    manager.state_mut().toggle_borders();
    assert!(manager.state().borders_enabled);

    // Access power graph
    assert_eq!(manager.power_graph().current_power(), 10.0);

    // Add power sample
    manager
        .power_graph_mut()
        .add_sample(50.0, Some(RefreshType::Partial));
    assert_eq!(manager.power_graph().current_power(), 50.0);
}

#[test]
fn test_debug_manager_event_result() {
    // Verify EventResult enum
    assert_eq!(EventResult::Consumed, EventResult::Consumed);
    assert_eq!(EventResult::NotHandled, EventResult::NotHandled);
    assert_ne!(EventResult::Consumed, EventResult::NotHandled);
}

#[test]
fn test_overlay_renderer_empty_components() {
    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0u32; 100 * 100];

    // Render with no components - should not panic
    renderer.render_borders(&mut buffer, 100, 100, &[]);

    // Buffer should remain unchanged
    assert!(buffer.iter().all(|&pixel| pixel == 0));
}

#[test]
fn test_power_graph_ring_buffer() {
    let mut graph = PowerGraph::new();

    // Add many samples to test ring buffer behavior
    for i in 0..350 {
        graph.add_sample(i as f32, None);
    }

    // Should only keep last 300 samples
    // (MAX_SAMPLES is 300, see power_graph.rs)
    assert_eq!(graph.current_power(), 349.0);

    // Average should be over recent samples only
    let avg = graph.average_power();
    // Average of 50..350 (300 samples)
    let expected_avg = (50.0 + 349.0) / 2.0; // Arithmetic mean of range
    assert!(
        (avg - expected_avg).abs() < 1.0,
        "Average should be ~{}, got {}",
        expected_avg,
        avg
    );
}

#[test]
fn test_debug_state_hovered_and_selected() {
    let mut state = DebugState::new();

    // Initially no hover or selection
    assert!(state.hovered_component.is_none());
    assert!(state.selected_component.is_none());

    // Set hovered component
    state.hovered_component = Some(ComponentInfo {
        component_type: "Button".to_string(),
        position: (10, 10),
        size: (100, 40),
        test_id: Some("hover-test".to_string()),
    });
    assert!(state.hovered_component.is_some());

    // Set selected component
    state.selected_component = Some(ComponentInfo {
        component_type: "Label".to_string(),
        position: (20, 20),
        size: (80, 20),
        test_id: Some("select-test".to_string()),
    });
    assert!(state.selected_component.is_some());

    // Clear selections
    state.hovered_component = None;
    state.selected_component = None;
    assert!(state.hovered_component.is_none());
    assert!(state.selected_component.is_none());
}

#[test]
fn test_full_debug_workflow() {
    // Test a complete debug workflow
    let mut manager = DebugManager::new();
    let overlay = OverlayRenderer::new();
    let panel = DebugPanel::new();
    let mut buffer = vec![0u32; 800 * 600];

    // 1. Enable debug features
    manager.state_mut().toggle_panel();
    manager.state_mut().toggle_borders();

    // 2. Add some power samples
    manager.power_graph_mut().add_sample(10.0, None);
    manager
        .power_graph_mut()
        .add_sample(210.0, Some(RefreshType::Full));
    manager.power_graph_mut().add_sample(15.0, None);

    // 3. Create component layout
    let components = vec![
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (0, 0),
            size: (800, 600),
            test_id: None,
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (100, 100),
            size: (150, 50),
            test_id: Some("main-button".to_string()),
        },
    ];

    // 4. Render debug overlays
    overlay.render_borders(&mut buffer, 800, 600, &components);
    panel.render(&mut buffer, 800, 600, manager.state());

    // 5. Verify rendering
    // Panel should be visible
    let panel_pixel = buffer[(0 * 800 + 700) as usize];
    assert_eq!(panel_pixel, 0xDC282828);

    // Component borders should be visible
    let container_border = buffer[(0 * 800 + 0) as usize];
    assert_eq!(container_border, 0xFF0080FF); // Container is blue

    let button_border = buffer[(100 * 800 + 100) as usize];
    assert_eq!(button_border, 0xFF00FF80); // Button is green

    // 6. Verify power graph state
    assert_eq!(manager.power_graph().current_power(), 15.0);
    // The manager pre-seeds 20 idle samples (10.0 mW each) so the average
    // includes those plus our 3 explicitly added samples.
    let avg = manager.power_graph().average_power();
    let expected_avg = (20.0_f32 * 10.0 + 10.0 + 210.0 + 15.0) / 23.0;
    assert!(
        (avg - expected_avg).abs() < 0.01,
        "avg={avg} expected={expected_avg}"
    );
}

#[test]
fn test_overlay_renderer_bounds_checking() {
    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0u32; 100 * 100];

    // Component partially off-screen
    let components = vec![ComponentInfo {
        component_type: "Button".to_string(),
        position: (-10, -10),
        size: (50, 50),
        test_id: None,
    }];

    // Should not panic
    renderer.render_borders(&mut buffer, 100, 100, &components);

    // Visible part should be rendered
    // Bottom-right corner of component at (39, 39)
    let idx = (39 * 100 + 39) as usize;
    assert_eq!(buffer[idx], 0xFF00FF80);
}

#[test]
fn test_power_graph_render() {
    let mut graph = PowerGraph::new();
    let mut buffer = vec![0u32; 1000 * 1000];

    // Add some samples
    for i in 0..50 {
        graph.add_sample(10.0 + (i as f32) * 2.0, None);
    }

    // Render graph
    graph.render(&mut buffer, 1000, 100, 100);

    // Should have drawn some green pixels
    let green_pixels = buffer.iter().filter(|&&px| px == 0xFF00FF00).count();
    assert!(green_pixels > 0, "Power graph should render green pixels");
}

#[test]
fn test_debug_panel_different_screen_sizes() {
    let panel = DebugPanel::new();
    let mut state = DebugState::new();
    state.panel_visible = true;

    // Test with 1024x768
    let mut buffer = vec![0xFFFFFFFF; 1024 * 768];
    panel.render(&mut buffer, 1024, 768, &state);

    // Panel starts at x = 1024 - 200 = 824
    let panel_pixel = buffer[(0 * 1024 + 900) as usize];
    assert_eq!(panel_pixel, 0xDC282828);

    let before_panel = buffer[(0 * 1024 + 700) as usize];
    assert_eq!(before_panel, 0xFFFFFFFF);
}
