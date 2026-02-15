//! Debug system showcase E2E test with realistic scenarios
//!
//! This test creates comprehensive screenshots demonstrating all debug features
//! with actual UI content at the real display size (480x800 portrait).

#![cfg(feature = "debug")]

use eink_emulator::debug::*;
use std::fs;
use std::path::Path;

/// Setup screenshot directory
fn setup_screenshot_dir() -> std::path::PathBuf {
    let dir = Path::new("target/debug_showcase");
    fs::create_dir_all(dir).expect("Failed to create screenshot directory");
    dir.to_path_buf()
}

/// Save framebuffer as PNG with timestamp
fn save_screenshot(buffer: &[u32], width: u32, height: u32, name: &str) {
    let dir = setup_screenshot_dir();
    let path = dir.join(format!("{}.png", name));

    // Convert ARGB to RGB
    let mut rgb_buffer = Vec::with_capacity((width * height * 3) as usize);
    for &pixel in buffer {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        rgb_buffer.push(r);
        rgb_buffer.push(g);
        rgb_buffer.push(b);
    }

    image::save_buffer(
        &path,
        &rgb_buffer,
        width,
        height,
        image::ColorType::Rgb8,
    )
    .unwrap_or_else(|e| eprintln!("Failed to save screenshot {}: {}", name, e));

    println!("📸 Screenshot saved: {}", path.display());
}

#[test]
fn test_showcase_01_borders_only() {
    println!("\n🎬 Showcase Test 1: Layout Borders (Portrait 480x800)");

    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 800;

    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0xFFFFFFFF; (WIDTH * HEIGHT) as usize]; // White background

    // Simulate a realistic UI layout
    let components = vec![
        // Header container
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (0, 0),
            size: (480, 60),
            test_id: Some("header".to_string()),
        },
        // Now Playing title
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (20, 20),
            size: (200, 24),
            test_id: Some("title".to_string()),
        },
        // Album art container
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (90, 100),
            size: (300, 300),
            test_id: Some("album-art".to_string()),
        },
        // Track info container
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (40, 420),
            size: (400, 120),
            test_id: Some("track-info".to_string()),
        },
        // Track name
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (60, 440),
            size: (360, 28),
            test_id: Some("track-name".to_string()),
        },
        // Artist name
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (60, 480),
            size: (360, 20),
            test_id: Some("artist-name".to_string()),
        },
        // Progress bar
        ComponentInfo {
            component_type: "ProgressBar".to_string(),
            position: (40, 560),
            size: (400, 8),
            test_id: Some("progress".to_string()),
        },
        // Time labels
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (40, 580),
            size: (50, 16),
            test_id: Some("time-current".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (390, 580),
            size: (50, 16),
            test_id: Some("time-total".to_string()),
        },
        // Control buttons
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (140, 640),
            size: (60, 60),
            test_id: Some("btn-prev".to_string()),
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (210, 640),
            size: (60, 60),
            test_id: Some("btn-play".to_string()),
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (280, 640),
            size: (60, 60),
            test_id: Some("btn-next".to_string()),
        },
    ];

    // Render borders
    renderer.render_borders(&mut buffer, WIDTH, HEIGHT, &components);

    save_screenshot(&buffer, WIDTH, HEIGHT, "01_borders_only");
    println!("  ✅ Rendered {} components with colored borders", components.len());
}

#[test]
fn test_showcase_02_panel_and_borders() {
    println!("\n🎬 Showcase Test 2: Debug Panel + Borders");

    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 800;

    let renderer = OverlayRenderer::new();
    let panel = DebugPanel::new();
    let mut buffer = vec![0xFFFFFFFF; (WIDTH * HEIGHT) as usize];
    let mut state = DebugState::new();
    state.panel_visible = true;

    // Smaller components to fit with panel
    let components = vec![
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (10, 10),
            size: (260, 200),
            test_id: Some("main".to_string()),
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (30, 50),
            size: (220, 40),
            test_id: Some("action-btn".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (30, 120),
            size: (220, 24),
            test_id: Some("status".to_string()),
        },
        ComponentInfo {
            component_type: "ProgressBar".to_string(),
            position: (30, 160),
            size: (220, 8),
            test_id: Some("loading".to_string()),
        },
    ];

    // Render borders first, then panel on top
    renderer.render_borders(&mut buffer, WIDTH, HEIGHT, &components);
    panel.render(&mut buffer, WIDTH, HEIGHT, &state);

    save_screenshot(&buffer, WIDTH, HEIGHT, "02_panel_and_borders");
    println!("  ✅ Debug panel visible with {} bordered components", components.len());
}

