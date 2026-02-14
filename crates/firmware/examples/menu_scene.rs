//! Simple Menu Scene - DAP main menu demonstration
//!
//! Demonstrates:
//! - Vertical layout with eink-system
//! - Menu items using eink-components
//! - Icons paired with labels
//! - List-style navigation interface

use eink_components::prelude::*;
use eink_emulator::{Emulator, EmulatorConfig, Rotation};
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("DAP - Main Menu Scene");
    println!("====================\n");

    // Create emulator in portrait mode
    let config = EmulatorConfig {
        rotation: Rotation::Degrees90,
        scale: 2,
    };
    let mut emulator = Emulator::with_config(config);

    // Render the menu scene
    render_menu(&mut emulator)?;

    println!("✓ Menu scene rendered\n");
    println!("Window shows:");
    println!("  - Header with title");
    println!("  - 6 menu items with icons");
    println!("  - Selection indicator");
    println!("  - Footer with navigation hints");
    println!("\nClose window to exit.");

    emulator.run();
    Ok(())
}

fn render_menu<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray4>,
{
    let size = display.bounding_box().size;

    // Clear to white background
    Rectangle::new(Point::zero(), size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(display)?;

    // Header background (dark bar)
    Rectangle::new(Point::zero(), Size::new(size.width, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    // Header title
    let header = Label::new("Main Menu")
        .color(Gray4::WHITE)
        .size(TextSize::Normal);
    header.render(display, Point::new(20, 25))?;

    // Menu items start position
    let menu_y = 80;
    let item_height = 60;
    let selected_item = 0; // First item selected

    // Menu items list
    let menu_items = [
        ("Now Playing", IconType::Play),
        ("Library", IconType::Settings), // Using Settings icon as placeholder
        ("Playlists", IconType::Stop),   // Using Stop icon as placeholder
        ("Settings", IconType::Settings),
        ("About", IconType::Pause),      // Using Pause icon as placeholder
        ("Power Off", IconType::Stop),
    ];

    // Render each menu item
    for (idx, (label_text, icon_type)) in menu_items.iter().enumerate() {
        let y_pos = menu_y + (idx as i32 * item_height);

        // Draw selection background for selected item
        if idx == selected_item {
            Rectangle::new(
                Point::new(10, y_pos - 5),
                Size::new(size.width - 20, item_height as u32 - 10),
            )
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xE)))
            .draw(display)?;
        }

        // Draw icon
        Icon::new(*icon_type, 24).render(
            display,
            Point::new(30, y_pos + 10),
        )?;

        // Draw label
        let label = Label::new(label_text)
            .color(if idx == selected_item {
                Gray4::new(0x0) // Black for selected
            } else {
                Gray4::new(0x4) // Dark gray for unselected
            })
            .size(TextSize::Normal);
        label.render(display, Point::new(70, y_pos + 15))?;

        // Draw separator line (except for last item)
        if idx < menu_items.len() - 1 {
            let line_y = y_pos + item_height - 5;
            for x in 20..(size.width - 20) {
                display.draw_iter(core::iter::once(Pixel(
                    Point::new(x as i32, line_y),
                    Gray4::new(0xC),
                )))?;
            }
        }
    }

    // Footer navigation hints
    let footer_y = (size.height - 40) as i32;

    // Footer background (light gray bar)
    Rectangle::new(
        Point::new(0, footer_y),
        Size::new(size.width, 40),
    )
    .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xE)))
    .draw(display)?;

    // Navigation hint: Up/Down
    Icon::new(IconType::VolumeUp, 16).render(
        display,
        Point::new(40, footer_y + 12),
    )?;
    let nav_label = Label::new("Navigate")
        .color(Gray4::new(0x4))
        .size(TextSize::Small);
    nav_label.render(display, Point::new(65, footer_y + 15))?;

    // Select hint
    Icon::new(IconType::Play, 16).render(
        display,
        Point::new((size.width - 100) as i32, footer_y + 12),
    )?;
    let select_label = Label::new("Select")
        .color(Gray4::new(0x4))
        .size(TextSize::Small);
    select_label.render(display, Point::new((size.width - 70) as i32, footer_y + 15))?;

    Ok(())
}
