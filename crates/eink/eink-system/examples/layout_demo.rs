//! Layout System Demo
//!
//! Demonstrates the flexbox-inspired layout engine for e-ink displays.

use eink_emulator::{DisplayDriver, Emulator, EmulatorConfig, Rotation};
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("E-Ink Layout System Demo");
    println!("========================\n");

    // Configure emulator: portrait mode, 2x scaling for visibility
    let config = EmulatorConfig {
        rotation: Rotation::Degrees90,
        scale: 2,
    };

    let mut emulator = Emulator::with_config(config);

    println!("Creating DAP-style layout...");

    // Clear to white
    let size = emulator.bounding_box().size;
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut emulator)?;

    // Demo 1: Simple VStack (vertical stack)
    demo_vstack(&mut emulator)?;

    emulator.refresh_full().await?;
    println!("✓ Layout rendered\n");

    println!("Window shows:");
    println!("  - Header (black bar)");
    println!("  - Content area (three gray boxes)");
    println!("  - Footer (dark gray bar)");
    println!("\nClose window to exit.");

    emulator.run();
    Ok(())
}

fn demo_vstack(emulator: &mut Emulator) -> Result<(), Box<dyn std::error::Error>> {
    // Header: 60px tall
    draw_box(emulator, Point::new(0, 0), Size::new(480, 60), Gray4::BLACK)?;
    Text::new(
        "Now Playing",
        Point::new(20, 40),
        MonoTextStyle::new(&FONT_10X20, Gray4::WHITE),
    )
    .draw(emulator)?;

    // Content area: 3 items with gaps
    let content_y: i32 = 60;
    let item_height: i32 = 100;
    let gap: i32 = 16;

    // Item 1: Artist
    draw_box(
        emulator,
        Point::new(16, content_y + gap),
        Size::new(448, item_height as u32),
        Gray4::new(0xC),
    )?;
    Text::new(
        "Artist Name",
        Point::new(32, content_y + gap + 40),
        MonoTextStyle::new(&FONT_10X20, Gray4::BLACK),
    )
    .draw(emulator)?;

    // Item 2: Album
    draw_box(
        emulator,
        Point::new(16, content_y + gap + item_height + gap),
        Size::new(448, item_height as u32),
        Gray4::new(0xA),
    )?;
    Text::new(
        "Album Title",
        Point::new(32, content_y + gap + item_height + gap + 40),
        MonoTextStyle::new(&FONT_10X20, Gray4::BLACK),
    )
    .draw(emulator)?;

    // Item 3: Progress
    draw_box(
        emulator,
        Point::new(16, content_y + gap + (item_height + gap) * 2),
        Size::new(448, item_height as u32),
        Gray4::new(0x8),
    )?;
    Text::new(
        "3:45 / 5:23",
        Point::new(32, content_y + gap + (item_height + gap) * 2 + 40),
        MonoTextStyle::new(&FONT_10X20, Gray4::BLACK),
    )
    .draw(emulator)?;

    // Footer: 80px tall
    let footer_y = 700;
    draw_box(
        emulator,
        Point::new(0, footer_y),
        Size::new(480, 100),
        Gray4::new(0x4),
    )?;
    Text::new(
        "Controls",
        Point::new(20, footer_y + 40),
        MonoTextStyle::new(&FONT_10X20, Gray4::WHITE),
    )
    .draw(emulator)?;

    Ok(())
}

fn draw_box(
    emulator: &mut Emulator,
    position: Point,
    size: Size,
    color: Gray4,
) -> Result<(), Box<dyn std::error::Error>> {
    Rectangle::new(position, size)
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(emulator)?;
    Ok(())
}
