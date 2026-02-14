//! Display Spec Emulation Example
//!
//! Demonstrates emulator with real display specifications.
//! Shows multiple grayscale levels and various UI elements.
//!
//! Run with: cargo run --target x86_64-pc-windows-msvc --example spec_emulation

use eink_emulator::{DisplayDriver, Emulator};
use embedded_graphics::mono_font::{
    ascii::{FONT_5X8, FONT_6X10},
    MonoTextStyle,
};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{
    Circle, CornerRadii, PrimitiveStyle, Rectangle, RoundedRectangle,
};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() {
    println!("E-Ink Emulator - Display Spec Demo");
    println!("===================================\n");

    // Use Waveshare 2.13" V4 specs (250×122)
    println!("Creating emulator for Waveshare 2.13\" V4");
    println!("Resolution: 250×122 pixels");
    println!("Grayscale: 4 levels (0-3)\n");

    let mut emulator = Emulator::new(250, 122);

    // Clear to white background
    emulator.framebuffer.clear();

    // Draw title bar with black background
    Rectangle::new(Point::new(0, 0), Size::new(250, 25))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // White text on black background for visibility
    Text::new(
        "WAVESHARE 2.13\" V4",
        Point::new(35, 15),
        MonoTextStyle::new(&FONT_6X10, Gray4::WHITE),
    )
    .draw(&mut emulator)
    .unwrap();

    // Draw main border
    RoundedRectangle::new(
        Rectangle::new(Point::new(5, 30), Size::new(240, 87)),
        CornerRadii::new(Size::new(6, 6)),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
    .draw(&mut emulator)
    .unwrap();

    // Grayscale demonstration
    Text::new(
        "4-Level Grayscale:",
        Point::new(15, 45),
        MonoTextStyle::new(&FONT_5X8, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Draw 4 grayscale boxes
    for i in 0u8..4 {
        let x = 15 + (i as i32) * 35;
        Rectangle::new(Point::new(x, 55), Size::new(30, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(i)))
            .draw(&mut emulator)
            .unwrap();

        Rectangle::new(Point::new(x, 55), Size::new(30, 20))
            .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
            .draw(&mut emulator)
            .unwrap();

        // Label
        let label = format!("{}", i);
        Text::new(
            &label,
            Point::new(x + 12, 83),
            MonoTextStyle::new(&FONT_5X8, Gray4::BLACK),
        )
        .draw(&mut emulator)
        .unwrap();
    }

    // Draw some UI elements
    Text::new(
        "UI Elements:",
        Point::new(155, 45),
        MonoTextStyle::new(&FONT_5X8, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Button
    RoundedRectangle::new(
        Rectangle::new(Point::new(155, 55), Size::new(50, 18)),
        CornerRadii::new(Size::new(3, 3)),
    )
    .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
    .draw(&mut emulator)
    .unwrap();

    RoundedRectangle::new(
        Rectangle::new(Point::new(155, 55), Size::new(50, 18)),
        CornerRadii::new(Size::new(3, 3)),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
    .draw(&mut emulator)
    .unwrap();

    Text::new(
        "PLAY",
        Point::new(165, 67),
        MonoTextStyle::new(&FONT_5X8, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Icon (circle)
    Circle::new(Point::new(215, 56), 16)
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(&mut emulator)
        .unwrap();

    Circle::new(Point::new(219, 60), 8)
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    // Progress bar
    Text::new(
        "Progress:",
        Point::new(15, 98),
        MonoTextStyle::new(&FONT_5X8, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Progress bar background
    Rectangle::new(Point::new(70, 92), Size::new(160, 10))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
        .draw(&mut emulator)
        .unwrap();

    // Progress bar fill (60%)
    Rectangle::new(Point::new(71, 93), Size::new(95, 8))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    // Footer info
    Text::new(
        "250x122 | Carta 1000 | SSD1680",
        Point::new(10, 112),
        MonoTextStyle::new(&FONT_5X8, Gray4::new(1)),
    )
    .draw(&mut emulator)
    .unwrap();

    println!("Rendering to display...");
    emulator.refresh_full().await.unwrap();

    println!("Saving screenshot...");
    emulator.screenshot("spec-emulation.png").unwrap();
    println!("Screenshot saved to: spec-emulation.png\n");

    println!("✅ Window opened!");
    println!("   - Shows 4 grayscale levels (0=white, 1=light gray, 2=dark gray, 3=black)");
    println!("   - Demonstrates UI elements (button, icon, progress bar)");
    println!("   - 2x upscaling for better desktop visibility");
    println!("\nClose the window to exit.");

    emulator.run();

    println!("\n✅ Emulation complete!");
}
