//! Alignment Constraints for E-Ink Partial Updates
//!
//! E-ink controllers have addressing limitations that require coordinates
//! to be aligned to multiples of 8 pixels.

use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// Alignment constraint for e-ink partial updates
///
/// Most e-ink controllers (SSD1680, UC8151, etc.) require partial update
/// regions to be aligned to 8-pixel boundaries due to how the display
/// RAM is addressed.
pub const ALIGNMENT: u32 = 8;

/// Align a coordinate down to the nearest multiple of ALIGNMENT
///
/// # Examples
/// ```
/// # use eink_emulator::alignment::align_down;
/// assert_eq!(align_down(0), 0);
/// assert_eq!(align_down(5), 0);
/// assert_eq!(align_down(8), 8);
/// assert_eq!(align_down(13), 8);
/// ```
// SAFETY: ALIGNMENT = 8, value is a display coordinate; (value / 8) * 8 cannot overflow u32.
#[allow(clippy::arithmetic_side_effects)]
pub fn align_down(value: u32) -> u32 {
    (value / ALIGNMENT) * ALIGNMENT
}

/// Align a coordinate up to the nearest multiple of ALIGNMENT
///
/// # Examples
/// ```
/// # use eink_emulator::alignment::align_up;
/// assert_eq!(align_up(0), 0);
/// assert_eq!(align_up(5), 8);
/// assert_eq!(align_up(8), 8);
/// assert_eq!(align_up(13), 16);
/// ```
// SAFETY: ALIGNMENT = 8, value is a display coordinate; div_ceil(8) * 8 cannot overflow u32.
#[allow(clippy::arithmetic_side_effects)]
pub fn align_up(value: u32) -> u32 {
    value.div_ceil(ALIGNMENT) * ALIGNMENT
}

/// Check if a value is properly aligned
///
/// # Examples
/// ```
/// # use eink_emulator::alignment::is_aligned;
/// assert!(is_aligned(0));
/// assert!(is_aligned(8));
/// assert!(is_aligned(16));
/// assert!(!is_aligned(5));
/// assert!(!is_aligned(13));
/// ```
pub fn is_aligned(value: u32) -> bool {
    value.is_multiple_of(ALIGNMENT)
}

/// Align a rectangle to meet e-ink partial update requirements
///
/// Expands the rectangle to ensure both position and size are aligned
/// to 8-pixel boundaries.
///
/// # Examples
/// ```
/// # use eink_emulator::alignment::align_rectangle;
/// # use embedded_graphics::prelude::*;
/// # use embedded_graphics::primitives::Rectangle;
///
/// let rect = Rectangle::new(Point::new(5, 5), Size::new(10, 10));
/// let aligned = align_rectangle(&rect);
///
/// assert_eq!(aligned.top_left, Point::new(0, 0));
/// assert_eq!(aligned.size, Size::new(16, 16));
/// ```
// SAFETY: align_rectangle operates on display coordinates; aligned_right >= aligned_x
// by construction (align_up >= align_down on non-negative inputs), so subtraction is safe.
#[allow(clippy::arithmetic_side_effects)]
pub fn align_rectangle(rect: &Rectangle) -> Rectangle {
    let top_left = rect.top_left;
    let bottom_right = rect.bottom_right().unwrap_or(top_left);

    // Align top-left down (expand outward)
    let aligned_x = align_down(top_left.x.max(0) as u32);
    let aligned_y = align_down(top_left.y.max(0) as u32);

    // Align bottom-right up (expand outward)
    let aligned_right = align_up(bottom_right.x.max(0) as u32);
    let aligned_bottom = align_up(bottom_right.y.max(0) as u32);

    // Calculate aligned size
    let width = aligned_right - aligned_x;
    let height = aligned_bottom - aligned_y;

    Rectangle::new(
        Point::new(aligned_x as i32, aligned_y as i32),
        Size::new(width, height),
    )
}

/// Validate and align a partial update region
///
/// Returns the aligned rectangle and a boolean indicating if alignment was needed.
pub fn validate_and_align(rect: &Rectangle) -> (Rectangle, bool) {
    let aligned = align_rectangle(rect);
    let needed_alignment = aligned != *rect;
    (aligned, needed_alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_down() {
        assert_eq!(align_down(0), 0);
        assert_eq!(align_down(1), 0);
        assert_eq!(align_down(7), 0);
        assert_eq!(align_down(8), 8);
        assert_eq!(align_down(9), 8);
        assert_eq!(align_down(15), 8);
        assert_eq!(align_down(16), 16);
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0), 0);
        assert_eq!(align_up(1), 8);
        assert_eq!(align_up(7), 8);
        assert_eq!(align_up(8), 8);
        assert_eq!(align_up(9), 16);
        assert_eq!(align_up(15), 16);
        assert_eq!(align_up(16), 16);
    }

    #[test]
    fn test_is_aligned() {
        assert!(is_aligned(0));
        assert!(is_aligned(8));
        assert!(is_aligned(16));
        assert!(is_aligned(24));

        assert!(!is_aligned(1));
        assert!(!is_aligned(7));
        assert!(!is_aligned(9));
        assert!(!is_aligned(15));
    }

    #[test]
    fn test_align_rectangle_already_aligned() {
        let rect = Rectangle::new(Point::new(0, 0), Size::new(16, 16));
        let aligned = align_rectangle(&rect);

        assert_eq!(rect, aligned);
    }

    #[test]
    fn test_align_rectangle_needs_alignment() {
        let rect = Rectangle::new(Point::new(5, 5), Size::new(10, 10));
        let aligned = align_rectangle(&rect);

        // Should expand to (0, 0) with size (16, 16)
        assert_eq!(aligned.top_left, Point::new(0, 0));
        assert_eq!(aligned.size, Size::new(16, 16));
    }

    #[test]
    fn test_align_rectangle_partial() {
        let rect = Rectangle::new(Point::new(8, 12), Size::new(24, 20));
        let aligned = align_rectangle(&rect);

        // Top-left: (8, 12) → (8, 8)
        // Bottom-right: (32, 32) → (32, 32)
        // Size: 24×24
        assert_eq!(aligned.top_left, Point::new(8, 8));
        assert_eq!(aligned.size, Size::new(24, 24));
    }

    #[test]
    fn test_validate_and_align() {
        // Already aligned
        let rect = Rectangle::new(Point::new(0, 0), Size::new(16, 16));
        let (aligned, needed) = validate_and_align(&rect);
        assert_eq!(rect, aligned);
        assert!(!needed);

        // Needs alignment
        let rect = Rectangle::new(Point::new(5, 5), Size::new(10, 10));
        let (aligned, needed) = validate_and_align(&rect);
        assert_ne!(rect, aligned);
        assert!(needed);
    }

    #[test]
    fn test_alignment_constant() {
        // Ensure alignment is 8 as per hardware requirements
        assert_eq!(ALIGNMENT, 8);
    }
}
