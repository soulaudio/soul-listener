//! Partial Update Window Management
//!
//! Handles dirty region tracking and alignment for e-ink partial updates.
//! E-ink controllers require partial update regions to be aligned to 8-pixel
//! boundaries for correct addressing.

use crate::alignment::{align_rectangle, validate_and_align};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// A partial update window with alignment information
///
/// Represents a rectangular region to be updated on the display.
/// The region is automatically aligned to 8-pixel boundaries as
/// required by e-ink controllers.
///
/// # Examples
/// ```
/// use eink_emulator::partial_window::PartialWindow;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::primitives::Rectangle;
///
/// let rect = Rectangle::new(Point::new(5, 5), Size::new(10, 10));
/// let mut window = PartialWindow::new(rect);
///
/// // Window is automatically aligned
/// assert!(window.was_aligned);
/// assert_eq!(window.aligned_rect.top_left, Point::new(0, 0));
/// assert_eq!(window.aligned_rect.size, Size::new(16, 16));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PartialWindow {
    /// Original rectangle before alignment
    pub rect: Rectangle,
    /// Rectangle after 8-pixel alignment
    pub aligned_rect: Rectangle,
    /// Whether alignment was necessary
    pub was_aligned: bool,
}

impl PartialWindow {
    /// Create a new partial window from a rectangle
    ///
    /// Automatically aligns the rectangle to 8-pixel boundaries.
    ///
    /// # Examples
    /// ```
    /// use eink_emulator::partial_window::PartialWindow;
    /// use embedded_graphics::prelude::*;
    /// use embedded_graphics::primitives::Rectangle;
    ///
    /// let rect = Rectangle::new(Point::new(10, 10), Size::new(20, 20));
    /// let window = PartialWindow::new(rect);
    ///
    /// assert_eq!(window.rect, rect);
    /// assert_eq!(window.aligned_rect.top_left, Point::new(8, 8));
    /// ```
    pub fn new(rect: Rectangle) -> Self {
        let (aligned_rect, was_aligned) = validate_and_align(&rect);
        Self {
            rect,
            aligned_rect,
            was_aligned,
        }
    }

    /// Manually trigger re-alignment
    ///
    /// Re-aligns the current rectangle. This is useful if the rectangle
    /// has been modified after creation.
    pub fn align(&mut self) {
        let (aligned_rect, was_aligned) = validate_and_align(&self.rect);
        self.aligned_rect = aligned_rect;
        self.was_aligned = was_aligned;
    }

    /// Check if the window contains a point
    pub fn contains(&self, point: Point) -> bool {
        self.aligned_rect.contains(point)
    }

    /// Get the area of the aligned window in pixels
    pub fn area(&self) -> u32 {
        self.aligned_rect.size.width * self.aligned_rect.size.height
    }

    /// Merge with another window to create a bounding box
    ///
    /// Returns a new PartialWindow that encompasses both windows.
    pub fn merge(&self, other: &PartialWindow) -> PartialWindow {
        // Compute bounding box manually
        let top_left = Point::new(
            self.aligned_rect
                .top_left
                .x
                .min(other.aligned_rect.top_left.x),
            self.aligned_rect
                .top_left
                .y
                .min(other.aligned_rect.top_left.y),
        );
        let bottom_right = Point::new(
            (self.aligned_rect.top_left.x + self.aligned_rect.size.width as i32)
                .max(other.aligned_rect.top_left.x + other.aligned_rect.size.width as i32),
            (self.aligned_rect.top_left.y + self.aligned_rect.size.height as i32)
                .max(other.aligned_rect.top_left.y + other.aligned_rect.size.height as i32),
        );
        let size = Size::new(
            (bottom_right.x - top_left.x) as u32,
            (bottom_right.y - top_left.y) as u32,
        );
        let merged_rect = Rectangle::new(top_left, size);
        PartialWindow::new(merged_rect)
    }
}

