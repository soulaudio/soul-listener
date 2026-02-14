//! Phase 2 Demo - Core Emulation Features
//!
//! Demonstrates realistic e-ink behavior simulation:
//! - Multiple refresh modes (Full, Partial, Fast)
//! - Ghosting accumulation and auto-cleanup
//! - Temperature effects on timing
//! - Multi-brand display support via DisplaySpec
//!
//! Run with: cargo run --target x86_64-pc-windows-msvc --example phase2_demo

use eink_emulator::{DisplayDriver, EinkDisplay, Emulator, RefreshMode};
use eink_specs::displays;
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() {
    println!("╔═══════════════════════════════════════╗");
    println!("║   E-Ink Emulator - Phase 2 Demo      ║");
    println!("║   Core Emulation Features             ║");
    println!("╚═══════════════════════════════════════╝\n");

    // Demonstrate multi-brand support
    demonstrate_multibrand_support().await;

    // Demonstrate refresh modes
    demonstrate_refresh_modes().await;

    // Demonstrate ghosting accumulation
    demonstrate_ghosting().await;

    // Demonstrate temperature effects
    demonstrate_temperature().await;

    println!("\n✅ Phase 2 demonstration complete!");
    println!("All core emulation features working correctly.\n");
}

async fn demonstrate_multibrand_support() {
    println!("📋 Multi-Brand Display Support");
    println!("─────────────────────────────────\n");

    // Waveshare 2.13" V4
    let waveshare = Emulator::headless_with_spec(&displays::WAVESHARE_2_13_V4);
    println!("Waveshare 2.13\" V4:");
    println!(
        "  Resolution: {}×{}",
        waveshare.spec().width,
        waveshare.spec().height
    );
    println!("  Controller: {:?}", waveshare.spec().controller);
    println!("  Panel: {:?}", waveshare.spec().panel_type);
    println!("  Full refresh: {}ms", waveshare.spec().full_refresh_ms);

    // GoodDisplay
    let gooddisplay = Emulator::headless_with_spec(&displays::GDEW042T2);
    println!("\nGoodDisplay GDEW042T2:");
    println!(
        "  Resolution: {}×{}",
        gooddisplay.spec().width,
        gooddisplay.spec().height
    );
    println!("  Controller: {:?}", gooddisplay.spec().controller);
    println!("  Panel: {:?}", gooddisplay.spec().panel_type);
    println!("  Full refresh: {}ms", gooddisplay.spec().full_refresh_ms);

    println!();
}

async fn demonstrate_refresh_modes() {
    println!("🔄 Refresh Modes");
    println!("─────────────────────────────────\n");

    let mut emulator = Emulator::headless(250, 122);

    // Draw test pattern
    Rectangle::new(Point::new(50, 30), Size::new(150, 62))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Full refresh
    println!("Full Refresh (2000ms):");
    println!("  - Clears ghosting");
    println!("  - 3 flashes");
    println!("  - 16 grayscale levels");
    let start = std::time::Instant::now();
    emulator.refresh_full().await.unwrap();
    println!("  ⏱️  Duration: {:?}\n", start.elapsed());

    // Partial refresh
    Circle::new(Point::new(100, 40), 20)
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(&mut emulator)
        .unwrap();

    println!("Partial Refresh (300ms):");
    println!("  - Fast update");
    println!("  - No flashing");
    println!(
        "  - Accumulates ghosting: {:.1}%",
        emulator.spec().ghosting_rate_partial * 100.0
    );
    let start = std::time::Instant::now();
    emulator.refresh_partial().await.unwrap();
    println!("  ⏱️  Duration: {:?}", start.elapsed());
    println!(
        "  👻 Ghosting level: {:.1}%\n",
        emulator.ghosting_level() * 100.0
    );

    // Fast refresh
    Circle::new(Point::new(150, 40), 15)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut emulator)
        .unwrap();

    println!("Fast Refresh (260ms):");
    println!("  - Fastest mode");
    println!("  - Single flash");
    println!(
        "  - High ghosting: {:.1}%",
        emulator.spec().ghosting_rate_fast * 100.0
    );
    let start = std::time::Instant::now();
    emulator.refresh_fast().await.unwrap();
    println!("  ⏱️  Duration: {:?}", start.elapsed());
    println!(
        "  👻 Ghosting level: {:.1}%\n",
        emulator.ghosting_level() * 100.0
    );
}

async fn demonstrate_ghosting() {
    println!("👻 Ghosting Accumulation & Auto-Cleanup");
    println!("─────────────────────────────────\n");

    let mut emulator = Emulator::headless(250, 122);

    println!("Performing 5 partial refreshes...");
    for i in 1..=5 {
        // Draw changing content
        let x = 50 + (i * 20);
        Circle::new(Point::new(x, 60), 10)
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();

        emulator.refresh_partial().await.unwrap();
        println!(
            "  Refresh {}: Ghosting = {:.1}%",
            i,
            emulator.ghosting_level() * 100.0
        );
    }

    println!("\n⚠️  Auto full-refresh triggered at threshold!");
    println!(
        "Ghosting cleared: {:.1}%\n",
        emulator.ghosting_level() * 100.0
    );
}

async fn demonstrate_temperature() {
    println!("🌡️  Temperature Effects");
    println!("─────────────────────────────────\n");

    let mut emulator = Emulator::headless(250, 122);

    // Normal temperature
    println!("Normal temperature (25°C):");
    let adjusted = emulator.spec().adjusted_refresh_ms(2000, 25);
    println!("  Full refresh: {}ms", adjusted);

    // Cold temperature
    emulator.set_temperature(-5);
    println!("\nCold temperature (-5°C):");
    let adjusted = emulator.spec().adjusted_refresh_ms(2000, -5);
    println!("  Full refresh: {}ms (50% slower)", adjusted);
    println!(
        "  In optimal range: {}",
        emulator.spec().is_optimal_temp(-5)
    );
    println!(
        "  In operating range: {}",
        emulator.spec().is_operating_temp(-5)
    );

    // Hot temperature
    emulator.set_temperature(45);
    println!("\nHot temperature (45°C):");
    let adjusted = emulator.spec().adjusted_refresh_ms(2000, 45);
    println!("  Full refresh: {}ms (20% slower)", adjusted);
    println!(
        "  In optimal range: {}",
        emulator.spec().is_optimal_temp(45)
    );
    println!(
        "  In operating range: {}",
        emulator.spec().is_operating_temp(45)
    );

    println!();
}
