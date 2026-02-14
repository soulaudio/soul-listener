//! Simple icon component

use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle, Triangle},
};

/// Icon types
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum IconType {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    Settings,
}

/// Simple icon component
pub struct Icon {
    icon_type: IconType,
    size: u32,
    color: Gray4,
}

impl Icon {
    /// Create a new icon
    pub fn new(icon_type: IconType, size: u32) -> Self {
        Self {
            icon_type,
            size,
            color: Gray4::BLACK,
        }
    }

    /// Set icon color
    pub fn color(mut self, color: Gray4) -> Self {
        self.color = color;
        self
    }

    /// Get icon dimensions
    pub fn dimensions(&self) -> Size {
        Size::new(self.size, self.size)
    }

    /// Render icon to display
    pub fn render<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        match self.icon_type {
            IconType::Play => self.render_play(display, position),
            IconType::Pause => self.render_pause(display, position),
            IconType::Stop => self.render_stop(display, position),
            IconType::Next => self.render_next(display, position),
            IconType::Previous => self.render_previous(display, position),
            IconType::VolumeUp => self.render_volume_up(display, position),
            IconType::VolumeDown => self.render_volume_down(display, position),
            IconType::Settings => self.render_settings(display, position),
        }
    }

    fn render_play<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw play triangle pointing right
        let offset = (self.size as i32) / 2;
        let p1 = position + Point::new(0, 0);
        let p2 = position + Point::new(0, self.size as i32);
        let p3 = position + Point::new(self.size as i32, offset);

        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        Ok(())
    }

    fn render_pause<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw two vertical bars
        let bar_width = self.size / 3;
        let gap = self.size / 6;

        Rectangle::new(position, Size::new(bar_width, self.size))
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        Rectangle::new(
            position + Point::new((bar_width + gap) as i32, 0),
            Size::new(bar_width, self.size),
        )
        .into_styled(PrimitiveStyle::with_fill(self.color))
        .draw(display)?;

        Ok(())
    }

    fn render_stop<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw solid square
        Rectangle::new(position, Size::new(self.size, self.size))
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        Ok(())
    }

    fn render_next<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw two triangles pointing right
        let half_size = self.size / 2;
        let offset = (half_size as i32) / 2;

        // First triangle
        let p1 = position;
        let p2 = position + Point::new(0, half_size as i32);
        let p3 = position + Point::new(half_size as i32, offset);
        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        // Second triangle
        let p1 = position + Point::new(half_size as i32, 0);
        let p2 = position + Point::new(half_size as i32, half_size as i32);
        let p3 = position + Point::new(self.size as i32, offset);
        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        Ok(())
    }

    fn render_previous<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw two triangles pointing left
        let half_size = self.size / 2;
        let offset = (half_size as i32) / 2;

        // First triangle (right)
        let p1 = position + Point::new(self.size as i32, 0);
        let p2 = position + Point::new(self.size as i32, half_size as i32);
        let p3 = position + Point::new(half_size as i32, offset);
        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        // Second triangle (left)
        let p1 = position + Point::new(half_size as i32, 0);
        let p2 = position + Point::new(half_size as i32, half_size as i32);
        let p3 = position + Point::new(0, offset);
        Triangle::new(p1, p2, p3)
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        Ok(())
    }

    fn render_volume_up<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw speaker symbol (simplified)
        let bar_width = self.size / 4;
        Rectangle::new(position, Size::new(bar_width, self.size / 2))
            .into_styled(PrimitiveStyle::with_fill(self.color))
            .draw(display)?;

        // Sound waves (circles)
        let offset_x = (bar_width + 4) as i32;
        let offset_y = (self.size / 4) as i32;
        Circle::new(
            position + Point::new(offset_x, offset_y),
            self.size / 8,
        )
        .into_styled(PrimitiveStyle::with_stroke(self.color, 1))
        .draw(display)?;

        Ok(())
    }

    fn render_volume_down<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Same as volume up but smaller
        self.render_volume_up(display, position)
    }

    fn render_settings<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        // Draw gear/cog (simplified as circle with center)
        let radius = self.size / 2;
        let center = position + Point::new(radius as i32, radius as i32);

        Circle::new(position, self.size)
            .into_styled(PrimitiveStyle::with_stroke(self.color, 2))
            .draw(display)?;

        Circle::new(
            center - Point::new((radius / 2) as i32, (radius / 2) as i32),
            radius,
        )
        .into_styled(PrimitiveStyle::with_fill(self.color))
        .draw(display)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_creation() {
        let icon = Icon::new(IconType::Play, 24);
        assert_eq!(icon.icon_type, IconType::Play);
        assert_eq!(icon.size, 24);
    }

    #[test]
    fn test_icon_color() {
        let icon = Icon::new(IconType::Play, 24).color(Gray4::new(0x8));
        assert_eq!(icon.color, Gray4::new(0x8));
    }

    #[test]
    fn test_icon_dimensions() {
        let icon = Icon::new(IconType::Play, 32);
        assert_eq!(icon.dimensions(), Size::new(32, 32));
    }
}
