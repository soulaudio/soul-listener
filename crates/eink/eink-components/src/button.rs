//! Button component

use eink_system::prelude::*;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, RoundedRectangle},
    text::{Alignment, Text},
};

/// Button style presets
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ButtonStyle {
    pub background: Gray4,
    pub foreground: Gray4,
    pub border: Option<Gray4>,
    pub padding: Edges,
    pub corner_radius: u32,
}

impl ButtonStyle {
    /// Primary button (dark background, light text)
    pub fn primary() -> Self {
        Self {
            background: Gray4::new(0x2),
            foreground: Gray4::WHITE,
            border: Some(Gray4::BLACK),
            padding: Edges::horizontal_vertical(16, 8),
            corner_radius: 4,
        }
    }

    /// Secondary button (light background, dark text)
    pub fn secondary() -> Self {
        Self {
            background: Gray4::new(0xC),
            foreground: Gray4::BLACK,
            border: Some(Gray4::new(0x8)),
            padding: Edges::horizontal_vertical(16, 8),
            corner_radius: 4,
        }
    }

    /// Text-only button (no background)
    pub fn text() -> Self {
        Self {
            background: Gray4::WHITE,
            foreground: Gray4::BLACK,
            border: None,
            padding: Edges::horizontal_vertical(8, 4),
            corner_radius: 0,
        }
    }
}

/// Button component
pub struct Button {
    label: &'static str,
    style: ButtonStyle,
    min_width: Option<u32>,
}

impl Button {
    /// Create a new button with the given label
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            style: ButtonStyle::primary(),
            min_width: None,
        }
    }

    /// Set button style
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Set minimum width
    pub fn min_width(mut self, width: u32) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Calculate button size including padding
    fn calculate_size(&self) -> Size {
        // Text size: approximately 10 pixels per character width, 20 pixels height
        let text_width = (self.label.len() as u32) * 10;
        let text_height = 20;

        let content_width = text_width + self.style.padding.horizontal();
        let content_height = text_height + self.style.padding.vertical();

        let final_width = if let Some(min_w) = self.min_width {
            content_width.max(min_w)
        } else {
            content_width
        };

        Size::new(final_width, content_height)
    }

    /// Render button to display
    pub fn render<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        let size = self.calculate_size();

        // Draw background (rounded rectangle if corner radius > 0)
        if self.style.corner_radius > 0 {
            let rounded = RoundedRectangle::new(
                Rectangle::new(position, size),
                embedded_graphics::primitives::CornerRadii::new(Size::new(
                    self.style.corner_radius,
                    self.style.corner_radius,
                )),
            );
            rounded
                .into_styled(PrimitiveStyle::with_fill(self.style.background))
                .draw(display)?;

            // Draw border if present
            if let Some(border_color) = self.style.border {
                rounded
                    .into_styled(PrimitiveStyle::with_stroke(border_color, 1))
                    .draw(display)?;
            }
        } else {
            // Regular rectangle
            Rectangle::new(position, size)
                .into_styled(PrimitiveStyle::with_fill(self.style.background))
                .draw(display)?;

            if let Some(border_color) = self.style.border {
                Rectangle::new(position, size)
                    .into_styled(PrimitiveStyle::with_stroke(border_color, 1))
                    .draw(display)?;
            }
        }

        // Draw text centered in button
        let text_style = MonoTextStyle::new(&FONT_10X20, self.style.foreground);
        let text_width = (self.label.len() as i32) * 10;
        let text_x = position.x + (size.width as i32 / 2) - (text_width / 2);
        let text_y = position.y + (size.height as i32 / 2) + 7; // Baseline offset

        Text::new(self.label, Point::new(text_x, text_y), text_style).draw(display)?;

        Ok(())
    }

    /// Get button bounding box
    pub fn bounds(&self, position: Point) -> Rectangle {
        Rectangle::new(position, self.calculate_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_creation() {
        let button = Button::new("Click Me");
        assert_eq!(button.label, "Click Me");
    }

    #[test]
    fn test_button_size_calculation() {
        let button = Button::new("Test");
        let size = button.calculate_size();
        assert!(size.width >= 40); // 4 chars * 10px + padding
        assert_eq!(size.height, 20 + 16); // text height + vertical padding
    }

    #[test]
    fn test_min_width() {
        let button = Button::new("Hi").min_width(100);
        let size = button.calculate_size();
        assert!(size.width >= 100);
    }

    #[test]
    fn test_style_presets() {
        let primary = ButtonStyle::primary();
        assert_eq!(primary.background, Gray4::new(0x2));

        let secondary = ButtonStyle::secondary();
        assert_eq!(secondary.background, Gray4::new(0xC));

        let text = ButtonStyle::text();
        assert!(text.border.is_none());
    }
}
