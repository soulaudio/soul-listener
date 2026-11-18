//! [`ProgressBar`] — a horizontal fill bar for a fraction (volume, elapsed time).

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::Drawable;

use crate::theme::{Role, Space, Theme};
use crate::widget::{Constraints, DrawError, Size, Widget};

/// A horizontal bar that fills `filled / total` of its width — the DAP's volume
/// slider and track-position indicator. Integer-only (no floats): the fill width
/// is `width * filled / total`, computed in `u64` to avoid overflow.
///
/// Fills the available width and is `thickness` pixels tall (default
/// [`Space::Sm`]); the unfilled track draws in [`Role::Faint`] and the fill in
/// [`Role::Ink`] by default. `filled` is clamped to `total`; a `total` of `0`
/// draws an empty track.
///
/// # Examples
///
/// ```
/// use eink_ui::{ProgressBar, Role, Space};
///
/// // 80% volume, a medium-thick bar.
/// let volume = ProgressBar::new(80, 100).thickness(Space::Md);
///
/// // 1:23 of a 3:00 track (seconds), de-emphasised track.
/// let elapsed = ProgressBar::new(83, 180).fill(Role::Muted);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ProgressBar {
    filled: u32,
    total: u32,
    thickness: Space,
    track: Role,
    fill: Role,
}

impl ProgressBar {
    /// A bar showing `filled / total` of its width.
    #[must_use]
    pub const fn new(filled: u32, total: u32) -> Self {
        ProgressBar {
            filled,
            total,
            thickness: Space::Sm,
            track: Role::Faint,
            fill: Role::Ink,
        }
    }

    /// Set the bar thickness (its fixed cross-axis height).
    #[must_use]
    pub const fn thickness(mut self, thickness: Space) -> Self {
        self.thickness = thickness;
        self
    }

    /// Set the unfilled-track color role.
    #[must_use]
    pub const fn track(mut self, track: Role) -> Self {
        self.track = track;
        self
    }

    /// Set the filled-portion color role.
    #[must_use]
    pub const fn fill(mut self, fill: Role) -> Self {
        self.fill = fill;
        self
    }

    /// The filled width in pixels for a track `width` px wide.
    fn fill_width(&self, width: u32) -> u32 {
        if self.total == 0 {
            return 0;
        }
        let clamped = self.filled.min(self.total);
        let w = u64::from(width)
            .saturating_mul(u64::from(clamped))
            .checked_div(u64::from(self.total))
            .unwrap_or(0);
        u32::try_from(w).unwrap_or(width).min(width)
    }
}

impl Widget for ProgressBar {
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size {
        // Spans the available width; fixed thickness tall.
        limits.clamp(Size::new(limits.max.w, theme.space(self.thickness)))
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        let track_style = PrimitiveStyleBuilder::new()
            .fill_color(theme.color(self.track))
            .build();
        rect.into_styled(track_style).draw(target)?;

        let fw = self.fill_width(rect.size.width);
        if fw > 0 {
            let fill_style = PrimitiveStyleBuilder::new()
                .fill_color(theme.color(self.fill))
                .build();
            Rectangle::new(rect.top_left, Size::new(fw, rect.size.height).to_eg())
                .into_styled(fill_style)
                .draw(target)?;
        }
        Ok(())
    }
}
