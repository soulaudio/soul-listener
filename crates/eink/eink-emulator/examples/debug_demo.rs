//! Debug system demonstration
//!
//! Demonstrates the debug system features:
//! - Debug panel toggle (F12)
//! - Layout border visualization (F1)
//! - Inspector mode (F2)
//! - Power graph (F3)
//!
//! Run with: cargo run --example debug_demo --features debug

use eink_emulator::{DisplayDriver, Emulator};
use embedded_graphics::mono_font::{ascii::FONT_9X18_BOLD, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() {
    println!("E-Ink Emulator - Debug System Demo");
    println!("===================================\n");

    // Create emulator for larger display (480×800 pixels)
    println!("Creating emulator (480×800 pixels)...");
    let mut emulator = Emulator::new(480, 800);

    // Clear background
    emulator.framebuffer.clear();

    // Draw some test content
    println!("Drawing test content...");

    // Header rectangle
    Rectangle::new(Point::new(50, 50), Size::new(380, 100))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    Text::new(
        "Debug Demo",
        Point::new(150, 110),
        MonoTextStyle::new(&FONT_9X18_BOLD, Gray4::WHITE),
    )
    .draw(&mut emulator)
    .unwrap();

    // Content rectangle
    Rectangle::new(Point::new(100, 200), Size::new(280, 150))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    Text::new(
        "Test Content",
        Point::new(170, 280),
        MonoTextStyle::new(&FONT_9X18_BOLD, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Footer rectangle
    Rectangle::new(Point::new(150, 400), Size::new(180, 80))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)
        .unwrap();

    // Refresh display
    println!("Refreshing display...");
    emulator.refresh_full().await.unwrap();

    // Debug information
    #[cfg(feature = "debug")]
    {
        println!("\n🐛 Debug mode enabled!");
        println!("\nDebug Hotkeys:");
        println!("  F12 - Toggle debug panel");
        println!("  F1  - Toggle layout borders");
        println!("  F2  - Toggle inspector mode");
        println!("  F3  - Toggle power graph");

        if let Some(debug) = emulator.debug_manager() {
            println!("\nDebug State:");
            println!("  Panel visible: {}", debug.state().panel_visible);
            println!("  Borders enabled: {}", debug.state().borders_enabled);
            println!("  Inspector mode: {}", debug.state().inspector_mode);
            println!("  Power graph enabled: {}", debug.state().power_graph_enabled);
        }
    }

    #[cfg(not(feature = "debug"))]
    {
        println!("\n⚠️  Debug mode is NOT enabled.");
        println!("Run with --features debug to see debug features:");
        println!("  cargo run --example debug_demo --features debug");
    }

    println!("\nWindow opened!");
    println!("Close the window to exit.");

    // Run event loop (blocks until window closes)
    emulator.run();

    println!("\nDebug demo complete!");
}
