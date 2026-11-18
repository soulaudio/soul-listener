//! [`List`] — a vertical list of homogeneous rows with a selected-row highlight.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::Drawable;

use crate::theme::{Role, Space, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};

/// A vertical list of homogeneous rows (`&[W]`) — the browse primitive. One row
/// may be marked selected via [`List::selected`]; it is filled with the
/// `highlight` role and its widget redraws with [`Theme::inverted`], so a row of
/// [`Role::Ink`] text reads as Paper-on-Ink (the e-ink selection look). Rows are
/// separated by an optional [`List::gap`] and span the full available width.
///
/// The list draws exactly the rows it is given, top to bottom: the caller does
/// any windowing/scrolling by passing the visible slice and the selected index
/// relative to it.
///
/// # Examples
///
/// ```
/// use eink_ui::{List, Space, Text};
///
/// let tracks = [Text::new("01  Intro"), Text::new("02  Verse"), Text::new("03  Outro")];
/// let browse = List::new(&tracks).selected(Some(1)).gap(Space::Xs);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct List<'a, W> {
    items: &'a [W],
    selected: Option<usize>,
    gap: Option<Space>,
    highlight: Role,
}

impl<'a, W: Widget> List<'a, W> {
    /// A list over `items`, nothing selected, no gap, an [`Role::Ink`] highlight.
    #[must_use]
    pub const fn new(items: &'a [W]) -> Self {
        List {
            items,
            selected: None,
            gap: None,
            highlight: Role::Ink,
        }
    }

    /// Set the selected row index (relative to the given slice), or `None`.
    #[must_use]
    pub const fn selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Set the vertical gap between rows.
    #[must_use]
    pub const fn gap(mut self, gap: Space) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Set the fill color role of the selected row.
    #[must_use]
    pub const fn highlight(mut self, highlight: Role) -> Self {
        self.highlight = highlight;
        self
    }

    fn gap_px(&self, theme: &Theme) -> u32 {
        self.gap.map_or(0, |s| theme.space(s))
    }
}

impl<'a, W: Widget> Widget for List<'a, W> {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        let n = self.items.len();
        if n == 0 {
            return limits.clamp(Size::ZERO);
        }
        let child_limits = Constraints::loose(limits.max);
        let mut height: u32 = 0;
        for item in self.items {
            height = height.saturating_add(item.measure(child_limits, theme).h);
        }
        let gaps = self
            .gap_px(theme)
            .saturating_mul((n as u32).saturating_sub(1));
        // Rows span the full available width so a highlight fills the row.
        limits.clamp(Size::new(limits.max.w, height.saturating_add(gaps)))
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let width = rect.size.width;
        let gap = self.gap_px(theme);
        let child_limits = Constraints::loose(Size::new(width, u32::MAX));
        let inverted = theme.inverted();

        let mut y = rect.top_left.y;
        for (i, item) in self.items.iter().enumerate() {
            let row_h = item.measure(child_limits, theme).h;
            let row_rect = Rectangle::new(
                Point::new(rect.top_left.x, y),
                Size::new(width, row_h).to_eg(),
            );

            if self.selected == Some(i) {
                row_rect
                    .into_styled(
                        PrimitiveStyleBuilder::new()
                            .fill_color(theme.color(self.highlight))
                            .build(),
                    )
                    .draw(target)?;
                item.draw(target, row_rect, &inverted)?;
            } else {
                item.draw(target, row_rect, theme)?;
            }

            // Non-negative row height + gap; the cast never wraps for a
            // display-sized list.
            #[allow(clippy::cast_possible_wrap)]
            let advance = row_h.saturating_add(gap) as i32;
            y = y.saturating_add(advance);
        }
        Ok(())
    }
}
