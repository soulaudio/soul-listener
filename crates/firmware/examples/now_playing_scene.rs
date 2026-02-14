//! Now Playing Scene - Simple DAP UI demonstration

use eink_components::prelude::*;
use eink_emulator::{Emulator, EmulatorConfig, Rotation, WaveformMode};
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DAP - Now Playing Scene");
    println!("========================\n");

    // Create emulator in portrait mode (DAP style)
    let config = EmulatorConfig {
        rotation: Rotation::Degrees90,
        scale: 2,
    };
    let mut emulator = Emulator::with_config(config);

    // Render the now playing scene
    render_now_playing(&mut emulator).await?;

    // Present the framebuffer to the window
    emulator.refresh_with_waveform(WaveformMode::GC16).await?;

    println!("✓ Now Playing scene rendered\n");
    println!("Window shows:");
    println!("  - Header with title");
    println!("  - Album art placeholder");
    println!("  - Track information");
    println!("  - Progress bar");
    println!("  - Playback controls");
    println!("\nClose window to exit.");

    emulator.run();
    Ok(())
}

async fn render_now_playing<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray4>,
{
    let size = display.bounding_box().size;

    // Clear to white background
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(display)?;

    // Header background (dark bar)
    Rectangle::new(Point::zero(), Size::new(size.width, 50))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    // Header title
    let header = Label::new("Now Playing")
        .color(Gray4::WHITE)
        .size(TextSize::Normal);
    header.render(display, Point::new(20, 20))?;

    // Album art placeholder (centered square)
    let art_size = 200;
    let art_x = (size.width / 2) - (art_size / 2);
    let art_y = 80;
    Rectangle::new(
        Point::new(art_x as i32, art_y as i32),
        Size::new(art_size, art_size),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::new(0x8), 2))
    .draw(display)?;

    // Placeholder text in album art
    let art_label = Label::new("ALBUM ART")
        .color(Gray4::new(0xA))
        .size(TextSize::Small);
    art_label.render(
        display,
        Point::new((size.width / 2 - 30) as i32, (art_y + art_size / 2) as i32),
    )?;

    // Track info section
    let info_y = art_y + art_size + 30;

    // Artist name
    let artist = LabelBuilder::subtitle("Artist Name");
    artist.render(display, Point::new(40, info_y as i32))?;

    // Track title (larger)
    let track = LabelBuilder::heading("Track Title");
    track.render(display, Point::new(40, (info_y + 20) as i32))?;

    // Album name
    let album = LabelBuilder::caption("Album Name • 2024");
    album.render(display, Point::new(40, (info_y + 50) as i32))?;

    // Progress bar
    let progress_y = info_y + 80;
    let progress = ProgressBar::new(size.width - 80, 12)
        .progress(0.35)
        .colors(Gray4::new(0xE), Gray4::new(0x2));
    progress.render(display, Point::new(40, progress_y as i32))?;

    // Time labels
    let time_current = Label::new("01:25").size(TextSize::Small);
    time_current.render(display, Point::new(40, (progress_y + 20) as i32))?;

    let time_total = Label::new("04:02").size(TextSize::Small);
    time_total.render(
        display,
        Point::new((size.width - 70) as i32, (progress_y + 20) as i32),
    )?;

    // Playback controls (centered)
    let controls_y = progress_y + 50;
    let icon_size = 32;
    let icon_spacing = 60;
    let start_x = (size.width / 2) as i32 - (icon_spacing * 2);

    // Previous
    Icon::new(IconType::Previous, icon_size).render(
        display,
        Point::new(start_x, controls_y as i32),
    )?;

    // Play (larger)
    Icon::new(IconType::Play, icon_size + 8).render(
        display,
        Point::new(start_x + icon_spacing, controls_y as i32 - 4),
    )?;

    // Next
    Icon::new(IconType::Next, icon_size).render(
        display,
        Point::new(start_x + icon_spacing * 2, controls_y as i32),
    )?;

    // Volume icon
    Icon::new(IconType::VolumeUp, 24).render(
        display,
        Point::new(start_x + icon_spacing * 3, controls_y as i32 + 4),
    )?;

    // Footer controls
    let footer_y = (size.height - 60) as i32;

    // Settings button
    let settings_btn = Button::new("Settings")
        .style(ButtonStyle::text())
        .min_width(80);
    settings_btn.render(display, Point::new(30, footer_y))?;

    // Menu button
    let menu_btn = Button::new("Menu")
        .style(ButtonStyle::text())
        .min_width(80);
    menu_btn.render(
        display,
        Point::new((size.width - 110) as i32, footer_y),
    )?;

    Ok(())
}
