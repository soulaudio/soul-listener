//! DAP Display Emulator
//!
//! Blank canvas for developing the DAP UI.
//! Run with: cargo run --example display_emulator --features emulator

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

use firmware::EmulatorDisplay;
use platform::config;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{} - Display Emulator", config::APP_NAME);
    println!("Display: 7.5\" E-Ink (800×480 → 480×800 portrait)\n");

    // Create runtime for async operations
    let rt = tokio::runtime::Runtime::new()?;

    // Configure emulator for portrait mode, native resolution
    // Using 7.5" display (800×480) rotated 90° = 480×800 portrait
    let emulator_config = eink_emulator::EmulatorConfig {
        rotation: eink_emulator::Rotation::Degrees90,  // Portrait orientation
        scale: 1,                                       // Native resolution
    };

    // Create emulator display with 7.5" spec (opens window in portrait mode)
    let spec = &eink_specs::displays::WAVESHARE_7_5_V2;
    let mut display = EmulatorDisplay::with_spec_and_config(spec, emulator_config);
    println!("Window opened - Portrait mode (480×800), native resolution\n");

    // Initialize with blank white screen
    println!("Initializing display...");

    // Clear to white
    let size = display.size();
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut display)?;

    use platform::DisplayDriver;
    rt.block_on(async {
        display.refresh_full().await
    })?;

    println!("✓ Display ready\n");

    // Render a demo UI (menu scene)
    println!("Rendering demo menu...");
    render_demo_menu(&mut display)?;

    rt.block_on(async {
        display.refresh_full().await
    })?;

    println!("✓ Demo menu rendered\n");
    println!("Close the window to exit.\n");

    // TODO: Replace render_demo_menu() with your own UI code
    // Example:
    // use embedded_graphics::text::Text;
    // use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
    //
    // Rectangle::new(Point::zero(), size)
    //     .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
    //     .draw(&mut display)?;
    //
    // Text::new("Hello DAP!", Point::new(100, 100), MonoTextStyle::new(&FONT_10X20, Gray4::BLACK))
    //     .draw(&mut display)?;
    //
    // rt.block_on(async { display.refresh_full().await })?;

    // Run event loop (blocks until window closed, keeps window responsive)
    display.into_inner().run();

    Ok(())
}

/// Render a simple demo menu (replace this with your own UI)
fn render_demo_menu<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray4>,
{
    use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
    use embedded_graphics::text::Text;

    let size = display.bounding_box().size;

    // Header background
    Rectangle::new(Point::zero(), Size::new(size.width, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    // Header text
    let header_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("Main Menu", Point::new(20, 35), header_style)
        .draw(display)?;

    // Menu items
    let menu_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    let menu_items = [
        "1. Now Playing",
        "2. Library",
        "3. Playlists",
        "4. Settings",
        "5. About",
    ];

    for (idx, item) in menu_items.iter().enumerate() {
        let y = 100 + (idx as i32 * 50);

        // Item background (alternate shading)
        if idx % 2 == 0 {
            Rectangle::new(
                Point::new(10, y - 5),
                Size::new(size.width - 20, 45),
            )
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xE)))
            .draw(display)?;
        }

        Text::new(item, Point::new(30, y + 20), menu_style)
            .draw(display)?;
    }

    // Footer text
    let footer_style = MonoTextStyle::new(&FONT_10X20, Gray4::new(0x8));
    Text::new(
        "Replace this with your UI in display_emulator.rs",
        Point::new(30, (size.height - 30) as i32),
        footer_style,
    )
    .draw(display)?;

    Ok(())
}