/// Merge multiple rectangles into a single bounding box
///
/// Returns the smallest aligned rectangle that contains all input rectangles.
/// Returns None if the input is empty.
///
/// # Examples
/// ```
/// use eink_emulator::partial_window::merge_rectangles;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::primitives::Rectangle;
///
/// let rects = vec![
///     Rectangle::new(Point::new(5, 5), Size::new(10, 10)),
///     Rectangle::new(Point::new(20, 20), Size::new(15, 15)),
/// ];
///
/// let merged = merge_rectangles(&rects).unwrap();
/// assert_eq!(merged.top_left, Point::new(0, 0));
/// assert_eq!(merged.size, Size::new(40, 40));
/// ```
pub fn merge_rectangles(rects: &[Rectangle]) -> Option<Rectangle> {
    if rects.is_empty() {
        return None;
    }

    let mut iter = rects.iter();
    let first = iter.next()?;
    let mut merged = *first;

    for rect in iter {
        // Compute bounding box manually
        let top_left = Point::new(
            merged.top_left.x.min(rect.top_left.x),
            merged.top_left.y.min(rect.top_left.y),
        );
        let bottom_right = Point::new(
            (merged.top_left.x + merged.size.width as i32)
                .max(rect.top_left.x + rect.size.width as i32),
            (merged.top_left.y + merged.size.height as i32)
                .max(rect.top_left.y + rect.size.height as i32),
        );
        let size = Size::new(
            (bottom_right.x - top_left.x) as u32,
            (bottom_right.y - top_left.y) as u32,
        );
        merged = Rectangle::new(top_left, size);
    }

    // Align the final merged rectangle
    Some(align_rectangle(&merged))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partial_window_new_aligned() {
        let rect = Rectangle::new(Point::new(0, 0), Size::new(16, 16));
        let window = PartialWindow::new(rect);

        assert_eq!(window.rect, rect);
        assert_eq!(window.aligned_rect, rect);
        assert!(!window.was_aligned);
    }

    #[test]
    fn test_partial_window_new_needs_alignment() {
        let rect = Rectangle::new(Point::new(5, 5), Size::new(10, 10));
        let window = PartialWindow::new(rect);

        assert_eq!(window.rect, rect);
        assert_ne!(window.aligned_rect, rect);
        assert!(window.was_aligned);
        assert_eq!(window.aligned_rect.top_left, Point::new(0, 0));
        assert_eq!(window.aligned_rect.size, Size::new(16, 16));
    }

    #[test]
    fn test_partial_window_align() {
        let mut window = PartialWindow::new(Rectangle::new(Point::new(0, 0), Size::new(16, 16)));
        assert!(!window.was_aligned);

        // Modify the rect
        window.rect = Rectangle::new(Point::new(5, 5), Size::new(10, 10));

        // Re-align
        window.align();
        assert!(window.was_aligned);
        assert_eq!(window.aligned_rect.top_left, Point::new(0, 0));
        assert_eq!(window.aligned_rect.size, Size::new(16, 16));
    }

    #[test]
    fn test_partial_window_contains() {
        let window = PartialWindow::new(Rectangle::new(Point::new(5, 5), Size::new(10, 10)));

        // Original rect is (5, 5, 10×10) but aligned to (0, 0, 16×16)
        assert!(window.contains(Point::new(0, 0)));
        assert!(window.contains(Point::new(8, 8)));
        assert!(window.contains(Point::new(15, 15)));
        assert!(!window.contains(Point::new(16, 16)));
    }

    #[test]
    fn test_partial_window_area() {
        let window = PartialWindow::new(Rectangle::new(Point::new(5, 5), Size::new(10, 10)));

        // Aligned to (0, 0, 16×16)
        assert_eq!(window.area(), 256);
    }

    #[test]
    fn test_partial_window_merge() {
        let window1 = PartialWindow::new(Rectangle::new(Point::new(0, 0), Size::new(8, 8)));
        let window2 = PartialWindow::new(Rectangle::new(Point::new(16, 16), Size::new(8, 8)));

        let merged = window1.merge(&window2);

        assert_eq!(merged.aligned_rect.top_left, Point::new(0, 0));
        assert_eq!(merged.aligned_rect.size, Size::new(24, 24));
    }

    #[test]
    fn test_merge_rectangles_empty() {
        let result = merge_rectangles(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_rectangles_single() {
        let rects = vec![Rectangle::new(Point::new(5, 5), Size::new(10, 10))];
        let merged = merge_rectangles(&rects).unwrap();

        // Should be aligned to (0, 0, 16×16)
        assert_eq!(merged.top_left, Point::new(0, 0));
        assert_eq!(merged.size, Size::new(16, 16));
    }

    #[test]
    fn test_merge_rectangles_multiple() {
        let rects = vec![
            Rectangle::new(Point::new(5, 5), Size::new(10, 10)),
            Rectangle::new(Point::new(20, 20), Size::new(15, 15)),
        ];

        let merged = merge_rectangles(&rects).unwrap();

        // Bounding box is (5, 5) to (35, 35)
        // Aligned: (0, 0) to (40, 40)
        assert_eq!(merged.top_left, Point::new(0, 0));
        assert_eq!(merged.size, Size::new(40, 40));
    }

    #[test]
    fn test_merge_rectangles_already_aligned() {
        let rects = vec![
            Rectangle::new(Point::new(0, 0), Size::new(16, 16)),
            Rectangle::new(Point::new(16, 16), Size::new(16, 16)),
        ];

        let merged = merge_rectangles(&rects).unwrap();

        assert_eq!(merged.top_left, Point::new(0, 0));
        assert_eq!(merged.size, Size::new(32, 32));
    }

    #[test]
    fn test_merge_rectangles_overlapping() {
        let rects = vec![
            Rectangle::new(Point::new(0, 0), Size::new(20, 20)),
            Rectangle::new(Point::new(10, 10), Size::new(20, 20)),
        ];

        let merged = merge_rectangles(&rects).unwrap();

        // Bounding box is (0, 0) to (30, 30)
        // Aligned: (0, 0) to (32, 32)
        assert_eq!(merged.top_left, Point::new(0, 0));
        assert_eq!(merged.size, Size::new(32, 32));
    }
}
