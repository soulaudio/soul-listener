//! Display Emulator Test
//!
//! Desktop test of the display driver using the eink-emulator.
//! Run with: cargo run --example display_emulator_test --features emulator

use embedded_graphics::mono_font::{ascii::FONT_9X18, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use tokio::time::{sleep, Duration};

use firmware::ui::{SplashScreen, TestPattern};
use firmware::{DapDisplay, EmulatorDisplay};
use platform::DisplayDriver;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Display Emulator Test ===\n");

    // Create emulator display (shows window)
    let mut display = EmulatorDisplay::new();

    // Test 1: Initialization
    println!("Test 1: Display Initialization");
    display.init().await?;
    println!("✓ Initialization successful\n");

    sleep(Duration::from_secs(1)).await;

    // Test 2: Splash screen
    println!("Test 2: Splash Screen");
    SplashScreen::render(&mut display)?;
    display.refresh_full().await?;
    println!("✓ Splash screen rendered\n");

    sleep(Duration::from_secs(2)).await;

    // Test 3: Test pattern
    println!("Test 3: Test Pattern");
    TestPattern::render(&mut display)?;
    display.refresh_full().await?;
    println!("✓ Test pattern rendered\n");

    sleep(Duration::from_secs(2)).await;

    // Test 4: Clear to white
    println!("Test 4: Clear to White");
    DapDisplay::clear(&mut display, firmware::Color::White).await?;
    println!("✓ Clear successful\n");

    sleep(Duration::from_secs(1)).await;

    // Test 5: Draw shapes
    println!("Test 5: Drawing Shapes");

    // Black rectangles
    Rectangle::new(Point::new(50, 50), Size::new(200, 100))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut display)?;

    Rectangle::new(Point::new(300, 50), Size::new(200, 100))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 3))
        .draw(&mut display)?;

    // Circles
    Circle::new(Point::new(100, 200), 80)
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut display)?;

    Circle::new(Point::new(350, 200), 80)
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 3))
        .draw(&mut display)?;

    display.refresh_full().await?;
    println!("✓ Shape drawing successful\n");

    sleep(Duration::from_secs(2)).await;

    // Test 6: Text rendering
    println!("Test 6: Text Rendering");

    DapDisplay::clear(&mut display, firmware::Color::White).await?;

    let text_style = MonoTextStyle::new(&FONT_9X18, Gray4::BLACK);

    Text::new("SoulAudio DAP", Point::new(50, 100), text_style).draw(&mut display)?;

    Text::new("Emulator Test", Point::new(50, 130), text_style).draw(&mut display)?;

    Text::new("800x480 E-Ink Display", Point::new(50, 160), text_style).draw(&mut display)?;

    Text::new("Firmware v0.1.0", Point::new(50, 190), text_style).draw(&mut display)?;

    display.refresh_full().await?;
    println!("✓ Text rendering successful\n");

    sleep(Duration::from_secs(2)).await;

    // Test 7: Partial refresh animation
    println!("Test 7: Partial Refresh Animation");

    for i in 0..5 {
        DapDisplay::clear(&mut display, firmware::Color::White).await?;

        let x = 100 + (i * 120) as i32;
        Rectangle::new(Point::new(x, 200), Size::new(100, 80))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut display)?;

        let text = format!("Frame {}", i + 1);
        Text::new(&text, Point::new(x + 15, 245), text_style).draw(&mut display)?;

        display.refresh_partial().await?;
        println!("  Frame {} rendered", i + 1);

        sleep(Duration::from_millis(400)).await;
    }
    println!("✓ Partial refresh animation successful\n");

    sleep(Duration::from_secs(1)).await;

    // Test 8: Sleep/wake
    println!("Test 8: Sleep and Wake");
    display.sleep().await?;
    println!("  Display sleeping...");

    sleep(Duration::from_secs(1)).await;

    display.wake().await?;
    println!("✓ Wake successful\n");

    // Final screen
    DapDisplay::clear(&mut display, firmware::Color::White).await?;

    Text::new("All Tests Complete!", Point::new(250, 220), text_style).draw(&mut display)?;

    Text::new("Press Ctrl+C to exit", Point::new(220, 260), text_style).draw(&mut display)?;

    display.refresh_full().await?;

    println!("=== All Tests Complete ===\n");
    println!("Emulator window is open. Press Ctrl+C to exit.");

    // Keep window open
    loop {
        sleep(Duration::from_secs(1)).await;
    }
}
