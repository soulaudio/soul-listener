//! The e-ink specialization: **dirty-region tracking** and **refresh hints**.
//!
//! E-ink's defining performance feature is the *partial refresh* — updating only
//! the rectangle that changed, rather than flashing the whole panel. This module
//! gives a screen the two things it needs to drive one:
//!
//! - [`DirtyTracker`] — a [`DrawTarget`] wrapper that records the bounding box of
//!   every pixel drawn through it. Render a frame through it and read back the
//!   [`DirtyRegion`]: the one rectangle the driver must push to the panel.
//! - [`RefreshKind`] / [`RefreshPolicy`] — whether to do a cheap `Partial` update
//!   or a `Full` ghost-clearing flash, and a simple policy that forces a periodic
//!   `Full` to keep e-ink ghosting in check.
//!
//! v1 is region-granular (the bounding box of what was drawn); a per-pixel diff
//! against the previous frame is a later refinement that keeps the same API.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Dimensions, Point};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Pixel;

/// An accumulating axis-aligned bounding box of drawn pixels — the "dirty"
/// region of a frame. Starts empty; [`DirtyRegion::include`] grows it to cover
/// each point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DirtyRegion {
    /// `(min_x, min_y, max_x, max_y)` inclusive, or `None` while empty.
    bounds: Option<(i32, i32, i32, i32)>,
}

impl DirtyRegion {
    /// An empty region (nothing drawn yet).
    #[must_use]
    pub const fn new() -> Self {
        DirtyRegion { bounds: None }
    }

    /// Whether nothing has been included yet.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bounds.is_none()
    }

    /// Grow the region to include `p`.
    pub fn include(&mut self, p: Point) {
        self.bounds = Some(match self.bounds {
            None => (p.x, p.y, p.x, p.y),
            Some((min_x, min_y, max_x, max_y)) => (
                min_x.min(p.x),
                min_y.min(p.y),
                max_x.max(p.x),
                max_y.max(p.y),
            ),
        });
    }

    /// Grow the region to include all of `rect` (its corners).
    pub fn include_rect(&mut self, rect: Rectangle) {
        if rect.size.width == 0 || rect.size.height == 0 {
            return;
        }
        let br = rect.top_left
            + Point::new(
                i32::try_from(rect.size.width.saturating_sub(1)).unwrap_or(i32::MAX),
                i32::try_from(rect.size.height.saturating_sub(1)).unwrap_or(i32::MAX),
            );
        self.include(rect.top_left);
        self.include(br);
    }

    /// Clear back to empty (reuse the accumulator for the next frame).
    pub fn reset(&mut self) {
        self.bounds = None;
    }

    /// The dirty region as a [`Rectangle`], or `None` if empty. The rectangle is
    /// inclusive of both corners (so a single drawn pixel yields a 1x1 rect).
    #[must_use]
    pub fn as_rectangle(&self) -> Option<Rectangle> {
        self.bounds.map(|(min_x, min_y, max_x, max_y)| {
            // max >= min by construction, so the +1 extents are >= 1.
            let w = max_x.saturating_sub(min_x).saturating_add(1);
            let h = max_y.saturating_sub(min_y).saturating_add(1);
            Rectangle::new(
                Point::new(min_x, min_y),
                embedded_graphics::geometry::Size::new(
                    u32::try_from(w).unwrap_or(0),
                    u32::try_from(h).unwrap_or(0),
                ),
            )
        })
    }
}

/// A [`DrawTarget`] wrapper that forwards every draw to an inner target while
/// recording the bounding box of the pixels touched. Render a frame through it,
/// then read [`DirtyTracker::dirty`] / [`DirtyTracker::into_dirty`] for the one
/// rectangle a partial refresh must cover.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "std")] {
/// use eink_ui::refresh::DirtyTracker;
/// use eink_ui::testing::Framebuffer;
/// use eink_ui::{Text, Theme, Widget, Size, Constraints};
/// use embedded_graphics::primitives::Rectangle;
/// use embedded_graphics::geometry::Point;
///
/// let mut fb = Framebuffer::new(128, 64);
/// let theme = Theme::default();
/// let mut tracker = DirtyTracker::new(&mut fb);
/// let text = Text::new("Hi");
/// let at = Rectangle::new(Point::new(10, 20), Size::new(40, 10).to_eg());
/// text.draw(&mut tracker, at, &theme).unwrap();
///
/// // The dirty region is contained within where we drew.
/// let dirty = tracker.into_dirty().as_rectangle().unwrap();
/// assert!(dirty.top_left.x >= 10 && dirty.top_left.y >= 20);
/// # }
/// ```
#[derive(Debug)]
pub struct DirtyTracker<'a, D> {
    inner: &'a mut D,
    dirty: DirtyRegion,
}

impl<'a, D> DirtyTracker<'a, D> {
    /// Wrap `inner`, starting with an empty dirty region.
    pub fn new(inner: &'a mut D) -> Self {
        DirtyTracker {
            inner,
            dirty: DirtyRegion::new(),
        }
    }

    /// The accumulated dirty region so far.
    #[must_use]
    pub const fn dirty(&self) -> &DirtyRegion {
        &self.dirty
    }

    /// Consume the tracker, returning the accumulated dirty region.
    #[must_use]
    pub fn into_dirty(self) -> DirtyRegion {
        self.dirty
    }
}

impl<D: Dimensions> Dimensions for DirtyTracker<'_, D> {
    fn bounding_box(&self) -> Rectangle {
        self.inner.bounding_box()
    }
}

impl<D: DrawTarget> DrawTarget for DirtyTracker<'_, D> {
    type Color = D::Color;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // Disjoint borrows of the two fields: `dirty` records each point as the
        // inner target consumes the (forwarded) iterator.
        let dirty = &mut self.dirty;
        self.inner
            .draw_iter(pixels.into_iter().inspect(|Pixel(p, _)| dirty.include(*p)))
    }
}

/// Whether a frame needs a cheap partial update or a full ghost-clearing flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefreshKind {
    /// A full-panel refresh: slower and flashes, but clears accumulated e-ink
    /// ghosting. Use on the first frame, on large changes, and periodically.
    Full,
    /// A partial refresh of just the dirty region: fast and flicker-free, but
    /// leaves faint ghosting that a periodic [`RefreshKind::Full`] clears.
    Partial,
}

/// A tiny policy that forces a periodic [`RefreshKind::Full`] to keep e-ink
/// ghosting in check, returning [`RefreshKind::Partial`] in between.
///
/// Call [`RefreshPolicy::advance`] once per rendered frame. The first frame, and
/// every `full_interval`-th frame after, is [`RefreshKind::Full`]; a
/// `full_interval` of `0` means *always* full.
#[derive(Debug, Clone, Copy)]
pub struct RefreshPolicy {
    full_interval: u32,
    since_full: u32,
}

impl RefreshPolicy {
    /// A policy that does a full refresh every `full_interval` frames (and on the
    /// first frame). `0` forces every frame to be full.
    #[must_use]
    pub const fn every(full_interval: u32) -> Self {
        RefreshPolicy {
            full_interval,
            // Start "due" so the first frame is Full.
            since_full: u32::MAX,
        }
    }

    /// Advance one frame and return the refresh kind to use for it.
    pub fn advance(&mut self) -> RefreshKind {
        if self.full_interval == 0 || self.since_full >= self.full_interval.saturating_sub(1) {
            self.since_full = 0;
            RefreshKind::Full
        } else {
            self.since_full = self.since_full.saturating_add(1);
            RefreshKind::Partial
        }
    }
}
