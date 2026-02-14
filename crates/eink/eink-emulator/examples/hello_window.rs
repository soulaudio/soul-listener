//! Hello Window Example
//!
//! Demonstrates the Phase 0 foundation:
//! - Window creation
//! - Framebuffer rendering
//! - embedded-graphics integration
//!
//! Run with: cargo run --example hello_window

use eink_emulator::{DisplayDriver, Emulator};
use embedded_graphics::mono_font::{
    ascii::{FONT_6X10, FONT_9X18_BOLD},
    MonoTextStyle,
};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{CornerRadii, PrimitiveStyle, Rectangle, RoundedRectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() {
    println!("E-Ink Emulator - Phase 0");
    println!("========================\n");

    // Create emulator for Waveshare 2.13" size (250×122 pixels)
    println!("Creating emulator (250×122 pixels)...");
    let mut emulator = Emulator::new(250, 122);

    // Clear background
    emulator.framebuffer.clear();

    // Draw border with rounded corners
    println!("Drawing UI elements...");
    RoundedRectangle::new(
        Rectangle::new(Point::new(5, 5), Size::new(240, 112)),
        CornerRadii::new(Size::new(8, 8)),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
    .draw(&mut emulator)
    .unwrap();

    // Draw title
    Text::new(
        "E-Ink Emulator",
        Point::new(40, 40),
        MonoTextStyle::new(&FONT_9X18_BOLD, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Draw subtitle
    Text::new(
        "Phase 0 Complete!",
        Point::new(60, 70),
        MonoTextStyle::new(&FONT_6X10, Gray4::new(2)),
    )
    .draw(&mut emulator)
    .unwrap();

    // Draw feature list
    Text::new(
        "✓ Window  ✓ Framebuffer  ✓ DrawTarget",
        Point::new(15, 95),
        MonoTextStyle::new(&FONT_6X10, Gray4::new(1)),
    )
    .draw(&mut emulator)
    .unwrap();

    // Refresh display
    println!("Refreshing display...");
    emulator.refresh_full().await.unwrap();

    // Save screenshot
    println!("Saving screenshot to 'phase0-screenshot.png'...");
    emulator.screenshot("phase0-screenshot.png").unwrap();

    println!("\nWindow opened!");
    println!("Close the window to exit.");

    // Run event loop (blocks until window closes)
    emulator.run();

    println!("\nPhase 0 demonstration complete!");
}
