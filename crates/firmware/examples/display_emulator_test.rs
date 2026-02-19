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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .compact()
        .init();

    tracing::info!("Display Emulator Test");

    // Create emulator display (shows window)
    let mut display = EmulatorDisplay::new();

    // Test 1: Initialization
    tracing::info!(test = 1, "Display Initialization");
    display.init().await?;
    tracing::info!(test = 1, "Initialization successful");

    sleep(Duration::from_secs(1)).await;

    // Test 2: Splash screen
    tracing::info!(test = 2, "Splash Screen");
    SplashScreen::render(&mut display)?;
    display.refresh_full().await?;
    tracing::info!(test = 2, "Splash screen rendered");

    sleep(Duration::from_secs(2)).await;

    // Test 3: Test pattern
    tracing::info!(test = 3, "Test Pattern");
    TestPattern::render(&mut display)?;
    display.refresh_full().await?;
    tracing::info!(test = 3, "Test pattern rendered");

    sleep(Duration::from_secs(2)).await;

    // Test 4: Clear to white
    tracing::info!(test = 4, "Clear to White");
    DapDisplay::clear(&mut display, firmware::Color::White).await?;
    tracing::info!(test = 4, "Clear successful");

    sleep(Duration::from_secs(1)).await;

    // Test 5: Draw shapes
    tracing::info!(test = 5, "Drawing Shapes");

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
    tracing::info!(test = 5, "Shape drawing successful");

    sleep(Duration::from_secs(2)).await;

    // Test 6: Text rendering
    tracing::info!(test = 6, "Text Rendering");

    DapDisplay::clear(&mut display, firmware::Color::White).await?;

    let text_style = MonoTextStyle::new(&FONT_9X18, Gray4::BLACK);

    Text::new("SoulAudio DAP", Point::new(50, 100), text_style).draw(&mut display)?;

    Text::new("Emulator Test", Point::new(50, 130), text_style).draw(&mut display)?;

    Text::new("800x480 E-Ink Display", Point::new(50, 160), text_style).draw(&mut display)?;

    Text::new("Firmware v0.1.0", Point::new(50, 190), text_style).draw(&mut display)?;

    display.refresh_full().await?;
    tracing::info!(test = 6, "Text rendering successful");

    sleep(Duration::from_secs(2)).await;

    // Test 7: Partial refresh animation
    tracing::info!(test = 7, "Partial Refresh Animation");

    for i in 0..5 {
        DapDisplay::clear(&mut display, firmware::Color::White).await?;

        let x = 100 + (i * 120) as i32;
        Rectangle::new(Point::new(x, 200), Size::new(100, 80))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut display)?;

        let text = format!("Frame {}", i + 1);
        Text::new(&text, Point::new(x + 15, 245), text_style).draw(&mut display)?;

        display.refresh_partial().await?;
        tracing::debug!(frame = i + 1, "Frame rendered");

        sleep(Duration::from_millis(400)).await;
    }
    tracing::info!(test = 7, "Partial refresh animation successful");

    sleep(Duration::from_secs(1)).await;

    // Test 8: Sleep/wake
    tracing::info!(test = 8, "Sleep and Wake");
    display.sleep().await?;
    tracing::debug!("Display sleeping");

    sleep(Duration::from_secs(1)).await;

    display.wake().await?;
    tracing::info!(test = 8, "Wake successful");

    // Final screen
    DapDisplay::clear(&mut display, firmware::Color::White).await?;

    Text::new("All Tests Complete!", Point::new(250, 220), text_style).draw(&mut display)?;

    Text::new("Press Ctrl+C to exit", Point::new(220, 260), text_style).draw(&mut display)?;

    display.refresh_full().await?;

    tracing::info!("All tests complete");
    tracing::info!("Emulator window open — press Ctrl+C to exit");

    // Keep window open
    loop {
        sleep(Duration::from_secs(1)).await;
    }
}
