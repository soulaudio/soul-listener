//! Menu screenshot - captures menu scene

use eink_components::prelude::*;
use eink_emulator::{Emulator, EmulatorConfig, Rotation, WaveformMode};
use eink_specs::displays::WAVESHARE_7_5_V2;
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Generating menu screenshot...\n");

    let rotation = Rotation::Degrees90;
    let config = EmulatorConfig { rotation, scale: 1 };
    let mut emulator = Emulator::with_spec_and_config(&WAVESHARE_7_5_V2, config);

    // Render menu
    render_menu(&mut emulator)?;

    // Refresh
    emulator.refresh_with_waveform(WaveformMode::GC16).await?;

    // Screenshot
    emulator.screenshot("menu_portrait.png")?;

    let size = emulator.bounding_box().size;
    println!("✓ Screenshot saved: menu_portrait.png");
    println!("  Size: {}×{} (portrait)", size.width, size.height);
    println!("  Rotation: {:?}", rotation);

    Ok(())
}

fn render_menu<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray4>,
{
    let size = display.bounding_box().size;

    // Clear white
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(display)?;

    // Header
    Rectangle::new(Point::zero(), Size::new(size.width, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    let header_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("Main Menu", Point::new(20, 35), header_style).draw(display)?;

    // Menu items
    let menu_items = [
        "1. Now Playing",
        "2. Library",
        "3. Playlists",
        "4. Settings",
        "5. About",
    ];

    let menu_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);

    for (idx, item) in menu_items.iter().enumerate() {
        let y = 100 + (idx as i32 * 70);

        // Alternate backgrounds
        if idx % 2 == 0 {
            Rectangle::new(Point::new(10, y - 5), Size::new(size.width - 20, 65))
                .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xE)))
                .draw(display)?;
        }

        Text::new(item, Point::new(30, y + 25), menu_style).draw(display)?;
    }

    // Footer
    let footer_style = MonoTextStyle::new(&FONT_10X20, Gray4::new(0x8));
    Text::new(
        "Portrait Mode - 480x800",
        Point::new(30, (size.height - 30) as i32),
        footer_style,
    )
    .draw(display)?;

    Ok(())
}
