//! [`Icon`] — a small built-in vector glyph (transport controls), drawn with
//! `embedded-graphics` primitives so it scales to any size without a bitmap.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder, Rectangle, Triangle};
use embedded_graphics::Drawable;

use crate::theme::{Role, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};

/// The built-in glyph set. v1 covers the transport controls a music player's
/// Now-Playing screen needs; more can be added without changing the [`Icon`] API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    /// A right-pointing filled triangle (▶).
    Play,
    /// Two vertical bars (⏸).
    Pause,
    /// A filled square (⏹).
    Stop,
    /// Right-pointing triangle against a trailing bar (⏭).
    Next,
    /// A leading bar against a left-pointing triangle (⏮).
    Prev,
}

/// A fixed-size square icon rendering one of the built-in [`Glyph`]s in a
/// [`Role`] color (default [`Role::Ink`]). Drawn from primitives, so it is crisp
/// at any `size`.
///
/// # Examples
///
/// ```
/// use eink_ui::{Icon, Role};
/// use eink_ui::widgets::Glyph;
///
/// let play = Icon::new(Glyph::Play, 40);
/// let paused = Icon::new(Glyph::Pause, 40).role(Role::Muted);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Icon {
    glyph: Glyph,
    size: u32,
    role: Role,
}

impl Icon {
    /// A `size`x`size` icon drawing `glyph` in [`Role::Ink`].
    #[must_use]
    pub const fn new(glyph: Glyph, size: u32) -> Self {
        Icon {
            glyph,
            size,
            role: Role::Ink,
        }
    }

    /// Set the icon's color role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

/// `rect`'s top-left and bottom-right as `(x0, y0, x1, y1)` i32 coordinates.
#[allow(clippy::cast_possible_wrap)] // display-sized rects, << i32::MAX
fn corners(rect: Rectangle) -> (i32, i32, i32, i32) {
    let x0 = rect.top_left.x;
    let y0 = rect.top_left.y;
    let x1 = x0.saturating_add(rect.size.width.saturating_sub(1) as i32);
    let y1 = y0.saturating_add(rect.size.height.saturating_sub(1) as i32);
    (x0, y0, x1, y1)
}

impl Widget for Icon {
    fn measure(&self, limits: Constraints, _theme: &Theme) -> Size {
        limits.clamp(Size::new(self.size, self.size))
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let fill = PrimitiveStyleBuilder::new()
            .fill_color(theme.color(self.role))
            .build();
        let (x0, y0, x1, y1) = corners(rect);
        let w = rect.size.width;
        let h = rect.size.height;
        // A bar one-third of the width, and the gap between paired bars.
        let bar_w = (w / 3).max(1);
        let ymid = y0.saturating_add(h.saturating_sub(1) as i32 / 2);

        match self.glyph {
            Glyph::Play => {
                Triangle::new(Point::new(x0, y0), Point::new(x0, y1), Point::new(x1, ymid))
                    .into_styled(fill)
                    .draw(target)?;
            }
            Glyph::Stop => {
                rect.into_styled(fill).draw(target)?;
            }
            Glyph::Pause => {
                let bar = |x: i32| Rectangle::new(Point::new(x, y0), Size::new(bar_w, h).to_eg());
                bar(x0).into_styled(fill).draw(target)?;
                bar(x1.saturating_sub(bar_w as i32).saturating_add(1))
                    .into_styled(fill)
                    .draw(target)?;
            }
            Glyph::Next => {
                // Triangle in the left two-thirds, bar at the right edge.
                let tri_x1 = x1.saturating_sub(bar_w as i32);
                Triangle::new(
                    Point::new(x0, y0),
                    Point::new(x0, y1),
                    Point::new(tri_x1, ymid),
                )
                .into_styled(fill)
                .draw(target)?;
                Rectangle::new(
                    Point::new(x1.saturating_sub(bar_w as i32).saturating_add(1), y0),
                    Size::new(bar_w, h).to_eg(),
                )
                .into_styled(fill)
                .draw(target)?;
            }
            Glyph::Prev => {
                // Bar at the left edge, left-pointing triangle in the right two-thirds.
                Rectangle::new(Point::new(x0, y0), Size::new(bar_w, h).to_eg())
                    .into_styled(fill)
                    .draw(target)?;
                let tri_x0 = x0.saturating_add(bar_w as i32);
                Triangle::new(
                    Point::new(x1, y0),
                    Point::new(x1, y1),
                    Point::new(tri_x0, ymid),
                )
                .into_styled(fill)
                .draw(target)?;
            }
        }
        Ok(())
    }
}
