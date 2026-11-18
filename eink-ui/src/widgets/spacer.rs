//! [`Spacer`] — a flexible gap that pushes flex siblings apart.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;

use crate::theme::Theme;
use crate::widget::{Constraints, DrawError, Size, Widget};

/// A zero-intrinsic-size widget that absorbs a flex container's leftover
/// main-axis space via its [`Widget::flex`] factor — the idiomatic way to push
/// siblings to opposite ends (like SwiftUI's `Spacer`).
///
/// `Spacer::new()` has grow `1`; two of them split the slack evenly, so
/// `row((a, Spacer::new(), b, Spacer::new(), c))` spreads `a`/`b`/`c` with equal
/// gaps. [`Spacer::grow`] weights it. Outside a flex container (or when every
/// child is a `Spacer`) it contributes nothing and draws nothing.
#[derive(Debug, Clone, Copy)]
pub struct Spacer {
    grow: u32,
}

impl Spacer {
    /// A spacer with grow factor `1`.
    #[must_use]
    pub const fn new() -> Self {
        Spacer { grow: 1 }
    }

    /// A spacer with an explicit grow factor (a `grow(2)` spacer absorbs twice
    /// the slack of a `grow(1)` one).
    #[must_use]
    pub const fn grow(grow: u32) -> Self {
        Spacer { grow }
    }
}

impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Spacer {
    fn measure(&self, limits: Constraints, _theme: &Theme) -> Size {
        // Zero intrinsic size (clamped up to any imposed minimum); the flex
        // container grows it at draw time.
        limits.clamp(Size::ZERO)
    }

    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        _target: &mut D,
        _rect: Rectangle,
        _theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        Ok(())
    }

    fn flex(&self) -> u32 {
        self.grow
    }
}
