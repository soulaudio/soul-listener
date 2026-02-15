//! User Interface Module
//!
//! This module contains UI screens and components for the SoulAudio DAP.
//! Currently a placeholder for future UI implementation.

use embedded_graphics::mono_font::{ascii::FONT_9X18, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray2;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

/// Splash screen - shown on boot
pub struct SplashScreen;

impl SplashScreen {
    /// Render the splash screen to a display
    pub fn render<D, C>(display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor + From<Gray2>,
    {
        // Clear screen (white background)
        let bounds = display.bounding_box();
        Rectangle::new(bounds.top_left, bounds.size)
            .into_styled(PrimitiveStyle::with_fill(C::from(Gray2::WHITE)))
            .draw(display)?;

        // Draw "SoulAudio" text in center (black text)
        let text_style = MonoTextStyle::new(&FONT_9X18, C::from(Gray2::BLACK));

        let center = bounds.center();
        Text::new(
            "SoulAudio",
            Point::new(center.x - 45, center.y - 9),
            text_style,
        )
        .draw(display)?;

        Text::new(
            "DAP v0.1.0",
            Point::new(center.x - 45, center.y + 18),
            text_style,
        )
        .draw(display)?;

        Ok(())
    }
}

/// Test pattern - for hardware validation
pub struct TestPattern;

impl TestPattern {
    /// Render a test pattern
    pub fn render<D, C>(display: &mut D) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
        C: PixelColor + From<Gray2>,
    {
        // Clear to white
        let bounds = display.bounding_box();
        Rectangle::new(bounds.top_left, bounds.size)
            .into_styled(PrimitiveStyle::with_fill(C::from(Gray2::WHITE)))
            .draw(display)?;

        // Draw border (black)
        Rectangle::new(bounds.top_left, bounds.size)
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;

        // Draw crosshair
        let center = bounds.center();
        let size = bounds.size;

        // Horizontal line
        Line::new(
            Point::new(0, center.y),
            Point::new(size.width as i32 - 1, center.y),
        )
        .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 1))
        .draw(display)?;

        // Vertical line
        Line::new(
            Point::new(center.x, 0),
            Point::new(center.x, size.height as i32 - 1),
        )
        .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 1))
        .draw(display)?;

        // Corner marks
        let mark_size = 20;

        // Top-left
        Line::new(Point::new(10, 10), Point::new(10 + mark_size, 10))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;
        Line::new(Point::new(10, 10), Point::new(10, 10 + mark_size))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;

        // Top-right
        let tr_x = size.width as i32 - 10;
        Line::new(Point::new(tr_x, 10), Point::new(tr_x - mark_size, 10))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;
        Line::new(Point::new(tr_x, 10), Point::new(tr_x, 10 + mark_size))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;

        // Bottom-left
        let bl_y = size.height as i32 - 10;
        Line::new(Point::new(10, bl_y), Point::new(10 + mark_size, bl_y))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;
        Line::new(Point::new(10, bl_y), Point::new(10, bl_y - mark_size))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;

        // Bottom-right
        let br_x = size.width as i32 - 10;
        let br_y = size.height as i32 - 10;
        Line::new(Point::new(br_x, br_y), Point::new(br_x - mark_size, br_y))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;
        Line::new(Point::new(br_x, br_y), Point::new(br_x, br_y - mark_size))
            .into_styled(PrimitiveStyle::with_stroke(C::from(Gray2::BLACK), 2))
            .draw(display)?;

        Ok(())
    }
}
