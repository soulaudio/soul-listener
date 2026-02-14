//! Progress bar component

use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};

/// Progress bar component
pub struct ProgressBar {
    width: u32,
    height: u32,
    progress: f32, // 0.0 to 1.0
    background: Gray4,
    foreground: Gray4,
    border: Option<Gray4>,
}

impl ProgressBar {
    /// Create a new progress bar
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            progress: 0.0,
            background: Gray4::WHITE,
            foreground: Gray4::BLACK,
            border: Some(Gray4::new(0x8)),
        }
    }

    /// Set progress (0.0 to 1.0)
    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    /// Set colors
    pub fn colors(mut self, background: Gray4, foreground: Gray4) -> Self {
        self.background = background;
        self.foreground = foreground;
        self
    }

    /// Set border color (None for no border)
    pub fn border(mut self, border: Option<Gray4>) -> Self {
        self.border = border;
        self
    }

    /// Get dimensions
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Render progress bar to display
    pub fn render<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        let size = Size::new(self.width, self.height);

        // Draw background
        Rectangle::new(position, size)
            .into_styled(PrimitiveStyle::with_fill(self.background))
            .draw(display)?;

        // Draw border if present
        if let Some(border_color) = self.border {
            Rectangle::new(position, size)
                .into_styled(PrimitiveStyle::with_stroke(border_color, 1))
                .draw(display)?;
        }

        // Draw filled portion
        let fill_width = ((self.width - 2) as f32 * self.progress) as u32; // -2 for border
        if fill_width > 0 {
            let fill_size = Size::new(fill_width, self.height - 2); // -2 for border
            let fill_position = Point::new(position.x + 1, position.y + 1);

            Rectangle::new(fill_position, fill_size)
                .into_styled(PrimitiveStyle::with_fill(self.foreground))
                .draw(display)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_creation() {
        let bar = ProgressBar::new(100, 10);
        assert_eq!(bar.width, 100);
        assert_eq!(bar.height, 10);
        assert_eq!(bar.progress, 0.0);
    }

    #[test]
    fn test_progress_clamping() {
        let bar = ProgressBar::new(100, 10).progress(1.5);
        assert_eq!(bar.progress, 1.0);

        let bar = ProgressBar::new(100, 10).progress(-0.5);
        assert_eq!(bar.progress, 0.0);
    }

    #[test]
    fn test_custom_colors() {
        let bar = ProgressBar::new(100, 10).colors(Gray4::new(0xA), Gray4::new(0x2));
        assert_eq!(bar.background, Gray4::new(0xA));
        assert_eq!(bar.foreground, Gray4::new(0x2));
    }

    #[test]
    fn test_no_border() {
        let bar = ProgressBar::new(100, 10).border(None);
        assert!(bar.border.is_none());
    }
}
