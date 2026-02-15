//! End-to-end visual tests for debug system with screenshot validation
//!
//! These tests validate the complete debug system by:
//! 1. Setting up the emulator with debug features
//! 2. Triggering various debug states
//! 3. Rendering to framebuffer
//! 4. Capturing screenshots
//! 5. Validating pixel-level correctness

#![cfg(feature = "debug")]

use eink_emulator::debug::*;
use std::fs;
use std::path::Path;

/// Helper to create screenshot output directory
fn setup_screenshot_dir() -> std::path::PathBuf {
    let dir = Path::new("target/debug_screenshots");
    fs::create_dir_all(dir).expect("Failed to create screenshot directory");
    dir.to_path_buf()
}

/// Helper to save framebuffer as PNG
fn save_screenshot(buffer: &[u32], width: u32, height: u32, name: &str) {
    let dir = setup_screenshot_dir();
    let path = dir.join(format!("{}.png", name));

    // Convert ARGB to RGB for image crate
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

/// Helper to count pixels of a specific color
fn count_pixels_of_color(buffer: &[u32], color: u32) -> usize {
    buffer.iter().filter(|&&px| px == color).count()
}

/// Helper to check if a pixel is approximately a color (allow some tolerance)
fn pixel_matches(pixel: u32, expected: u32, tolerance: u8) -> bool {
    let r1 = ((pixel >> 16) & 0xFF) as i32;
    let g1 = ((pixel >> 8) & 0xFF) as i32;
    let b1 = (pixel & 0xFF) as i32;

    let r2 = ((expected >> 16) & 0xFF) as i32;
    let g2 = ((expected >> 8) & 0xFF) as i32;
    let b2 = (expected & 0xFF) as i32;

    let diff = (r1 - r2).abs() + (g1 - g2).abs() + (b1 - b2).abs();
    diff <= tolerance as i32 * 3
}

#[test]
fn test_e2e_overlay_borders_render() {
    println!("\n🧪 E2E Test: Overlay Borders Rendering");

    // Setup
    let renderer = OverlayRenderer::new();
    let mut buffer = vec![0xFF000000; 800 * 600];  // Black background

    let components = vec![
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (100, 100),
            size: (200, 50),
            test_id: Some("play-button".to_string()),
        },
        ComponentInfo {
            component_type: "Label".to_string(),
            position: (100, 200),
            size: (200, 30),
            test_id: Some("title-label".to_string()),
        },
        ComponentInfo {
            component_type: "Container".to_string(),
            position: (50, 50),
            size: (300, 300),
            test_id: None,
        },
    ];

    // Render borders
    renderer.render_borders(&mut buffer, 800, 600, &components);

    // Validate: Check that borders were drawn
    let green_pixels = count_pixels_of_color(&buffer, 0xFF00FF80); // Button green
    let red_pixels = count_pixels_of_color(&buffer, 0xFFFF4040);   // Label red
    let blue_pixels = count_pixels_of_color(&buffer, 0xFF0080FF);  // Container blue

    println!("  ✅ Green pixels (Button): {}", green_pixels);
    println!("  ✅ Red pixels (Label): {}", red_pixels);
    println!("  ✅ Blue pixels (Container): {}", blue_pixels);

    assert!(green_pixels > 0, "Button border not rendered");
    assert!(red_pixels > 0, "Label border not rendered");
    assert!(blue_pixels > 0, "Container border not rendered");

    // Validate: Check specific border pixels
    // Top-left corner of button (100, 100)
    let button_corner = buffer[(100 * 800 + 100) as usize];
    assert_eq!(button_corner, 0xFF00FF80, "Button corner should be green");

    // Save screenshot for manual inspection
    save_screenshot(&buffer, 800, 600, "e2e_overlay_borders");

    println!("  ✅ Overlay borders E2E test passed");
}

