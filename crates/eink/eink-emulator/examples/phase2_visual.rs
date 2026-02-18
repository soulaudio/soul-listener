//! Phase 2 Visual Demo - Watch E-Ink Behavior in Real-Time!
//!
//! Opens windows showing realistic e-ink refresh animations:
//! - Full refresh with flashing
//! - Partial refresh with ghosting
//! - Fast refresh
//! - Temperature effects
//!
//! Run with: cargo run --target x86_64-pc-windows-msvc --example phase2_visual

use eink_emulator::{DisplayDriver, Emulator};
use embedded_graphics::mono_font::{
    ascii::{FONT_6X10, FONT_9X18_BOLD},
    MonoTextStyle,
};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() {
    println!("╔═══════════════════════════════════════════════╗");
    println!("║   E-Ink Emulator - Phase 2 Visual Demo       ║");
    println!("║   Watch Realistic E-Ink Behavior!            ║");
    println!("╚═══════════════════════════════════════════════╝\n");

    println!("This demo will show you:");
    println!("  1. Full refresh with flash animation");
    println!("  2. Partial refresh (fast, no flashing)");
    println!("  3. Ghosting accumulation");
    println!("  4. Auto full-refresh cleanup\n");

    // Create emulator with window
    let mut emulator = Emulator::new(250, 122);

    // Draw initial content
    println!("1️⃣  Drawing initial screen...");
    draw_title(&mut emulator, "FULL REFRESH");
    draw_progress_bar(&mut emulator, 0);

    println!("2️⃣  Performing FULL REFRESH...");
    println!("    Watch for 3 flashes (black → white → final)");
    emulator.refresh_full().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Partial refreshes with ghosting accumulation
    println!("\n3️⃣  Performing 5 PARTIAL REFRESHES...");
    println!("    Watch ghosting accumulate!");

    for i in 1..=5 {
        draw_title(&mut emulator, "PARTIAL REFRESH");
        draw_progress_bar(&mut emulator, i * 20);

        Text::new(
            &format!("Refresh {}/5", i),
            Point::new(80, 90),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(&mut emulator)
        .unwrap();

        Text::new(
            &format!("Ghosting: {:.0}%", emulator.ghosting_level() * 100.0),
            Point::new(65, 105),
            MonoTextStyle::new(&FONT_6X10, Gray4::new(1)),
        )
        .draw(&mut emulator)
        .unwrap();

        println!(
            "    Refresh {}/5 - Ghosting: {:.1}%",
            i,
            emulator.ghosting_level() * 100.0
        );

        if i == 5 {
            println!("\n⚠️  THRESHOLD REACHED!");
            println!("    Auto full-refresh will trigger...");
        }

        emulator.refresh_partial().await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    println!(
        "\n4️⃣  Ghosting cleared: {:.1}%",
        emulator.ghosting_level() * 100.0
    );

    // Final screen
    draw_title(&mut emulator, "DEMO COMPLETE");

    Rectangle::new(Point::new(40, 50), Size::new(170, 50))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(&mut emulator)
        .unwrap();

    Text::new(
        "Phase 2 Complete!",
        Point::new(55, 75),
        MonoTextStyle::new(&FONT_9X18_BOLD, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    emulator.refresh_full().await.unwrap();

    println!("\n✅ Phase 2 demonstration complete!");
    println!("\nClose the window to exit.");

    // Keep window open
    emulator.run();
}

fn draw_title(emulator: &mut Emulator, title: &str) {
    // Clear title area
    Rectangle::new(Point::new(0, 0), Size::new(250, 30))
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(emulator)
        .unwrap();

    // Black title bar
    Rectangle::new(Point::new(0, 0), Size::new(250, 25))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(emulator)
        .unwrap();

    // White title text
    let text_x = (250 - (title.len() as i32 * 6)) / 2;
    Text::new(
        title,
        Point::new(text_x, 15),
        MonoTextStyle::new(&FONT_6X10, Gray4::WHITE),
    )
    .draw(emulator)
    .unwrap();
}

fn draw_progress_bar(emulator: &mut Emulator, percent: i32) {
    // Clear content area
    Rectangle::new(Point::new(0, 30), Size::new(250, 92))
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(emulator)
        .unwrap();

    // Progress bar background
    Rectangle::new(Point::new(30, 50), Size::new(190, 20))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(emulator)
        .unwrap();

    // Progress bar fill
    if percent > 0 {
        let width = (190 * percent / 100) as u32;
        Rectangle::new(Point::new(32, 52), Size::new(width.max(1), 16))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
            .draw(emulator)
            .unwrap();
    }

    // Percentage text
    let text = format!("{}%", percent);
    Text::new(
        &text,
        Point::new(110, 68),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(emulator)
    .unwrap();
}
