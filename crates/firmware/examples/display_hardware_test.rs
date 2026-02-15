//! Display Hardware Test
//!
//! Comprehensive test of the SSD1677 display driver on real hardware.
//! Tests initialization, drawing, and refresh operations.

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_time::{Duration, Timer};
use embedded_graphics::mono_font::{ascii::FONT_9X18, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use static_cell::StaticCell;

use firmware::{DapDisplay, Ssd1677Display, DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE};

use panic_probe as _;

#[link_section = ".axisram"]
static mut FRAMEBUFFER: [u8; FRAMEBUFFER_SIZE] = [0xFF; FRAMEBUFFER_SIZE];

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    defmt::info!("=== SSD1677 Display Hardware Test ===");

    let p = embassy_stm32::init(Default::default());

    // Setup SPI and GPIO (same as main.rs)
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(4_000_000);

    let spi = Spi::new(
        p.SPI1, p.PA5, p.PA7, p.PA6, p.DMA1_CH0, p.DMA1_CH1, spi_config,
    );

    let dc = Output::new(p.PB0, Level::Low, Speed::VeryHigh);
    let cs = Output::new(p.PB1, Level::High, Speed::VeryHigh);
    let rst = Output::new(p.PB2, Level::High, Speed::VeryHigh);
    let busy = Input::new(p.PB3, Pull::None);

    let mut display = Ssd1677Display::new(spi, dc, cs, rst, busy);

    // Test 1: Initialization
    defmt::info!("Test 1: Display Initialization");
    match display.init().await {
        Ok(_) => defmt::info!("✓ Initialization successful"),
        Err(e) => {
            defmt::error!("✗ Initialization failed: {:?}", e);
            panic!("Cannot continue without display");
        }
    }

    Timer::after(Duration::from_secs(1)).await;

    // Test 2: Clear to white (full refresh)
    defmt::info!("Test 2: Clear to White");
    match display.clear(firmware::Color::White).await {
        Ok(_) => defmt::info!("✓ Clear to white successful"),
        Err(e) => defmt::error!("✗ Clear failed: {:?}", e),
    }

    Timer::after(Duration::from_secs(2)).await;

    // Test 3: Draw rectangles
    defmt::info!("Test 3: Drawing Rectangles");

    // Black rectangle
    Rectangle::new(Point::new(50, 50), Size::new(200, 100))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
        .draw(&mut display)
        .ok();

    // White rectangle inside black
    Rectangle::new(Point::new(70, 70), Size::new(160, 60))
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(&mut display)
        .ok();

    match display.refresh_full().await {
        Ok(_) => defmt::info!("✓ Rectangle drawing successful"),
        Err(e) => defmt::error!("✗ Drawing failed: {:?}", e),
    }

    Timer::after(Duration::from_secs(2)).await;

    // Test 4: Draw circles
    defmt::info!("Test 4: Drawing Circles");

    // Clear
    display.clear(firmware::Color::White).await.ok();

    // Draw concentric circles
    for i in 0..5 {
        let radius = 20 + i * 15;
        Circle::new(
            Point::new(400 - radius as i32, 240 - radius as i32),
            radius * 2,
        )
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::Off, 2))
        .draw(&mut display)
        .ok();
    }

    match display.refresh_full().await {
        Ok(_) => defmt::info!("✓ Circle drawing successful"),
        Err(e) => defmt::error!("✗ Drawing failed: {:?}", e),
    }

    Timer::after(Duration::from_secs(2)).await;

    // Test 5: Text rendering
    defmt::info!("Test 5: Text Rendering");

    display.clear(firmware::Color::White).await.ok();

    let text_style = MonoTextStyle::new(&FONT_9X18, BinaryColor::Off);

    Text::new("SSD1677 Test", Point::new(50, 100), text_style)
        .draw(&mut display)
        .ok();

    Text::new("800x480 E-Ink Display", Point::new(50, 130), text_style)
        .draw(&mut display)
        .ok();

    Text::new("Full Refresh", Point::new(50, 160), text_style)
        .draw(&mut display)
        .ok();

    match display.refresh_full().await {
        Ok(_) => defmt::info!("✓ Text rendering successful"),
        Err(e) => defmt::error!("✗ Text rendering failed: {:?}", e),
    }

    Timer::after(Duration::from_secs(2)).await;

    // Test 6: Partial refresh
    defmt::info!("Test 6: Partial Refresh Test");

    for i in 0..3 {
        // Draw a moving rectangle
        display.clear(firmware::Color::White).await.ok();

        let x = 100 + (i * 150) as i32;
        Rectangle::new(Point::new(x, 200), Size::new(100, 80))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
            .draw(&mut display)
            .ok();

        Text::new("Partial", Point::new(x + 10, 240), text_style)
            .draw(&mut display)
            .ok();

        match display.refresh_partial().await {
            Ok(_) => defmt::info!("✓ Partial refresh {} successful", i + 1),
            Err(e) => defmt::error!("✗ Partial refresh failed: {:?}", e),
        }

        Timer::after(Duration::from_millis(500)).await;
    }

    Timer::after(Duration::from_secs(2)).await;

    // Test 7: Sleep mode
    defmt::info!("Test 7: Sleep Mode");

    match display.sleep().await {
        Ok(_) => defmt::info!("✓ Entered sleep mode"),
        Err(e) => defmt::error!("✗ Sleep mode failed: {:?}", e),
    }

    Timer::after(Duration::from_secs(2)).await;

    // Test 8: Wake from sleep
    defmt::info!("Test 8: Wake from Sleep");

    match display.wake().await {
        Ok(_) => defmt::info!("✓ Woke from sleep"),
        Err(e) => defmt::error!("✗ Wake failed: {:?}", e),
    }

    // Final message
    display.clear(firmware::Color::White).await.ok();
    Text::new("All Tests Complete!", Point::new(250, 230), text_style)
        .draw(&mut display)
        .ok();
    display.refresh_full().await.ok();

    defmt::info!("=== Test Complete ===");

    // Heartbeat loop
    let mut counter = 0u32;
    loop {
        Timer::after(Duration::from_secs(1)).await;
        counter += 1;
        defmt::trace!("Heartbeat: {}", counter);
    }
}