#[test]
fn test_e2e_debug_panel_render() {
    println!("\n🧪 E2E Test: Debug Panel Rendering");

    // Setup
    let panel = DebugPanel::new();
    let mut buffer = vec![0xFFFFFFFF; 800 * 600];  // White background
    let mut state = DebugState::new();

    // Test 1: Panel hidden
    panel.render(&mut buffer, 800, 600, &state);
    let white_pixels_hidden = count_pixels_of_color(&buffer, 0xFFFFFFFF);

    println!("  ℹ️  Panel hidden - white pixels: {}", white_pixels_hidden);
    assert_eq!(
        white_pixels_hidden,
        800 * 600,
        "Buffer should be unchanged when panel hidden"
    );

    // Test 2: Panel visible
    state.panel_visible = true;
    panel.render(&mut buffer, 800, 600, &state);

    const PANEL_BG: u32 = 0xDC282828;  // Semi-transparent dark gray
    let panel_pixels = count_pixels_of_color(&buffer, PANEL_BG);

    println!("  ✅ Panel visible - panel pixels: {}", panel_pixels);
    assert!(panel_pixels > 0, "Panel background should be rendered");

    // Validate: Panel should be 200px wide on right side
    // Check that left side is still white, right side is panel color
    let left_side_pixel = buffer[(300 * 800 + 100) as usize];  // x=100 (left)
    let right_side_pixel = buffer[(300 * 800 + 700) as usize]; // x=700 (right, in panel)

    assert_eq!(left_side_pixel, 0xFFFFFFFF, "Left side should be white");
    assert_eq!(right_side_pixel, PANEL_BG, "Right side should be panel color");

    // Save screenshot for manual inspection
    save_screenshot(&buffer, 800, 600, "e2e_debug_panel");

    println!("  ✅ Debug panel E2E test passed");
}

#[test]
fn test_e2e_power_graph_render() {
    println!("\n🧪 E2E Test: Power Graph Rendering");

    // Setup
    let mut graph = PowerGraph::new();
    let mut buffer = vec![0xFF000000; 800 * 600];  // Black background

    // Add sample data
    for i in 0..50 {
        let power = 10.0 + (i as f32 * 2.0);  // Increasing power
        graph.add_sample(power, None);
    }

    // Render graph
    graph.render(&mut buffer, 800, 10, 10);

    // Validate: Check that green pixels were drawn (graph line)
    let green_pixels = count_pixels_of_color(&buffer, 0xFF00FF00);

    println!("  ✅ Green pixels (graph line): {}", green_pixels);
    assert!(green_pixels > 0, "Graph line should be rendered");
    assert!(green_pixels >= 40, "Graph should have multiple points");

    // Save screenshot for manual inspection
    save_screenshot(&buffer, 800, 600, "e2e_power_graph");

    println!("  ✅ Power graph E2E test passed");
}

#[test]
fn test_e2e_complete_debug_scene() {
    println!("\n🧪 E2E Test: Complete Debug Scene (All Features)");

    // Setup complete scene
    let overlay = OverlayRenderer::new();
    let panel = DebugPanel::new();
    let mut graph = PowerGraph::new();
    let mut state = DebugState::new();

    // Enable all debug features
    state.panel_visible = true;
    state.borders_enabled = true;
    state.power_graph_enabled = true;

    // Create test components
    let components = vec![
        ComponentInfo {
            component_type: "Button".to_string(),
            position: (50, 50),
            size: (150, 40),
            test_id: Some("btn1".to_string()),
        },
        ComponentInfo {
            component_type: "ProgressBar".to_string(),
            position: (50, 120),
            size: (200, 20),
            test_id: Some("progress".to_string()),
        },
    ];

    // Add power samples
    for i in 0..30 {
        graph.add_sample(10.0 + (i % 10) as f32 * 5.0, None);
    }

    // Create framebuffer
    let mut buffer = vec![0xFF202020; 800 * 600];  // Dark gray background

    // Layer 1: Render component borders
    if state.borders_enabled {
        overlay.render_borders(&mut buffer, 800, 600, &components);
    }

    // Layer 2: Render debug panel
    if state.panel_visible {
        panel.render(&mut buffer, 800, 600, &state);
    }

    // Layer 3: Render power graph (inside panel area)
    if state.power_graph_enabled {
        graph.render(&mut buffer, 800, 610, 10);
    }

    // Validate complete scene
    let green_pixels = count_pixels_of_color(&buffer, 0xFF00FF80);   // Button border
    let purple_pixels = count_pixels_of_color(&buffer, 0xFFC040FF);  // ProgressBar border
    let panel_pixels = count_pixels_of_color(&buffer, 0xDC282828);   // Panel background

    println!("  ✅ Green pixels (Button): {}", green_pixels);
    println!("  ✅ Purple pixels (ProgressBar): {}", purple_pixels);
    println!("  ✅ Panel pixels: {}", panel_pixels);

    assert!(green_pixels > 0, "Button borders should be visible");
    assert!(purple_pixels > 0, "ProgressBar borders should be visible");
    assert!(panel_pixels > 0, "Panel should be visible");

    // Save complete screenshot
    save_screenshot(&buffer, 800, 600, "e2e_complete_scene");

    println!("  ✅ Complete debug scene E2E test passed");
}

