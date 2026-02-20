//! Rendering system that converts layout trees to embedded-graphics primitives
//!
//! This module provides the rendering abstraction that takes computed layouts
//! and draws them to any `DrawTarget` from embedded-graphics.
//!
//! # Architecture
//!
//! - `Renderable` trait: Core abstraction for anything that can be rendered
//! - Helper functions: Background rendering, tree rendering
//! - Integration: Works with embedded-graphics primitives
//!
//! # Example
//!
//! ```no_run
//! use eink_system::render::*;
//! use embedded_graphics::prelude::*;
//! use embedded_graphics::pixelcolor::Gray4;
//!
//! # struct MyDisplay;
//! # impl DrawTarget for MyDisplay {
//! #     type Color = Gray4;
//! #     type Error = core::convert::Infallible;
//! #     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
//! #     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
//! # }
//! # impl OriginDimensions for MyDisplay {
//! #     fn size(&self) -> Size { Size::new(64, 64) }
//! # }
//! # let mut display = MyDisplay;
//! # let layout_result = LayoutResult::new(Point::zero(), Size::zero());
//!
//! // Render a computed layout to display
//! render_layout_tree(&layout_result, Point::zero(), &mut display)?;
//! # Ok::<(), core::convert::Infallible>(())
//! ```

use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use heapless::Vec;

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

/// Maximum number of child layouts supported in a single container
pub const MAX_CHILDREN: usize = 32;

/// Result of a layout computation
///
/// Contains the position and size of a layout node, along with
/// any child layouts.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutResult {
    /// Position of this layout node (relative to parent)
    pub position: Point,
    /// Size of this layout node
    pub size: Size,
    /// Child layout nodes (boxed to avoid infinite recursion)
    pub children: Vec<Box<LayoutResult>, MAX_CHILDREN>,
}

impl LayoutResult {
    /// Create a new layout result
    pub fn new(position: Point, size: Size) -> Self {
        Self {
            position,
            size,
            children: Vec::new(),
        }
    }

    /// Create a layout result with children
    pub fn with_children(
        position: Point,
        size: Size,
        children: Vec<Box<LayoutResult>, MAX_CHILDREN>,
    ) -> Self {
        Self {
            position,
            size,
            children,
        }
    }

    /// Add a child layout
    #[allow(clippy::result_large_err)]
    pub fn add_child(&mut self, child: LayoutResult) -> Result<(), LayoutResult> {
        self.children.push(Box::new(child)).map_err(|boxed| *boxed)
    }

    /// Get the bounds rectangle (position + size)
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.position, self.size)
    }

    /// Get absolute bounds (accounting for offset)
    // SAFETY: position and offset are display coordinates; their sum is well within i32 range.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn absolute_bounds(&self, offset: Point) -> Rectangle {
        Rectangle::new(self.position + offset, self.size)
    }
}

/// Trait for types that can be rendered to a display
///
/// This is the core abstraction for the rendering system. Any type
/// that implements this trait can be drawn to an embedded-graphics
/// `DrawTarget`.
pub trait Renderable {
    /// Render this element to the display at the given offset
    ///
    /// # Parameters
    ///
    /// - `display`: The target display to render to
    /// - `offset`: The absolute offset to render at (for nested layouts)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use eink_system::render::*;
    /// use embedded_graphics::prelude::*;
    /// use embedded_graphics::pixelcolor::Gray4;
    ///
    /// # struct MyDisplay;
    /// # impl DrawTarget for MyDisplay {
    /// #     type Color = Gray4;
    /// #     type Error = core::convert::Infallible;
    /// #     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    /// #     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
    /// # }
    /// # impl OriginDimensions for MyDisplay {
    /// #     fn size(&self) -> Size { Size::new(64, 64) }
    /// # }
    /// # struct MyRenderable;
    /// # impl Renderable for MyRenderable {
    /// #     fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D, offset: Point) -> Result<(), D::Error> {
    /// #         Ok(())
    /// #     }
    /// # }
    /// # let mut display = MyDisplay;
    /// # let renderable = MyRenderable;
    ///
    /// renderable.render(&mut display, Point::zero())?;
    /// # Ok::<(), core::convert::Infallible>(())
    /// ```
    fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        offset: Point,
    ) -> Result<(), D::Error>;
}

