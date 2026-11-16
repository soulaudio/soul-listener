//! The [`Widget`] trait: the entire contract of the toolkit.
//!
//! Each frame, a container calls [`Widget::measure`] on its children (under
//! a [`Constraints`] box), runs its layout algorithm, then calls
//! [`Widget::draw`] on each child with its computed [`Rectangle`]. Leaves
//! (e.g. [`crate::widgets::Text`]) measure their intrinsic size and draw
//! directly. There is no retained widget tree: widgets are lightweight
//! values constructed fresh each frame.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;

use crate::theme::Theme;

/// A 2D size in pixels.
///
/// Distinct from `embedded_graphics::geometry::Size` (which uses
/// `width`/`height`) to keep this crate's public API self-contained; use
/// [`Size::to_eg`] / [`Size::from_eg`] to convert at the `embedded-graphics`
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Size {
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl Size {
    /// The zero size (no area).
    pub const ZERO: Size = Size { w: 0, h: 0 };

    /// Construct a `Size` from a width and height in pixels.
    #[must_use]
    pub const fn new(w: u32, h: u32) -> Self {
        Size { w, h }
    }

    /// Convert to an `embedded_graphics::geometry::Size`.
    #[must_use]
    pub const fn to_eg(self) -> embedded_graphics::geometry::Size {
        embedded_graphics::geometry::Size::new(self.w, self.h)
    }

    /// Convert from an `embedded_graphics::geometry::Size`.
    #[must_use]
    pub const fn from_eg(size: embedded_graphics::geometry::Size) -> Self {
        Size {
            w: size.width,
            h: size.height,
        }
    }
}

/// A box of minimum/maximum [`Size`] a widget may be measured and laid out
/// within, analogous to CSS's min/max constraints.
///
/// A container clamps its own computed size to its *own* constraints, and
/// passes a (usually tighter) set of constraints down to each child.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Constraints {
    /// The smallest acceptable size.
    pub min: Size,
    /// The largest acceptable size.
    pub max: Size,
}

impl Constraints {
    /// Constraints that force an exact size (`min == max == size`).
    #[must_use]
    pub const fn tight(size: Size) -> Self {
        Constraints {
            min: size,
            max: size,
        }
    }

    /// Constraints with no minimum, up to `max`.
    #[must_use]
    pub const fn loose(max: Size) -> Self {
        Constraints {
            min: Size::ZERO,
            max,
        }
    }

    /// Clamp a candidate size into `[min, max]` on both axes.
    #[must_use]
    pub fn clamp(&self, size: Size) -> Size {
        Size {
            w: size.w.clamp(self.min.w, self.max.w.max(self.min.w)),
            h: size.h.clamp(self.min.h, self.max.h.max(self.min.h)),
        }
    }
}

/// The error type returned by [`Widget::draw`]: wraps whatever error the
/// underlying `D: DrawTarget` produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawError<E> {
    /// The `DrawTarget` itself failed (e.g. an SPI/bus error on real
    /// hardware; infallible on in-memory targets).
    Target(E),
}

impl<E> From<E> for DrawError<E> {
    fn from(err: E) -> Self {
        DrawError::Target(err)
    }
}

/// The whole contract of the toolkit: a widget can report how big it wants
/// to be, and can draw itself into a given rectangle.
///
/// Implementors are plain values (no identity, no persistent state) held by
/// value or by reference — composition is generic (`Container<T: Widget>`),
/// never `dyn`/`Box`, on the no-alloc path.
pub trait Widget {
    /// Compute this widget's size given the available [`Constraints`].
    ///
    /// Implementations should return their *intrinsic* size clamped into
    /// `limits` (see [`Constraints::clamp`]) — e.g. a line of text returns
    /// its natural glyph-advance width and font height, clamped.
    fn measure(&self, limits: Constraints, theme: &Theme) -> Size;

    /// Draw this widget into `rect` of the target.
    ///
    /// `rect` is normally the exact size [`Widget::measure`] returned for
    /// the same constraints (a container is responsible for that), but
    /// implementations should not panic if given a different size —
    /// clipping to `rect` is acceptable.
    fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>>;
}
