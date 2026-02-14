//! Component demonstration

use eink_components::prelude::*;
use eink_emulator::{Emulator, EmulatorConfig, Rotation};
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("E-Ink Components Demo");
    println!("=====================\n");

    // Create emulator in portrait mode
    let config = EmulatorConfig {
        rotation: Rotation::Degrees90,
        scale: 2,
    };
    let mut emulator = Emulator::with_config(config);

    // Clear to white
    let size = emulator.bounding_box().size;
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(&mut emulator)?;

    println!("Rendering components...");

    // Draw buttons
    let button1 = Button::new("Play").style(ButtonStyle::primary());
    button1.render(&mut emulator, Point::new(20, 20))?;

    let button2 = Button::new("Stop")
        .style(ButtonStyle::secondary())
        .min_width(100);
    button2.render(&mut emulator, Point::new(20, 70))?;

    let button3 = Button::new("Settings").style(ButtonStyle::text());
    button3.render(&mut emulator, Point::new(20, 120))?;

    // Draw labels
    let heading = LabelBuilder::heading("Now Playing");
    heading.render(&mut emulator, Point::new(200, 30))?;

    let subtitle = LabelBuilder::subtitle("Track Information");
    subtitle.render(&mut emulator, Point::new(200, 55))?;

    let caption = LabelBuilder::caption("00:45 / 03:42");
    caption.render(&mut emulator, Point::new(200, 75))?;

    // Draw progress bars
    let progress1 = ProgressBar::new(200, 10).progress(0.25);
    progress1.render(&mut emulator, Point::new(200, 100))?;

    let progress2 = ProgressBar::new(200, 10).progress(0.75).colors(
        Gray4::new(0xE),
        Gray4::new(0x2),
    );
    progress2.render(&mut emulator, Point::new(200, 120))?;

    // Draw icons
    let icon_size = 24;
    let icon_y = 160;
    Icon::new(IconType::Play, icon_size).render(&mut emulator, Point::new(20, icon_y))?;
    Icon::new(IconType::Pause, icon_size).render(&mut emulator, Point::new(60, icon_y))?;
    Icon::new(IconType::Stop, icon_size).render(&mut emulator, Point::new(100, icon_y))?;
    Icon::new(IconType::Previous, icon_size).render(&mut emulator, Point::new(140, icon_y))?;
    Icon::new(IconType::Next, icon_size).render(&mut emulator, Point::new(180, icon_y))?;
    Icon::new(IconType::Settings, icon_size).render(&mut emulator, Point::new(220, icon_y))?;

    // Refresh display
    emulator.refresh_full().await?;

    println!("✓ Components rendered\n");
    println!("Window shows:");
    println!("  - Buttons (Primary, Secondary, Text styles)");
    println!("  - Labels (Heading, Subtitle, Caption)");
    println!("  - Progress bars (different progress levels)");
    println!("  - Icons (Play, Pause, Stop, Previous, Next, Settings)");
    println!("\nClose window to exit.");

    emulator.run();
    Ok(())
}
