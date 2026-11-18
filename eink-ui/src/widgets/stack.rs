//! [`Stack`] — overlay children on the same rectangle (z-order).

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;

use crate::layout::{Align, Children};
use crate::theme::Theme;
use crate::widget::{Constraints, DrawError, Size, Widget};

/// Draws its [`Children`] one atop another in the same rectangle — the first
/// child at the back, the last on top. The stack measures to the bounding box of
/// its largest child; each child is placed within that box by a 2-D [`Align`]
/// (applied to both axes), or stretched to fill it under [`Align::Stretch`].
///
/// Useful for a filled background behind a centered label, a progress ring with
/// a centered readout, a badge pinned to a corner (`Align::End`), etc.
///
/// # Examples
///
/// ```
/// use eink_ui::{Container, Role, Stack, Text};
/// use eink_ui::layout::Align;
///
/// let badge = Stack::new((
///     Container::new(Text::new("")).background(Some(Role::Faint)),
///     Text::new("12"),
/// ))
/// .align(Align::Center);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Stack<C> {
    children: C,
    align: Align,
}

impl<C: Children> Stack<C> {
    /// A stack over `children`, centering each by default.
    #[must_use]
    pub const fn new(children: C) -> Self {
        Stack {
            children,
            align: Align::Center,
        }
    }

    /// Set how each child is placed within the stack's bounding box.
    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

/// One-axis offset placing a `child`-long item within an `avail`-long track.
#[allow(clippy::cast_possible_wrap)] // offsets are display coordinates, << i32::MAX
fn axis_offset(align: Align, avail: u32, child: u32) -> i32 {
    let slack = avail.saturating_sub(child);
    match align {
        Align::Start | Align::Stretch => 0,
        Align::Center => (slack / 2) as i32,
        Align::End => slack as i32,
    }
}

impl<C: Children> Widget for Stack<C> {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        let child_limits = Constraints::loose(limits.max);
        let mut max = Size::ZERO;
        for i in 0..self.children.len() {
            let s = self.children.measure_at(i, child_limits, theme);
            max = Size::new(max.w.max(s.w), max.h.max(s.h));
        }
        limits.clamp(max)
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let avail = Size::from_eg(rect.size);
        let child_limits = Constraints::loose(avail);
        for i in 0..self.children.len() {
            let child = match self.align {
                // Stretch: fill the stack's rect on both axes.
                Align::Stretch => avail,
                _ => self.children.measure_at(i, child_limits, theme),
            };
            let offset = embedded_graphics::geometry::Point::new(
                axis_offset(self.align, avail.w, child.w),
                axis_offset(self.align, avail.h, child.h),
            );
            let child_rect = Rectangle::new(rect.top_left + offset, child.to_eg());
            self.children.draw_at(i, target, child_rect, theme)?;
        }
        Ok(())
    }
}