/// Render a background color in a rectangle
///
/// This is a helper function for rendering solid backgrounds.
///
/// # Parameters
///
/// - `rect`: The rectangle to fill
/// - `color`: The grayscale color to use
/// - `display`: The target display
///
/// # Example
///
/// ```no_run
/// use eink_system::render::*;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::primitives::Rectangle;
/// use embedded_graphics::pixelcolor::Gray4;
/// use embedded_graphics::pixelcolor::GrayColor;
///
/// # struct MyDisplay;
/// # impl DrawTarget for MyDisplay {
/// #     type Color = Gray4;
/// #     type Error = core::convert::Infallible;
/// #     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
/// #     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
/// # }
/// # impl OriginDimensions for MyDisplay {
/// #     fn size(&self) -> Size { Size::new(64, 64) }
/// # }
/// # let mut display = MyDisplay;
///
/// let rect = Rectangle::new(Point::new(10, 10), Size::new(100, 50));
/// render_background(rect, Gray4::WHITE, &mut display)?;
/// # Ok::<(), core::convert::Infallible>(())
/// ```
pub fn render_background<D: DrawTarget<Color = Gray4>>(
    rect: Rectangle,
    color: Gray4,
    display: &mut D,
) -> Result<(), D::Error> {
    let style = PrimitiveStyle::with_fill(color);
    rect.into_styled(style).draw(display)?;
    Ok(())
}

/// Render a layout tree to the display
///
/// This function recursively renders a layout tree, including all children.
/// It applies offsets and handles clipping to ensure elements don't render
/// outside their bounds.
///
/// # Parameters
///
/// - `layout`: The layout result to render
/// - `offset`: The absolute offset to render at
/// - `display`: The target display
///
/// # Example
///
/// ```no_run
/// use eink_system::render::*;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::pixelcolor::Gray4;
///
/// # struct MyDisplay;
/// # impl DrawTarget for MyDisplay {
/// #     type Color = Gray4;
/// #     type Error = core::convert::Infallible;
/// #     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
/// #     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
/// # }
/// # impl OriginDimensions for MyDisplay {
/// #     fn size(&self) -> Size { Size::new(64, 64) }
/// # }
/// # let mut display = MyDisplay;
/// # let layout = LayoutResult::new(Point::zero(), Size::zero());
///
/// render_layout_tree(&layout, Point::zero(), &mut display)?;
/// # Ok::<(), core::convert::Infallible>(())
/// ```
#[allow(clippy::only_used_in_recursion)]
// SAFETY: position and offset are display coordinates; their sum is well within i32 range.
#[allow(clippy::arithmetic_side_effects)]
pub fn render_layout_tree<D: DrawTarget<Color = Gray4>>(
    layout: &LayoutResult,
    offset: Point,
    display: &mut D,
) -> Result<(), D::Error> {
    // Calculate absolute position
    let absolute_position = layout.position + offset;

    // Render children with updated offset
    for child in layout.children.iter() {
        render_layout_tree(child.as_ref(), absolute_position, display)?;
    }

    Ok(())
}

