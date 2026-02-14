use eink_emulator::{DisplayDriver, Emulator};
use eink_specs::displays::gooddisplay::GDEM0397T81P;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use platform::config;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("{} - Display Demo", config::APP_NAME);
    println!("Display: Good Display GDEM0397T81P (3.97\", 800×480)");
    println!();

    // Create emulator with the exact DAP display spec
    let mut emulator = Emulator::with_spec(&GDEM0397T81P);

    // Initialize display (simulates hardware init sequence)
    println!("Initializing display...");
    emulator.initialize().await.unwrap();
    println!("✓ Display initialized");

    // Show display info
    println!();
    println!("Display Specifications:");
    println!("  Resolution: {}×{}", GDEM0397T81P.width, GDEM0397T81P.height);
    println!("  Controller: SSD1677");
    println!("  Panel: Carta 1200");
    println!("  Grayscale: {} levels", GDEM0397T81P.grayscale_levels);
    println!("  Full Refresh: {}ms", GDEM0397T81P.full_refresh_ms);
    println!("  Fast Refresh: {}ms", GDEM0397T81P.fast_refresh_ms);
    println!("  Partial Refresh: {}ms", GDEM0397T81P.partial_refresh_ms);
    println!("  Active Area: 86.40 × 51.84mm");
    println!("  PPI: 235 (high resolution!)");
    println!();

    // Draw DAP UI mockup
    draw_dap_ui(&mut emulator).unwrap();

    // Full refresh to show the UI
    println!("Performing full refresh (3 seconds)...");
    emulator.update_buffer().await.unwrap();
    emulator.refresh_full().await.unwrap();

    println!("✓ Display updated");
    println!();

    // Show power stats
    let stats = emulator.power_stats();
    println!("Power Consumption:");
    println!("  Average: {:.1}mA", stats.average_current_ua as f32 / 1000.0);
    println!("  Peak: {:.1}mA", stats.peak_current_ua as f32 / 1000.0);
    println!(
        "  Energy: {:.2}mWh",
        stats.total_energy_uwh as f32 / 1000.0
    );

    if stats.average_current_ua > 0 {
        let battery_3000 = stats.estimated_battery_life_hours(3000);
        println!(
            "  Estimated battery (3000mAh): {:.1} hours",
            battery_3000
        );
    }

    println!();
    println!("Window open - sleeping for 60 seconds (press Ctrl+C to exit)");

    // Keep window open for 60 seconds
    sleep(Duration::from_secs(60)).await;
}

fn draw_dap_ui(display: &mut Emulator) -> Result<(), Box<dyn std::error::Error>> {
    // Text styles
    let title_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    let small_style = MonoTextStyle::new(&FONT_6X10, Gray4::BLACK);
    let light_style = MonoTextStyle::new(&FONT_6X10, Gray4::new(8)); // Mid gray

    // Header bar (top 60px)
    Rectangle::new(Point::new(0, 0), Size::new(800, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(display)?;

    // Title (white on black)
    let title_white = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new(config::APP_NAME, Point::new(10, 35), title_white).draw(display)?;

    // Battery indicator (simple)
    Rectangle::new(Point::new(750, 15), Size::new(40, 20))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::WHITE, 2))
        .draw(display)?;
    Rectangle::new(Point::new(790, 20), Size::new(5, 10))
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(display)?;
    // Battery level (80%)
    Rectangle::new(Point::new(753, 18), Size::new(30, 14))
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(display)?;

    // Album art placeholder (150x150, centered horizontally)
    let album_x = (800 - 150) / 2;
    Rectangle::new(Point::new(album_x as i32, 80), Size::new(150, 150))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(display)?;

    // Diagonal cross for album placeholder
    Line::new(
        Point::new(album_x as i32, 80),
        Point::new(album_x as i32 + 150, 230),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::new(12), 1))
    .draw(display)?;
    Line::new(
        Point::new(album_x as i32 + 150, 80),
        Point::new(album_x as i32, 230),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::new(12), 1))
    .draw(display)?;

    // Album art label
    Text::new("Album Art", Point::new(350, 155), light_style).draw(display)?;

    // Track info (below album art)
    Text::new("Now Playing", Point::new(350, 260), small_style).draw(display)?;

    Text::new(
        "Midnight Groove",
        Point::new(400, 290),
        title_style,
    )
    .draw(display)?;

    Text::new("by The Ambient Collective", Point::new(290, 315), small_style).draw(display)?;

    Text::new("Album: Echoes of Tomorrow", Point::new(280, 335), light_style).draw(display)?;

    // Progress bar
    let progress_y = 360;
    let progress_width = 600;
    let progress_x = (800 - progress_width) / 2;

    // Progress bar background
    Rectangle::new(
        Point::new(progress_x as i32, progress_y),
        Size::new(progress_width, 10),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
    .draw(display)?;

    // Progress (60% complete)
    let filled_width = (progress_width as f32 * 0.6) as u32;
    Rectangle::new(
        Point::new(progress_x as i32, progress_y),
        Size::new(filled_width, 10),
    )
    .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
    .draw(display)?;

    // Time labels
    Text::new("2:24", Point::new(progress_x as i32 - 30, progress_y + 8), small_style).draw(display)?;
    Text::new(
        "4:03",
        Point::new((progress_x + progress_width) as i32 + 5, progress_y + 8),
        small_style,
    )
    .draw(display)?;

    // Playback controls (simple circles)
    let controls_y = 400;
    let center_x = 400;

    // Previous
    Circle::new(Point::new(center_x - 120, controls_y), 30)
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(display)?;
    Text::new("⏮", Point::new(center_x - 115, controls_y + 20), title_style).draw(display)?;

    // Play/Pause
    Circle::new(Point::new(center_x - 20, controls_y), 40)
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(display)?;
    let play_white = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("▶", Point::new(center_x - 7, controls_y + 27), play_white).draw(display)?;

    // Next
    Circle::new(Point::new(center_x + 90, controls_y), 30)
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
        .draw(display)?;
    Text::new("⏭", Point::new(center_x + 95, controls_y + 20), title_style).draw(display)?;

    // Footer info
    Text::new("Format: FLAC 24-bit/96kHz", Point::new(20, 465), small_style).draw(display)?;
    Text::new("♫ Volume: 75%", Point::new(680, 465), small_style).draw(display)?;

    Ok(())
}
