//! User Interface Module
//!
//! This module contains UI screens and components for the SoulAudio DAP.
//! Currently a placeholder for future UI implementation.

#![allow(clippy::doc_markdown)] // UI docs reference types that are clearer without enforced backtick formatting
// UI rendering code casts display dimensions (u32 from embedded-graphics) to i32
// for coordinate arithmetic.  Display sizes are at most 800×480, which fit safely
// in i32.  Arithmetic operations on small display coordinates cannot overflow i32.
#![allow(
    clippy::cast_possible_wrap,
    clippy::arithmetic_side_effects,
)]

use embedded_graphics::mono_font::{ascii::FONT_9X18, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray2;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;

/// Splash screen - shown on boot
pub struct SplashScreen;

impl SplashScreen {
    /// Render the splash screen to a display
    ///
    /// # Errors
    ///
    /// Returns `D::Error` if any drawing operation fails.
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
    ///
    /// # Errors
    ///
    /// Returns `D::Error` if any drawing operation fails.
    #[allow(clippy::similar_names)] // bl_y / br_y: corner coordinates, names are intentionally symmetric
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

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::pixelcolor::Gray2;

    struct TestDisplay {
        width: u32,
        height: u32,
        pixel_count: usize,
    }

    impl TestDisplay {
        fn new(width: u32, height: u32) -> Self {
            Self {
                width,
                height,
                pixel_count: 0,
            }
        }
    }

    impl DrawTarget for TestDisplay {
        type Color = Gray2;
        type Error = core::convert::Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            self.pixel_count += pixels.into_iter().count();
            Ok(())
        }
    }

    impl OriginDimensions for TestDisplay {
        fn size(&self) -> Size {
            Size::new(self.width, self.height)
        }
    }

    #[test]
    fn test_splash_screen_renders_without_error() {
        let mut display = TestDisplay::new(200, 100);
        let result = SplashScreen::render(&mut display);
        assert!(result.is_ok());
        assert!(display.pixel_count > 0, "SplashScreen should draw pixels");
    }

    #[test]
    fn test_test_pattern_renders_without_error() {
        let mut display = TestDisplay::new(200, 100);
        let result = TestPattern::render(&mut display);
        assert!(result.is_ok());
        assert!(display.pixel_count > 0, "TestPattern should draw pixels");
    }

    #[test]
    fn test_splash_screen_small_display() {
        // Should not panic on a very small display
        let mut display = TestDisplay::new(50, 50);
        assert!(SplashScreen::render(&mut display).is_ok());
    }

    #[test]
    fn test_test_pattern_small_display() {
        let mut display = TestDisplay::new(50, 50);
        assert!(TestPattern::render(&mut display).is_ok());
    }
}
