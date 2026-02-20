#[cfg(feature = "emulator")]
use embedded_graphics::pixelcolor::Gray4;
#[cfg(feature = "emulator")]
use embedded_graphics::prelude::*;
#[cfg(feature = "emulator")]
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

/// Render a simple demo menu onto the display.
///
/// This is the hot-reloadable rendering function. Edit this code, save, and
/// the emulator will reflect the changes without restarting.
///
/// Replace this function with your own UI rendering code.
#[cfg(feature = "emulator")]
pub fn render_demo_menu(
    display: &mut eink_emulator::Emulator,
) -> Result<(), core::convert::Infallible> {
    use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
    use embedded_graphics::text::Text;

    let size = display.bounding_box().size;

    // Header background
    Rectangle::new(Point::zero(), Size::new(size.width, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    // Header text - EDIT THIS to see hot-reload in action!
    let header_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("Main Menu", Point::new(20, 35), header_style).draw(display)?;

    // Menu items
    let menu_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    let menu_items = [
        "1. Now Playing",
        "2. Library",
        "3. Playlists",
        "4. Settings",
        "5. About",
    ];

    for (idx, item) in menu_items.iter().enumerate() {
        let y = 100 + (idx as i32 * 50);

        // Item background (alternate shading)
        if idx % 2 == 0 {
            Rectangle::new(Point::new(10, y - 5), Size::new(size.width - 20, 45))
                .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xE)))
                .draw(display)?;
        }

        Text::new(item, Point::new(30, y + 20), menu_style).draw(display)?;
    }

    // Footer hint
    let footer_style = MonoTextStyle::new(&FONT_10X20, Gray4::new(0x8));
    Text::new(
        "Edit render.rs and save to hot-reload!",
        Point::new(30, (size.height - 30) as i32),
        footer_style,
    )
    .draw(display)?;

    Ok(())
}

/// Render onto a mutable Emulator reference (used by the hot-reload C ABI).
///
/// This is called by render_ui() in lib.rs via the raw pointer interface.
/// Delegates directly to render_demo_menu().
#[cfg(feature = "emulator")]
#[allow(dead_code)] // called via raw fn pointer by hot-lib-reloader; clippy can't see it
pub fn render_onto_emulator(
    emulator: &mut eink_emulator::Emulator,
) -> Result<(), core::convert::Infallible> {
    render_demo_menu(emulator)
}
