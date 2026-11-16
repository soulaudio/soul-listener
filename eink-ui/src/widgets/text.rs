//! [`Text`] — a single line of styled text, the toolkit's basic leaf widget.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Baseline, Text as EgText};
use embedded_graphics::Drawable;

use crate::theme::{Role, TextScale, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};

/// A single line of text rendered with a [`TextScale`] font at a [`Role`]
/// color.
///
/// `Text` borrows its content (`&'a str`) and carries no allocation, so it
/// is usable in the default no-alloc build. Construct with [`Text::new`]
/// and chain [`Text::scale`] / [`Text::role`] to style it; both default to
/// [`TextScale::Body`] / [`Role::Ink`].
///
/// # Examples
///
/// ```
/// use eink_ui::{Role, Text, TextScale};
///
/// let label = Text::new("Now Playing").scale(TextScale::Title).role(Role::Ink);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Text<'a> {
    content: &'a str,
    scale: TextScale,
    role: Role,
}

impl<'a> Text<'a> {
    /// Create a `Text` widget over `content`, defaulting to
    /// [`TextScale::Body`] / [`Role::Ink`].
    #[must_use]
    pub const fn new(content: &'a str) -> Self {
        Text {
            content,
            scale: TextScale::Body,
            role: Role::Ink,
        }
    }

    /// Set the typography scale.
    #[must_use]
    pub const fn scale(mut self, scale: TextScale) -> Self {
        self.scale = scale;
        self
    }

    /// Set the color role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

impl<'a> Widget for Text<'a> {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        let font = theme.font(self.scale);
        let char_count = u32::try_from(self.content.chars().count()).unwrap_or(u32::MAX);

        let natural_w = if char_count == 0 {
            0
        } else {
            let glyphs = font.character_size.width.saturating_mul(char_count);
            let gaps = font
                .character_spacing
                .saturating_mul(char_count.saturating_sub(1));
            glyphs.saturating_add(gaps)
        };
        let natural_h = font.character_size.height;

        limits.clamp(Size::new(natural_w, natural_h))
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let font = theme.font(self.scale);
        let color = theme.color(self.role);
        let char_style = MonoTextStyleBuilder::new()
            .font(&font)
            .text_color(color)
            .build();

        EgText::with_baseline(self.content, rect.top_left, char_style, Baseline::Top)
            .draw(target)?;
        Ok(())
    }
}