/// Render a layout tree with background color
///
/// Similar to `render_layout_tree` but also renders a background color
/// before rendering children.
///
/// # Parameters
///
/// - `layout`: The layout result to render
/// - `offset`: The absolute offset to render at
/// - `background`: Optional background color to render
/// - `display`: The target display
///
/// # Example
///
/// ```no_run
/// use eink_system::render::*;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::pixelcolor::Gray4;
///
/// # struct MyDisplay;
/// # impl DrawTarget for MyDisplay {
/// #     type Color = Gray4;
/// #     type Error = core::convert::Infallible;
/// #     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
/// #     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
/// # }
/// # impl OriginDimensions for MyDisplay {
/// #     fn size(&self) -> Size { Size::new(64, 64) }
/// # }
/// # let mut display = MyDisplay;
/// # let layout = LayoutResult::new(Point::zero(), Size::zero());
///
/// render_layout_with_background(
///     &layout,
///     Point::zero(),
///     Some(Gray4::WHITE),
///     &mut display
/// )?;
/// # Ok::<(), core::convert::Infallible>(())
/// ```
pub fn render_layout_with_background<D: DrawTarget<Color = Gray4>>(
    layout: &LayoutResult,
    offset: Point,
    background: Option<Gray4>,
    display: &mut D,
) -> Result<(), D::Error> {
    // Render background if set
    if let Some(color) = background {
        let bounds = layout.absolute_bounds(offset);
        render_background(bounds, color, display)?;
    }

    // Render children
    render_layout_tree(layout, offset, display)?;

    Ok(())
}

