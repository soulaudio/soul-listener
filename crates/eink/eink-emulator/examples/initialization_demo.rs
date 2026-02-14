//! Initialization Sequence Demo
//!
//! Demonstrates the 7-step initialization process with visual feedback.
//!
//! Run with: cargo run --example initialization_demo --target x86_64-pc-windows-msvc

use eink_emulator::{DisplayDriver, Emulator, InitializationState};
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() {
    println!("==============================================");
    println!("E-Ink Initialization Sequence Demo");
    println!("==============================================\n");

    // Create emulator
    println!("Creating emulator (250×122 pixels)...");
    let mut emulator = Emulator::new(250, 122);

    // Enable initialization requirement
    println!("Enabling initialization requirement...");
    emulator.set_requires_initialization(true);

    // Check initial state
    println!("Initial state: {:?}\n", emulator.init_state());
    assert_eq!(emulator.init_state(), &InitializationState::Uninitialized);

    // Try to refresh without initialization (should fail)
    println!("Attempting refresh without initialization...");
    match emulator.refresh_full().await {
        Ok(_) => println!("  ❌ ERROR: Refresh should have failed!"),
        Err(e) => println!("  ✓ Expected error: {}\n", e),
    }

    // Perform initialization
    println!("Starting 7-step initialization sequence:");
    println!("  Step 1: Power settling (100ms)");
    println!("  Step 2: Panel detection (50ms)");
    println!("  Step 3: Temperature sensor (20ms)");
    println!("  Step 4: Waveform tables (200ms)");
    println!("  Step 5: VCOM calibration (100ms)");
    println!("  Step 6: Checkerboard pattern (500ms) - VISUAL");
    println!("  Step 7: Clear to white (1000ms) - VISUAL\n");

    println!("Initializing display...");
    let start = std::time::Instant::now();
    emulator.initialize().await.unwrap();
    let elapsed = start.elapsed();

    println!("✓ Initialization complete in {}ms\n", elapsed.as_millis());

    // Check state after initialization
    println!("Final state: {:?}", emulator.init_state());
    assert_eq!(emulator.init_state(), &InitializationState::Initialized);
    assert!(emulator.init_state().is_ready());

    // Now refresh should work
    println!("\nDrawing test pattern...");
    Rectangle::new(Point::new(10, 10), Size::new(230, 102))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(&mut emulator)
        .unwrap();

    Text::new(
        "Initialization Complete!",
        Point::new(30, 60),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    println!("Refreshing display (this should work now)...");
    emulator.refresh_full().await.unwrap();
    println!("✓ Refresh successful!\n");

    // Save screenshot
    println!("Saving screenshot to 'initialization-complete.png'...");
    emulator.screenshot("initialization-complete.png").unwrap();

    println!("\n==============================================");
    println!("Summary:");
    println!("  - Initialization took: {}ms", elapsed.as_millis());
    println!("  - Expected time: ~1970ms");
    println!("  - Visual steps: 2 (checkerboard + clear)");
    println!("  - State: {:?}", emulator.init_state());
    println!("==============================================\n");

    println!("Close the window to exit.");
    emulator.run();
}
