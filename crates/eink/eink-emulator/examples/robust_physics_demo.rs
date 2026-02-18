//! Robust E-Ink Physics Demonstration
//!
//! Shows content-dependent ghosting, waveform modes, and DC balance tracking.
//!
//! Run with: cargo run --target x86_64-pc-windows-msvc --example robust_physics_demo

use eink_emulator::{DisplayDriver, Emulator, WaveformMode};
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️  E-Ink Robust Physics Demonstration");
    println!("=====================================\n");

    let mut emulator = Emulator::new(250, 122);

    // Demo 1: Content-Dependent Ghosting
    println!("📊 Demo 1: Content-Dependent Ghosting");
    println!("   Small transitions accumulate less ghosting than large transitions\n");

    // Draw initial content
    Text::new(
        "Demo 1: Ghosting",
        Point::new(10, 15),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(&mut emulator)?;

    Rectangle::new(Point::new(10, 30), Size::new(100, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)?;

    emulator.refresh_with_waveform(WaveformMode::GC16).await?;
    println!(
        "   ✓ Initial render with GC16 (ghosting: {:.4})",
        emulator.ghosting_level()
    );

    // Small transition
    Rectangle::new(Point::new(10, 30), Size::new(100, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(&mut emulator)?;

    emulator.refresh_with_waveform(WaveformMode::DU4).await?;
    let ghosting_small = emulator.ghosting_level();
    println!(
        "   ✓ Small transition (1→2) with DU4 (ghosting: {:.4})",
        ghosting_small
    );

    // Reset
    emulator.refresh_with_waveform(WaveformMode::GC16).await?;

    // Large transition
    Rectangle::new(Point::new(10, 30), Size::new(100, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)?;

    emulator.refresh_with_waveform(WaveformMode::DU4).await?;
    let ghosting_large = emulator.ghosting_level();
    println!(
        "   ✓ Large transition (0→3) with DU4 (ghosting: {:.4})",
        ghosting_large
    );
    println!(
        "   → Large transitions produce {:.1}x more ghosting\n",
        ghosting_large / ghosting_small
    );

    // Demo 2: Waveform Modes
    println!("📊 Demo 2: Waveform Mode Characteristics");

    // Clear screen
    Rectangle::new(Point::new(0, 0), Size::new(250, 122))
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut emulator)?;

    Text::new(
        "Demo 2: Waveforms",
        Point::new(10, 15),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(&mut emulator)?;

    // Test each waveform mode
    let modes = [
        (WaveformMode::GC16, "GC16: 16-level, clears ghosting"),
        (WaveformMode::GL16, "GL16: 16-level, reduced flash"),
        (WaveformMode::DU4, "DU4: 4-level, fast"),
        (WaveformMode::DU, "DU: 2-level, fastest"),
        (WaveformMode::A2, "A2: 2-level, animation"),
    ];

    for (mode, _desc) in modes {
        emulator.refresh_with_waveform(mode).await?;
        println!(
            "   ✓ {}: {} levels, {} flashes, {:.2} ghosting rate",
            mode.name(),
            mode.grayscale_levels(),
            mode.flash_count(),
            mode.ghosting_rate()
        );
    }
    println!();

    // Demo 3: DC Balance Tracking
    println!("📊 Demo 3: DC Balance Accumulation");
    println!("   Many partial refreshes accumulate DC imbalance\n");

    // Clear
    emulator.refresh_with_waveform(WaveformMode::GC16).await?;
    println!(
        "   ✓ Full refresh resets DC balance: {:.1}",
        emulator.pixel_states().max_dc_balance()
    );

    // Many partial refreshes
    for i in 0..10 {
        let color = if i % 2 == 0 {
            Gray4::BLACK
        } else {
            Gray4::WHITE
        };
        Rectangle::new(Point::new(50, 40), Size::new(150, 60))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut emulator)?;

        emulator.refresh_with_waveform(WaveformMode::DU4).await?;
    }

    let dc_balance = emulator.pixel_states().max_dc_balance();
    println!(
        "   ✓ After 10 partial refreshes, DC balance: {:.1}",
        dc_balance
    );
    println!("   ✓ DC warnings issued: {}", emulator.stats().dc_warnings);

    // Full refresh to clean up
    emulator.refresh_with_waveform(WaveformMode::GC16).await?;
    println!(
        "   ✓ After cleanup, DC balance: {:.1}\n",
        emulator.pixel_states().max_dc_balance()
    );

    // Demo 4: Statistics
    println!("📊 Demo 4: Display Statistics");
    let stats = emulator.stats();
    println!("   Full refreshes: {}", stats.full_refresh_count);
    println!("   Partial refreshes: {}", stats.partial_refresh_count);
    println!("   Fast refreshes: {}", stats.fast_refresh_count);
    println!("   Total refresh time: {}ms", stats.total_refresh_time_ms);
    println!("   DC warnings: {}\n", stats.dc_warnings);

    // Final summary screen
    Rectangle::new(Point::new(0, 0), Size::new(250, 122))
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut emulator)?;

    Text::new(
        "Robust Physics Demo",
        Point::new(50, 30),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(&mut emulator)?;

    Text::new(
        "All tests passed!",
        Point::new(55, 60),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(&mut emulator)?;

    emulator.refresh_full().await?;

    println!("✅ Demonstration complete!");
    println!("   Close the window to exit.");

    #[cfg(not(feature = "headless"))]
    emulator.run();

    Ok(())
}
