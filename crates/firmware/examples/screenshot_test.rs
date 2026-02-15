//! Screenshot test - captures current UI state

use eink_emulator::{Emulator, EmulatorConfig, Rotation, WaveformMode};
use eink_specs::displays::WAVESHARE_7_5_V2;
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Taking screenshot of current display configuration...\n");

    // Create emulator with 7.5" display, rotated for portrait
    let rotation = Rotation::Degrees90;
    let config = EmulatorConfig { rotation, scale: 1 };
    let mut emulator = Emulator::with_spec_and_config(&WAVESHARE_7_5_V2, config);

    // Render test content
    let size = emulator.bounding_box().size;
    println!(
        "Display size after rotation: {}×{}",
        size.width, size.height
    );

    // Clear to white
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut emulator)?;

    // Draw header
    Rectangle::new(Point::zero(), Size::new(size.width, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(&mut emulator)?;

    let header_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("TEST - Portrait Mode", Point::new(20, 35), header_style).draw(&mut emulator)?;

    // Draw some test content
    let text_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    Text::new(
        &format!("Display: {}x{}", size.width, size.height),
        Point::new(20, 100),
        text_style,
    )
    .draw(&mut emulator)?;
    Text::new(
        "This should be PORTRAIT (tall)",
        Point::new(20, 130),
        text_style,
    )
    .draw(&mut emulator)?;
    Text::new(
        "Width should be LESS than height",
        Point::new(20, 160),
        text_style,
    )
    .draw(&mut emulator)?;

    // Refresh to display
    emulator.refresh_with_waveform(WaveformMode::GC16).await?;

    // Save screenshot
    let screenshot_path = "screenshot_current.png";
    emulator.screenshot(screenshot_path)?;

    println!("\n✓ Screenshot saved to: {}", screenshot_path);
    println!("  Size: {}×{} pixels", size.width, size.height);
    println!("  Expected: 480×800 (portrait)");
    println!("  Rotation: {:?}", rotation);

    Ok(())
}
