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
    println!("Display: GDEM0397T81P (800×480)\n");

    // Create runtime for async operations
    let rt = tokio::runtime::Runtime::new()?;

    // Configure emulator for portrait mode, no upscaling (as preferred for DAP)
    let emulator_config = eink_emulator::EmulatorConfig {
        rotation: eink_emulator::Rotation::Degrees90,  // Portrait orientation
        scale: 1,                                       // Native resolution (no upscaling)
    };

    // Create emulator display (opens window in portrait mode)
    let mut display = EmulatorDisplay::with_config(emulator_config);
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

    // TODO: Your UI code goes here
    // Example:
    // use embedded_graphics::text::Text;
    // use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
    //
    // Text::new("Hello DAP!", Point::new(100, 100), MonoTextStyle::new(&FONT_10X20, Gray4::BLACK))
    //     .draw(&mut display)?;
    //
    // rt.block_on(async { display.refresh_full().await })?;

    println!("Close the window to exit.\n");

    // Run event loop (blocks until window closed, keeps window responsive)
    display.into_inner().run();

    Ok(())
}