#[test]
fn test_showcase_03_power_graph() {
    println!("\n🎬 Showcase Test 3: Power Consumption Graph");

    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 800;

    let mut graph = PowerGraph::new();
    let mut buffer = vec![0xFFFFFFFF; (WIDTH * HEIGHT) as usize];

    // Simulate realistic power consumption pattern
    // Idle -> partial refresh -> idle -> full refresh -> idle
    let power_pattern = [
        (10.0, None, 20),                             // Idle state
        (60.0, Some(RefreshType::Partial), 5),       // Partial refreshes
        (10.0, None, 10),                             // Back to idle
        (210.0, Some(RefreshType::Full), 3),         // Full refresh spike
        (10.0, None, 15),                             // Idle again
        (60.0, Some(RefreshType::Partial), 8),       // More partial refreshes
        (10.0, None, 10),                             // Settle to idle
    ];

    for (power, refresh_type, count) in power_pattern {
        for _ in 0..count {
            graph.add_sample(power, refresh_type);
        }
    }

    // Render graph in upper portion of screen
    graph.render(&mut buffer, WIDTH, 10, 100);

    save_screenshot(&buffer, WIDTH, HEIGHT, "03_power_graph");
    println!("  ✅ Power graph rendered");
    println!("  ℹ️  Average power: {:.1}mW", graph.average_power());
    println!("  ℹ️  Current power: {:.1}mW", graph.current_power());
}

#[test]
fn test_showcase_04_complete_debug_ui() {
    println!("\n🎬 Showcase Test 4: Complete Debug UI (All Features)");

    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 800;

    let renderer = OverlayRenderer::new();
    let panel = DebugPanel::new();
    let mut graph = PowerGraph::new();
    let mut buffer = vec![0xFF282828; (WIDTH * HEIGHT) as usize]; // Dark background
    let mut state = DebugState::new();

    // Enable all debug features
    state.panel_visible = true;
    state.borders_enabled = true;
    state.power_graph_enabled = true;

    // Add power samples
    for i in 0..30 {
        let power = 10.0 + (i as f32 * 3.0).sin() * 50.0;
        let refresh = if i % 10 == 0 {
            Some(RefreshType::Full)
        } else if i % 3 == 0 {
            Some(RefreshType::Partial)
        } else {
            None
        };
        graph.add_sample(power, refresh);
    }

    // Component layout
    let components = vec![
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (10, 10),
            size: (260, 350),
            test_id: Some("content".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (30, 30),
            size: (220, 24),
            test_id: Some("header".to_string()),
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (30, 80),
            size: (220, 50),
            test_id: Some("btn1".to_string()),
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (30, 150),
            size: (220, 50),
            test_id: Some("btn2".to_string()),
        },
        ComponentInfo {
            component_type: "ProgressBar".to_string(),
            position: (30, 220),
            size: (220, 12),
            test_id: Some("progress".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (30, 250),
            size: (220, 20),
            test_id: Some("status".to_string()),
        },
    ];

    // Render all debug features
    renderer.render_borders(&mut buffer, WIDTH, HEIGHT, &components);
    graph.render(&mut buffer, WIDTH, 10, 400);
    panel.render(&mut buffer, WIDTH, HEIGHT, &state);

    save_screenshot(&buffer, WIDTH, HEIGHT, "04_complete_debug_ui");
    println!("  ✅ All debug features rendered:");
    println!("     • {} component borders", components.len());
    println!("     • Power graph (avg: {:.1}mW)", graph.average_power());
    println!("     • Debug panel (200px wide)");
}

#[test]
fn test_showcase_05_comparison_before_after() {
    println!("\n🎬 Showcase Test 5: Before/After Comparison");

    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 800;

    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0xFFFFFFFF; (WIDTH * HEIGHT) as usize];

    // Create a simple UI
    let components = vec![
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (40, 40),
            size: (400, 200),
            test_id: Some("card".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (60, 60),
            size: (360, 30),
            test_id: Some("title".to_string()),
        },
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (140, 150),
            size: (200, 60),
            test_id: Some("action".to_string()),
        },
    ];

    // BEFORE: No debug features
    save_screenshot(&buffer, WIDTH, HEIGHT, "05a_before_debug");

    // AFTER: With debug borders
    renderer.render_borders(&mut buffer, WIDTH, HEIGHT, &components);
    save_screenshot(&buffer, WIDTH, HEIGHT, "05b_after_debug");

    println!("  ✅ Before/After comparison screenshots saved");
}
