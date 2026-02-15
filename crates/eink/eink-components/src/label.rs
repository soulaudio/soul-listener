//! Label component for displaying text

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray4,
    prelude::*,
    text::Text,
};

/// Text size variants
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TextSize {
    Small,  // 6x10 font
    Normal, // 10x20 font
}

impl TextSize {
    pub fn line_height(&self) -> u32 {
        match self {
            TextSize::Small => 10,
            TextSize::Normal => 20,
        }
    }

    pub fn char_width(&self) -> u32 {
        match self {
            TextSize::Small => 6,
            TextSize::Normal => 10,
        }
    }
}

/// Label component for static text display
pub struct Label {
    text: &'static str,
    color: Gray4,
    size: TextSize,
}

impl Label {
    /// Create a new label with the given text
    pub fn new(text: &'static str) -> Self {
        Self {
            text,
            color: Gray4::BLACK,
            size: TextSize::Normal,
        }
    }

    /// Set text color
    pub fn color(mut self, color: Gray4) -> Self {
        self.color = color;
        self
    }

    /// Set text size
    pub fn size(mut self, size: TextSize) -> Self {
        self.size = size;
        self
    }

    /// Get text dimensions
    pub fn dimensions(&self) -> Size {
        Size::new(
            (self.text.len() as u32) * self.size.char_width(),
            self.size.line_height(),
        )
    }

    /// Render label to display
    pub fn render<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        let text_style = match self.size {
            TextSize::Small => MonoTextStyle::new(&FONT_6X10, self.color),
            TextSize::Normal => MonoTextStyle::new(&FONT_10X20, self.color),
        };

        Text::new(self.text, position, text_style).draw(display)?;

        Ok(())
    }
}

/// Helper for creating labels with different styles
pub struct LabelBuilder;

impl LabelBuilder {
    /// Create a heading label (larger, bold equivalent)
    pub fn heading(text: &'static str) -> Label {
        Label::new(text).color(Gray4::BLACK).size(TextSize::Normal)
    }

    /// Create a subtitle label (smaller)
    pub fn subtitle(text: &'static str) -> Label {
        Label::new(text)
            .color(Gray4::new(0x4))
            .size(TextSize::Small)
    }

    /// Create a caption label (small, light)
    pub fn caption(text: &'static str) -> Label {
        Label::new(text)
            .color(Gray4::new(0x8))
            .size(TextSize::Small)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_creation() {
        let label = Label::new("Hello World");
        assert_eq!(label.text, "Hello World");
        assert_eq!(label.color, Gray4::BLACK);
    }

    #[test]
    fn test_label_dimensions() {
        let label = Label::new("Test");
        let dims = label.dimensions();
        assert_eq!(dims.width, 4 * 10); // 4 chars * 10px
        assert_eq!(dims.height, 20);
    }

    #[test]
    fn test_text_sizes() {
        assert_eq!(TextSize::Small.line_height(), 10);
        assert_eq!(TextSize::Normal.line_height(), 20);
        assert_eq!(TextSize::Small.char_width(), 6);
        assert_eq!(TextSize::Normal.char_width(), 10);
    }

    #[test]
    fn test_label_builder() {
        let heading = LabelBuilder::heading("Title");
        assert_eq!(heading.size, TextSize::Normal);

        let subtitle = LabelBuilder::subtitle("Subtitle");
        assert_eq!(subtitle.size, TextSize::Small);

        let caption = LabelBuilder::caption("Caption");
        assert_eq!(caption.size, TextSize::Small);
    }
}
