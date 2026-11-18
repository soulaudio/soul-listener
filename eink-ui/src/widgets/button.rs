//! [`Button`] — a labelled, bordered control with a pressed (inverted) state.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::Drawable;

use crate::theme::{Role, Space, TextScale, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};
use crate::widgets::Text;

/// 1-pixel border, present on each of the four edges, so the content inset is
/// `BORDER` on each side (`2 * BORDER` total per axis).
const BORDER: u32 = 1;

/// A bordered control wrapping a centered text label, with a **pressed** state
/// that inverts its colors (the e-ink "selected" look): unpressed draws
/// [`Role::Ink`] text on a [`Role::Paper`] fill; pressed swaps them, giving a
/// filled dark button with light text. Always carries a 1px [`Role::Ink`]
/// border.
///
/// Borrows its label (`&str`), so it works in the no-alloc core. Size is the
/// label's measured size plus `padding` and the border on all sides.
///
/// # Examples
///
/// ```
/// use eink_ui::{Button, Space};
///
/// let play = Button::new("Play").padding(Space::Md);
/// let selected = Button::new("Pause").pressed(true);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Button<'a> {
    label: &'a str,
    pressed: bool,
    scale: TextScale,
    padding: Space,
}

impl<'a> Button<'a> {
    /// A button labelled `label`, unpressed, [`TextScale::Body`] /
    /// [`Space::Sm`] padding.
    #[must_use]
    pub const fn new(label: &'a str) -> Self {
        Button {
            label,
            pressed: false,
            scale: TextScale::Body,
            padding: Space::Sm,
        }
    }

    /// Set the pressed/selected state (inverts the colors when `true`).
    #[must_use]
    pub const fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = pressed;
        self
    }

    /// Set the label's typography scale.
    #[must_use]
    pub const fn scale(mut self, scale: TextScale) -> Self {
        self.scale = scale;
        self
    }

    /// Set the padding between the border and the label.
    #[must_use]
    pub const fn padding(mut self, padding: Space) -> Self {
        self.padding = padding;
        self
    }

    /// Total inset per side: padding + the 1px border.
    fn inset(&self, theme: &Theme) -> u32 {
        theme.space(self.padding).saturating_add(BORDER)
    }

    /// `(background, foreground)` roles for the current pressed state.
    const fn colors(&self) -> (Role, Role) {
        if self.pressed {
            (Role::Ink, Role::Paper)
        } else {
            (Role::Paper, Role::Ink)
        }
    }

    fn label_widget(&self, fg: Role) -> Text<'a> {
        Text::new(self.label).scale(self.scale).role(fg)
    }
}

impl<'a> Widget for Button<'a> {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        let inset = self.inset(theme);
        let span = inset.saturating_mul(2);
        let inner = Constraints::loose(Size::new(
            limits.max.w.saturating_sub(span),
            limits.max.h.saturating_sub(span),
        ));
        let (_, fg) = self.colors();
        let text = self.label_widget(fg).measure(inner, theme);
        limits.clamp(Size::new(
            text.w.saturating_add(span),
            text.h.saturating_add(span),
        ))
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let (bg, fg) = self.colors();

        rect.into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(theme.color(bg))
                .stroke_color(theme.color(Role::Ink))
                .stroke_width(BORDER)
                .build(),
        )
        .draw(target)?;

        // Center the label within the whole button rect.
        let label = self.label_widget(fg);
        let inner = Constraints::loose(Size::from_eg(rect.size));
        let tsize = label.measure(inner, theme);
        let avail = Size::from_eg(rect.size);
        // Non-negative slack / 2; the cast never wraps for display-sized rects.
        #[allow(clippy::cast_possible_wrap)]
        let offset = Point::new(
            (avail.w.saturating_sub(tsize.w) / 2) as i32,
            (avail.h.saturating_sub(tsize.h) / 2) as i32,
        );
        let label_rect = Rectangle::new(rect.top_left + offset, tsize.to_eg());
        label.draw(target, label_rect, theme)
    }
}