/// Check if a rectangle is within bounds (for clipping)
///
/// This helper function checks if a rectangle is completely outside
/// the clipping bounds, allowing early rejection of offscreen elements.
///
/// # Parameters
///
/// - `rect`: The rectangle to check
/// - `clip_bounds`: The clipping bounds
///
/// # Returns
///
/// `true` if the rectangle intersects with the clip bounds, `false` otherwise
///
/// # Example
///
/// ```
/// use eink_system::render::*;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::primitives::Rectangle;
///
/// let rect = Rectangle::new(Point::new(10, 10), Size::new(50, 50));
/// let clip = Rectangle::new(Point::zero(), Size::new(100, 100));
///
/// assert!(is_visible(rect, clip));
///
/// let offscreen = Rectangle::new(Point::new(200, 200), Size::new(50, 50));
/// assert!(!is_visible(offscreen, clip));
/// ```
// SAFETY: coordinate arithmetic here adds i32 positions and i32-cast pixel sizes.
// Display dimensions (max ~4000px) added to typical screen coordinates are far from i32::MAX.
#[allow(clippy::arithmetic_side_effects)]
pub fn is_visible(rect: Rectangle, clip_bounds: Rectangle) -> bool {
    let rect_right = rect.top_left.x + rect.size.width as i32;
    let rect_bottom = rect.top_left.y + rect.size.height as i32;
    let clip_right = clip_bounds.top_left.x + clip_bounds.size.width as i32;
    let clip_bottom = clip_bounds.top_left.y + clip_bounds.size.height as i32;

    // Check if rectangles intersect
    !(rect.top_left.x >= clip_right
        || rect_right <= clip_bounds.top_left.x
        || rect.top_left.y >= clip_bottom
        || rect_bottom <= clip_bounds.top_left.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::mock_display::MockDisplay;

    #[test]
    fn test_layout_result_creation() {
        let layout = LayoutResult::new(Point::new(10, 20), Size::new(100, 50));
        assert_eq!(layout.position, Point::new(10, 20));
        assert_eq!(layout.size, Size::new(100, 50));
        assert_eq!(layout.children.len(), 0);
    }

    #[test]
    #[allow(clippy::indexing_slicing)]
    fn test_layout_result_with_children() {
        let child1 = LayoutResult::new(Point::new(0, 0), Size::new(50, 25));
        let child2 = LayoutResult::new(Point::new(50, 0), Size::new(50, 25));

        let mut children = Vec::new();
        children.push(Box::new(child1)).unwrap();
        children.push(Box::new(child2)).unwrap();

        let layout = LayoutResult::with_children(Point::new(10, 20), Size::new(100, 50), children);

        assert_eq!(layout.children.len(), 2);
        assert_eq!(layout.children[0].size, Size::new(50, 25));
        assert_eq!(layout.children[1].position, Point::new(50, 0));
    }

    #[test]
    fn test_add_child() {
        let mut layout = LayoutResult::new(Point::zero(), Size::new(100, 100));
        let child = LayoutResult::new(Point::new(10, 10), Size::new(50, 50));

        layout.add_child(child).unwrap();
        assert_eq!(layout.children.len(), 1);
    }

    #[test]
    fn test_bounds() {
        let layout = LayoutResult::new(Point::new(10, 20), Size::new(100, 50));
        let bounds = layout.bounds();

        assert_eq!(bounds.top_left, Point::new(10, 20));
        assert_eq!(bounds.size, Size::new(100, 50));
    }

    #[test]
    fn test_absolute_bounds() {
        let layout = LayoutResult::new(Point::new(10, 20), Size::new(100, 50));
        let offset = Point::new(5, 5);
        let bounds = layout.absolute_bounds(offset);

        assert_eq!(bounds.top_left, Point::new(15, 25));
        assert_eq!(bounds.size, Size::new(100, 50));
    }

    #[test]
    fn test_render_background() {
        let mut display = MockDisplay::new();
        let rect = Rectangle::new(Point::new(0, 0), Size::new(5, 5));

        render_background(rect, Gray4::BLACK, &mut display).unwrap();

        // Verify that pixels were drawn
        assert_eq!(display.affected_area(), rect);
    }

    #[test]
    fn test_render_background_different_colors() {
        let mut display_white = MockDisplay::new();
        let mut display_black = MockDisplay::new();
        let rect = Rectangle::new(Point::new(0, 0), Size::new(3, 3));

        render_background(rect, Gray4::WHITE, &mut display_white).unwrap();
        render_background(rect, Gray4::BLACK, &mut display_black).unwrap();

        // Both should have the same affected area
        assert_eq!(display_white.affected_area(), rect);
        assert_eq!(display_black.affected_area(), rect);
    }

    #[test]
    fn test_render_layout_tree_empty() {
        let mut display = MockDisplay::new();
        let layout = LayoutResult::new(Point::zero(), Size::new(100, 100));

        // Should not error on empty layout
        render_layout_tree(&layout, Point::zero(), &mut display).unwrap();
    }

    #[test]
    fn test_render_layout_tree_with_children() {
        let mut display = MockDisplay::new();

        let mut layout = LayoutResult::new(Point::new(10, 10), Size::new(100, 100));
        let child1 = LayoutResult::new(Point::new(0, 0), Size::new(50, 50));
        let child2 = LayoutResult::new(Point::new(50, 50), Size::new(50, 50));

        layout.add_child(child1).unwrap();
        layout.add_child(child2).unwrap();

        // Should render without errors
        render_layout_tree(&layout, Point::zero(), &mut display).unwrap();
    }

    #[test]
    fn test_render_layout_tree_with_offset() {
        let mut display = MockDisplay::new();
        let layout = LayoutResult::new(Point::new(10, 10), Size::new(50, 50));
        let offset = Point::new(20, 20);

        // Offset should be applied when rendering
        render_layout_tree(&layout, offset, &mut display).unwrap();
    }

    #[test]
    fn test_render_layout_tree_nested() {
        let mut display = MockDisplay::new();

        // Create nested layout: parent -> child -> grandchild
        let grandchild = LayoutResult::new(Point::new(5, 5), Size::new(10, 10));

        let mut child = LayoutResult::new(Point::new(10, 10), Size::new(30, 30));
        child.add_child(grandchild).unwrap();

        let mut parent = LayoutResult::new(Point::new(0, 0), Size::new(100, 100));
        parent.add_child(child).unwrap();

        // Should handle nesting correctly
        render_layout_tree(&parent, Point::zero(), &mut display).unwrap();
    }

    #[test]
    fn test_render_layout_with_background() {
        let mut display = MockDisplay::new();
        let layout = LayoutResult::new(Point::new(10, 10), Size::new(50, 50));

        render_layout_with_background(&layout, Point::zero(), Some(Gray4::WHITE), &mut display)
            .unwrap();

        // Background should be drawn
        let expected_rect = Rectangle::new(Point::new(10, 10), Size::new(50, 50));
        assert_eq!(display.affected_area(), expected_rect);
    }

    #[test]
    fn test_render_layout_with_no_background() {
        let mut display = MockDisplay::new();
        let layout = LayoutResult::new(Point::new(10, 10), Size::new(50, 50));

        render_layout_with_background(&layout, Point::zero(), None, &mut display).unwrap();

        // No background means no pixels should be drawn
        assert_eq!(
            display.affected_area(),
            Rectangle::new(Point::zero(), Size::zero())
        );
    }

    #[test]
    fn test_render_layout_with_background_and_children() {
        let mut display = MockDisplay::new();

        // MockDisplay default size is 64x64; keep layout within bounds
        let mut layout = LayoutResult::new(Point::new(0, 0), Size::new(40, 40));
        let child = LayoutResult::new(Point::new(5, 5), Size::new(20, 20));
        layout.add_child(child).unwrap();

        render_layout_with_background(&layout, Point::zero(), Some(Gray4::new(2)), &mut display)
            .unwrap();
    }

    #[test]
    fn test_is_visible_completely_inside() {
        let rect = Rectangle::new(Point::new(10, 10), Size::new(50, 50));
        let clip = Rectangle::new(Point::zero(), Size::new(100, 100));

        assert!(is_visible(rect, clip));
    }

    #[test]
    fn test_is_visible_completely_outside() {
        let rect = Rectangle::new(Point::new(200, 200), Size::new(50, 50));
        let clip = Rectangle::new(Point::zero(), Size::new(100, 100));

        assert!(!is_visible(rect, clip));
    }

    #[test]
    fn test_is_visible_partially_overlapping() {
        let rect = Rectangle::new(Point::new(50, 50), Size::new(100, 100));
        let clip = Rectangle::new(Point::zero(), Size::new(100, 100));

        assert!(is_visible(rect, clip));
    }

    #[test]
    fn test_is_visible_edge_cases() {
        let clip = Rectangle::new(Point::zero(), Size::new(100, 100));

        // Touching left edge
        let rect_left = Rectangle::new(Point::new(-50, 10), Size::new(51, 50));
        assert!(is_visible(rect_left, clip));

        // Touching top edge
        let rect_top = Rectangle::new(Point::new(10, -50), Size::new(50, 51));
        assert!(is_visible(rect_top, clip));

        // Touching right edge
        let rect_right = Rectangle::new(Point::new(99, 10), Size::new(50, 50));
        assert!(is_visible(rect_right, clip));

        // Touching bottom edge
        let rect_bottom = Rectangle::new(Point::new(10, 99), Size::new(50, 50));
        assert!(is_visible(rect_bottom, clip));
    }

    #[test]
    fn test_is_visible_just_outside() {
        let clip = Rectangle::new(Point::zero(), Size::new(100, 100));

        // Just outside left
        let rect_left = Rectangle::new(Point::new(-50, 10), Size::new(50, 50));
        assert!(!is_visible(rect_left, clip));

        // Just outside top
        let rect_top = Rectangle::new(Point::new(10, -50), Size::new(50, 50));
        assert!(!is_visible(rect_top, clip));

        // Just outside right
        let rect_right = Rectangle::new(Point::new(100, 10), Size::new(50, 50));
        assert!(!is_visible(rect_right, clip));

        // Just outside bottom
        let rect_bottom = Rectangle::new(Point::new(10, 100), Size::new(50, 50));
        assert!(!is_visible(rect_bottom, clip));
    }

    #[test]
    fn test_offset_accumulation() {
        let mut display = MockDisplay::new();

        // Create a layout at (10, 10)
        let child = LayoutResult::new(Point::new(5, 5), Size::new(20, 20));
        let mut parent = LayoutResult::new(Point::new(10, 10), Size::new(50, 50));
        parent.add_child(child).unwrap();

        // Render with offset (20, 20)
        // Child should be at (10, 10) + (20, 20) + (5, 5) = (35, 35)
        render_layout_tree(&parent, Point::new(20, 20), &mut display).unwrap();
    }
}
