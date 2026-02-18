//! Demonstration of rotation and scaling configuration
//!
//! Shows how to configure the emulator for different orientations and scales.

use eink_emulator::{DisplayDriver, Emulator, EmulatorConfig, Rotation};
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("E-Ink Emulator - Rotation & Scaling Demo");
    println!("=========================================\n");

    // Configuration 1: Portrait mode (90° rotation), no upscaling
    let config = EmulatorConfig {
        rotation: Rotation::Degrees90,
        scale: 1,
    };

    println!("Creating emulator with:");
    println!("  Rotation: 90° clockwise (portrait)");
    println!("  Scale: 1x (native resolution)");
    println!("  Logical display: 800×480");
    println!("  Window: 480×800 (rotated)\n");

    let mut emulator = Emulator::with_config(config);

    // Draw content in LOGICAL coordinates (always 800×480, regardless of rotation)
    draw_demo_content(&mut emulator)?;

    // Perform a full refresh
    emulator.refresh_full().await?;

    println!("Window displayed. The rotation is applied to the presentation only.");
    println!("DrawTarget coordinates remain logical (800×480).\n");

    println!("Try these other configurations:");
    println!("  EmulatorConfig {{ rotation: Rotation::Degrees0, scale: 2 }} - Landscape 2x");
    println!("  EmulatorConfig {{ rotation: Rotation::Degrees180, scale: 1 }} - Upside-down");
    println!("  EmulatorConfig {{ rotation: Rotation::Degrees270, scale: 2 }} - Portrait 2x");
    println!("\nClose the window to exit.");

    // Run the event loop (blocks until window closed)
    emulator.run();

    Ok(())
}

/// Draw demonstration content (in logical coordinates)
fn draw_demo_content(emulator: &mut Emulator) -> Result<(), Box<dyn std::error::Error>> {
    // Clear to white
    let size = emulator.bounding_box().size;
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(emulator)?;

    // Draw title (top-left corner in logical space)
    let text_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    Text::new("Rotation Demo", Point::new(10, 20), text_style).draw(emulator)?;

    // Draw orientation markers
    Text::new("TOP", Point::new(350, 30), text_style).draw(emulator)?;

    Text::new("LEFT", Point::new(10, 240), text_style).draw(emulator)?;

    Text::new("RIGHT", Point::new(720, 240), text_style).draw(emulator)?;

    Text::new("BOTTOM", Point::new(340, 470), text_style).draw(emulator)?;

    // Draw a circle in the center
    Circle::new(Point::new(350, 190), 100)
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 3))
        .draw(emulator)?;

    // Draw corner markers
    let corner_size = 30;
    // Top-left
    Rectangle::new(Point::zero(), Size::new(corner_size, corner_size))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(emulator)?;

    // Top-right
    Rectangle::new(
        Point::new(800 - corner_size as i32, 0),
        Size::new(corner_size, corner_size),
    )
    .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x8)))
    .draw(emulator)?;

    // Bottom-left
    Rectangle::new(
        Point::new(0, 480 - corner_size as i32),
        Size::new(corner_size, corner_size),
    )
    .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xC)))
    .draw(emulator)?;

    // Bottom-right
    Rectangle::new(
        Point::new(800 - corner_size as i32, 480 - corner_size as i32),
        Size::new(corner_size, corner_size),
    )
    .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
    .draw(emulator)?;

    Ok(())
}