#[test]
fn test_e2e_toggle_states() {
    println!("\n🧪 E2E Test: Toggle State Changes");

    let mut state = DebugState::new();

    // Test all toggles
    println!("  Testing panel toggle...");
    assert!(!state.panel_visible);
    state.toggle_panel();
    assert!(state.panel_visible);
    state.toggle_panel();
    assert!(!state.panel_visible);

    println!("  Testing borders toggle...");
    assert!(!state.borders_enabled);
    state.toggle_borders();
    assert!(state.borders_enabled);

    println!("  Testing inspector toggle...");
    assert!(!state.inspector_mode);
    state.toggle_inspector();
    assert!(state.inspector_mode);

    println!("  Testing power graph toggle...");
    assert!(!state.power_graph_enabled);
    state.toggle_power_graph();
    assert!(state.power_graph_enabled);

    println!("  ✅ All toggles work correctly");
}

#[test]
fn test_e2e_color_accuracy() {
    println!("\n🧪 E2E Test: Color Accuracy Validation");

    let renderer = OverlayRenderer::new();
    // Test each component type color
    let test_cases = vec![
        ("Container", 0xFF0080FF),   // Blue
        ("Button", 0xFF00FF80),      // Green
        ("Label", 0xFFFF4040),       // Red
        ("ProgressBar", 0xFFC040FF), // Purple
        ("Unknown", 0xFFFFCC00),     // Yellow
    ];

    for (component_type, expected_color) in test_cases {
        let mut local_buffer = vec![0xFF000000; 400 * 400];

        let component = ComponentInfo {
            component_type: component_type.to_string(),
            position: (100, 100),
            size: (100, 50),
            test_id: None,
        };

        renderer.render_borders(&mut local_buffer, 400, 400, &[component]);

        // Check corner pixel
        let corner_pixel = local_buffer[(100 * 400 + 100) as usize];

        println!(
            "  {} - Expected: 0x{:08X}, Got: 0x{:08X}",
            component_type, expected_color, corner_pixel
        );

        assert_eq!(
            corner_pixel, expected_color,
            "{} color should match expected color",
            component_type
        );
    }

    println!("  ✅ All component colors accurate");
}

