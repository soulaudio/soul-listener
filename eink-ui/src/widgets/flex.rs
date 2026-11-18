//! [`Flex`] — the flexbox container — with [`row`] / [`col`] constructors.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;

use crate::layout::{draw_flex, measure_flex, Align, Axis, Children, Justify};
use crate::theme::{Space, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};

/// A flexbox container: lays its [`Children`] out along a main [`Axis`],
/// distributing leftover space by [`Justify`] (or to flexible children), and
/// placing each child on the cross axis by [`Align`]. Children may be a tuple of
/// differing widget types (`(a, b, c)`) or a `&[W]` slice — no heap, no boxing.
///
/// Prefer the [`row`] / [`col`] constructors; chain [`Flex::gap`],
/// [`Flex::padding`], [`Flex::justify`], [`Flex::align`] to configure it.
///
/// # Examples
///
/// ```
/// use eink_ui::{col, row, Role, Space, Spacer, Text, TextScale};
/// use eink_ui::layout::Align;
///
/// // A header row: title on the left, elapsed time pushed to the right.
/// let header = row((
///     Text::new("Now Playing").scale(TextScale::Title),
///     Spacer::new(),
///     Text::new("1:23").role(Role::Muted),
/// ))
/// .align(Align::Center);
///
/// // A centered two-line stack.
/// let body = col((Text::new("Song Title"), Text::new("Artist")))
///     .gap(Space::Xs)
///     .align(Align::Center);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Flex<C> {
    children: C,
    axis: Axis,
    justify: Justify,
    align: Align,
    gap: Option<Space>,
    padding: Option<Space>,
}

impl<C: Children> Flex<C> {
    /// A flex container over `children` along `axis`, defaulting to
    /// [`Justify::Start`] / [`Align::Start`] with no gap or padding.
    #[must_use]
    pub const fn new(axis: Axis, children: C) -> Self {
        Flex {
            children,
            axis,
            justify: Justify::Start,
            align: Align::Start,
            gap: None,
            padding: None,
        }
    }

    /// Set the main-axis distribution of leftover space.
    #[must_use]
    pub const fn justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }

    /// Set the cross-axis placement of each child.
    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Set the spacing inserted *between* adjacent children.
    #[must_use]
    pub const fn gap(mut self, gap: Space) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Set uniform padding between the container's edge and its children.
    #[must_use]
    pub const fn padding(mut self, padding: Space) -> Self {
        self.padding = Some(padding);
        self
    }

    fn gap_px(&self, theme: &Theme) -> u32 {
        self.gap.map_or(0, |s| theme.space(s))
    }

    fn pad_px(&self, theme: &Theme) -> u32 {
        self.padding.map_or(0, |s| theme.space(s))
    }
}

impl<C: Children> Widget for Flex<C> {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        let pad = self.pad_px(theme);
        let inset = pad.saturating_mul(2);
        let inner = Constraints {
            min: Size::new(
                limits.min.w.saturating_sub(inset),
                limits.min.h.saturating_sub(inset),
            ),
            max: Size::new(
                limits.max.w.saturating_sub(inset),
                limits.max.h.saturating_sub(inset),
            ),
        };

        let content = measure_flex(&self.children, self.axis, self.gap_px(theme), inner, theme);
        let total = Size::new(
            content.w.saturating_add(inset),
            content.h.saturating_add(inset),
        );
        limits.clamp(total)
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let pad = self.pad_px(theme);
        let inset = pad.saturating_mul(2);
        // `pad` is a small token value (single/double-digit px), so this narrowing
        // cast never truncates in practice.
        #[allow(clippy::cast_possible_wrap)]
        let pad_i32 = pad as i32;
        let inner = Rectangle::new(
            rect.top_left + Point::new(pad_i32, pad_i32),
            Size::new(
                rect.size.width.saturating_sub(inset),
                rect.size.height.saturating_sub(inset),
            )
            .to_eg(),
        );

        draw_flex(
            &self.children,
            self.axis,
            self.justify,
            self.align,
            self.gap_px(theme),
            inner,
            theme,
            target,
        )
    }
}

/// A horizontal flex container (children flow left-to-right). See [`Flex`].
#[must_use]
pub const fn row<C: Children>(children: C) -> Flex<C> {
    Flex::new(Axis::Horizontal, children)
}

/// A vertical flex container (children flow top-to-bottom). See [`Flex`].
#[must_use]
pub const fn col<C: Children>(children: C) -> Flex<C> {
    Flex::new(Axis::Vertical, children)
}
