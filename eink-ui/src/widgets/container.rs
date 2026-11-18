//! [`Container`] — padding, background, and border around a single child.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::Drawable;

use crate::theme::{Role, Space, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};

/// Wraps a single child widget with padding and an optional flat
/// background / 1px border, both drawn in a [`Role`] color.
///
/// Generic over the child's [`Widget`] type (`Container<T>`) — composition
/// is monomorphized, not boxed, so this works unchanged in the no-alloc
/// build.
///
/// # Examples
///
/// ```
/// use eink_ui::{Container, Role, Space, Text};
///
/// let boxed = Container::new(Text::new("hello"))
///     .padding(Space::Md)
///     .background(Some(Role::Paper))
///     .border(Some(Role::Ink));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Container<T> {
    child: T,
    padding: Space,
    background: Option<Role>,
    border: Option<Role>,
}

impl<T: Widget> Container<T> {
    /// Wrap `child`, defaulting to [`Space::Md`] padding and no
    /// background/border.
    #[must_use]
    pub const fn new(child: T) -> Self {
        Container {
            child,
            padding: Space::Md,
            background: None,
            border: None,
        }
    }

    /// Set the padding between the container's edge and its child.
    #[must_use]
    pub const fn padding(mut self, padding: Space) -> Self {
        self.padding = padding;
        self
    }

    /// Set the flat background fill color, or `None` for no fill.
    #[must_use]
    pub const fn background(mut self, background: Option<Role>) -> Self {
        self.background = background;
        self
    }

    /// Set the 1px border color, or `None` for no border.
    #[must_use]
    pub const fn border(mut self, border: Option<Role>) -> Self {
        self.border = border;
        self
    }

    /// The padding in pixels for a given outer [`Constraints`]' theme.
    fn pad_px(&self, theme: &Theme) -> u32 {
        theme.space(self.padding)
    }
}

impl<T: Widget> Widget for Container<T> {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        let pad = self.pad_px(theme);
        let inset = pad.saturating_mul(2);

        let inner_limits = Constraints {
            min: Size::new(
                limits.min.w.saturating_sub(inset),
                limits.min.h.saturating_sub(inset),
            ),
            max: Size::new(
                limits.max.w.saturating_sub(inset),
                limits.max.h.saturating_sub(inset),
            ),
        };

        let child_size = self.child.measure(inner_limits, theme);
        let total = Size::new(
            child_size.w.saturating_add(inset),
            child_size.h.saturating_add(inset),
        );

        limits.clamp(total)
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        if let Some(role) = self.background {
            let style = PrimitiveStyleBuilder::new()
                .fill_color(theme.color(role))
                .build();
            rect.into_styled(style).draw(target)?;
        }
        if let Some(role) = self.border {
            let style = PrimitiveStyleBuilder::new()
                .stroke_color(theme.color(role))
                .stroke_width(1)
                .build();
            rect.into_styled(style).draw(target)?;
        }

        let pad = self.pad_px(theme);
        let inset = pad.saturating_mul(2);
        // `pad` is a small token value (single/double-digit px), so this
        // narrowing cast never truncates in practice.
        #[allow(clippy::cast_possible_wrap)]
        let pad_i32 = pad as i32;
        let inner_rect = Rectangle::new(
            rect.top_left + Point::new(pad_i32, pad_i32),
            Size::new(
                rect.size.width.saturating_sub(inset),
                rect.size.height.saturating_sub(inset),
            )
            .to_eg(),
        );

        self.child.draw(target, inner_rect, theme)
    }
}