#[test]
fn test_e2e_performance_benchmark() {
    println!("\n🧪 E2E Test: Performance Benchmark");

    use std::time::Instant;

    let overlay = OverlayRenderer::new();
    let panel = DebugPanel::new();
    let mut graph = PowerGraph::new();
    let mut state = DebugState::new();
    state.panel_visible = true;
    state.borders_enabled = true;

    // Create realistic component count
    let mut components = Vec::new();
    for i in 0..20 {
        components.push(ComponentInfo {
            component_type: "Button".to_string(),
            position: (50, 50 + i * 25),
            size: (100, 20),
            test_id: Some(format!("btn{}", i)),
        });
    }

    // Add power samples
    for i in 0..100 {
        graph.add_sample(10.0 + (i % 20) as f32, None);
    }

    let mut buffer = vec![0xFF000000; 800 * 600];

    // Benchmark complete render
    let start = Instant::now();

    overlay.render_borders(&mut buffer, 800, 600, &components);
    panel.render(&mut buffer, 800, 600, &state);
    graph.render(&mut buffer, 800, 610, 10);

    let duration = start.elapsed();

    println!("  ⏱️  Render time: {:?}", duration);
    println!("  ⏱️  Render time (ms): {:.2}", duration.as_secs_f64() * 1000.0);

    // Assert performance target: <5ms
    assert!(
        duration.as_millis() < 5,
        "Render should complete in <5ms, got {}ms",
        duration.as_millis()
    );

    println!("  ✅ Performance target met (<5ms)");
}

#[test]
fn test_e2e_edge_cases() {
    println!("\n🧪 E2E Test: Edge Cases");

    let overlay = OverlayRenderer::new();
    let mut buffer = vec![0xFF000000; 800 * 600];

    // Edge case 1: Empty components
    println!("  Testing empty components...");
    overlay.render_borders(&mut buffer, 800, 600, &[]);
    let black_pixels = count_pixels_of_color(&buffer, 0xFF000000);
    assert_eq!(black_pixels, 800 * 600, "Empty components should not render");

    // Edge case 2: Component at screen edge
    println!("  Testing component at screen edge...");
    let edge_component = ComponentInfo {
        component_type: "Button".to_string(),
        position: (0, 0),
        size: (50, 50),
        test_id: None,
    };
    overlay.render_borders(&mut buffer, 800, 600, &[edge_component]);
    // Should not panic

    // Edge case 3: Component partially off-screen
    println!("  Testing component partially off-screen...");
    let offscreen_component = ComponentInfo {
        component_type: "Button".to_string(),
        position: (750, 550),
        size: (100, 100),  // Extends beyond 800x600
        test_id: None,
    };
    buffer.fill(0xFF000000);
    overlay.render_borders(&mut buffer, 800, 600, &[offscreen_component]);
    // Should not panic, should clip gracefully

    // Edge case 4: Very small component (1x1)
    println!("  Testing 1x1 component...");
    let tiny_component = ComponentInfo {
        component_type: "Button".to_string(),
        position: (400, 300),
        size: (1, 1),
        test_id: None,
    };
    buffer.fill(0xFF000000);
    overlay.render_borders(&mut buffer, 800, 600, &[tiny_component]);
    let pixel_400_300 = buffer[(300 * 800 + 400) as usize];
    assert_eq!(pixel_400_300, 0xFF00FF80, "1x1 component should render single pixel");

    println!("  ✅ All edge cases handled correctly");
}

#[test]
fn test_e2e_layering_order() {
    println!("\n🧪 E2E Test: Layer Rendering Order");

    let overlay = OverlayRenderer::new();
    let panel = DebugPanel::new();
    let mut state = DebugState::new();
    state.panel_visible = true;

    let mut buffer = vec![0xFFFFFFFF; 800 * 600];  // White background

    // Create component that overlaps with panel area
    let component = ComponentInfo {
        component_type: "Button".to_string(),
        position: (550, 100),  // x=550 is in panel area (starts at 600)
        size: (100, 50),
        test_id: None,
    };

    // Render in correct order: borders first, then panel
    overlay.render_borders(&mut buffer, 800, 600, &[component]);
    panel.render(&mut buffer, 800, 600, &state);

    // Check layering: panel should be on top
    let panel_area_pixel = buffer[(100 * 800 + 650) as usize];
    assert_eq!(
        panel_area_pixel, 0xDC282828,
        "Panel should be rendered on top of borders"
    );

    println!("  ✅ Layering order correct (panel on top)");
}
