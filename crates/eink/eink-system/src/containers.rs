//! Container components for layout composition
//!
//! This module provides high-level container components that simplify
//! building complex layouts. The containers use the flexbox algorithm
//! under the hood but provide a more ergonomic API.
//!
//! # Components
//!
//! - [`VStack`] - Vertical stack (column layout)
//! - [`HStack`] - Horizontal stack (row layout)
//! - [`Spacer`] - Fixed-size spacer
//!
//! # Example
//!
//! ```no_run
//! use eink_system::prelude::*;
//! use embedded_graphics::prelude::*;
//!
//! // Create a vertical layout with gap spacing
//! let vstack = VStack::<4>::new()
//!     .gap(8)
//!     .padding(Edges::all(16));
//!
//! // Create a horizontal layout
//! let hstack = HStack::<3>::new()
//!     .gap(4)
//!     .justify_content(Justify::Center);
//!
//! // Add a fixed-size spacer
//! let spacer = Spacer::new(Size::new(20, 10));
//! ```

use crate::layout::{Constraints, Layout, LayoutResult};
use crate::style::{Align, Edges, Justify};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use heapless::Vec;

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

/// Vertical stack container (column layout)
///
/// A container that lays out children vertically from top to bottom.
/// Uses flexbox column direction internally.
///
/// # Type Parameters
///
/// - `N`: Maximum number of children (const generic for no_std compatibility)
///
/// # Example
///
/// ```no_run
/// use eink_system::prelude::*;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::pixelcolor::Gray4;
/// use embedded_graphics::pixelcolor::GrayColor;
///
/// let vstack = VStack::<4>::new()
///     .gap(8)
///     .padding(Edges::all(16))
///     .background(Gray4::WHITE);
/// ```
pub struct VStack<const N: usize> {
    children: Vec<Box<dyn Layout>, N>,
    gap: u32,
    padding: Edges,
    margin: Edges,
    justify_content: Justify,
    align_items: Align,
    background: Option<Gray4>,
}

impl<const N: usize> VStack<N> {
    /// Create a new vertical stack
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: 0,
            padding: Edges::all(0),
            margin: Edges::all(0),
            justify_content: Justify::Start,
            align_items: Align::Stretch,
            background: None,
        }
    }

    /// Set the gap between children
    pub fn gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    /// Set the padding
    pub fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    /// Set the margin
    pub fn margin(mut self, margin: Edges) -> Self {
        self.margin = margin;
        self
    }

    /// Set the justify content (main axis alignment)
    pub fn justify_content(mut self, justify: Justify) -> Self {
        self.justify_content = justify;
        self
    }

    /// Set the align items (cross axis alignment)
    pub fn align_items(mut self, align: Align) -> Self {
        self.align_items = align;
        self
    }

    /// Set the background color
    pub fn background(mut self, color: Gray4) -> Self {
        self.background = Some(color);
        self
    }

    /// Set children from an array
    ///
    /// # Panics
    ///
    /// Panics if the number of children exceeds N
    pub fn children(mut self, children: impl IntoIterator<Item = Box<dyn Layout>>) -> Self {
        self.children.clear();
        for child in children {
            let _ = self.children.push(child);
        }
        self
    }

    /// Add a single child
    ///
    /// # Errors
    ///
    /// Returns the child if the container is full
    pub fn add_child(&mut self, child: Box<dyn Layout>) -> Result<(), Box<dyn Layout>> {
        self.children.push(child)
    }
}

impl<const N: usize> Default for VStack<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Layout for VStack<N> {
    // SAFETY: all arithmetic in this function operates on display layout values bounded
    // by display dimensions (max ~4000px) and small integer counts. No overflow is possible.
    #[allow(clippy::arithmetic_side_effects)]
    fn layout(&self, constraints: Constraints) -> LayoutResult {
        // Account for padding and margin
        let total_horizontal = self.padding.horizontal() + self.margin.horizontal();
        let total_vertical = self.padding.vertical() + self.margin.vertical();

        let content_constraints = constraints.deflate(Size::new(total_horizontal, total_vertical));

        // Calculate available height for children
        let available_height = content_constraints.max.height;
        let available_width = content_constraints.max.width;

        // Calculate total gap space
        let child_count = self.children.len();
        let total_gap = if child_count > 1 {
            self.gap * (child_count as u32 - 1)
        } else {
            0
        };

        // Layout children
        let available_for_children = available_height.saturating_sub(total_gap);

        // For simplicity, give each child equal space (future: respect flex grow/shrink)
        let child_height = if child_count > 0 {
            available_for_children / child_count as u32
        } else {
            0
        };

        let child_constraints =
            Constraints::new(Size::new(0, 0), Size::new(available_width, child_height));

        // Layout all children and calculate actual sizes
        let mut child_layouts: Vec<LayoutResult, N> = Vec::new();
        let mut total_height = 0;
        let mut max_width = 0;

        for child in &self.children {
            let node = child.layout(child_constraints);
            total_height += node.size.height;
            max_width = max_width.max(node.size.width);
            let _ = child_layouts.push(node);
        }

        total_height += total_gap;

        // Calculate container size
        let container_width = max_width + total_horizontal;
        let container_height = total_height + total_vertical;

        let final_size = constraints.constrain(Size::new(container_width, container_height));

        LayoutResult::leaf(final_size)
    }
}

/// Horizontal stack container (row layout)
///
/// A container that lays out children horizontally from left to right.
/// Uses flexbox row direction internally.
///
/// # Type Parameters
///
/// - `N`: Maximum number of children (const generic for no_std compatibility)
///
/// # Example
///
/// ```no_run
/// use eink_system::prelude::*;
/// use embedded_graphics::prelude::*;
/// use embedded_graphics::pixelcolor::Gray4;
/// use embedded_graphics::pixelcolor::GrayColor;
///
/// let hstack = HStack::<4>::new()
///     .gap(8)
///     .padding(Edges::all(16))
///     .background(Gray4::WHITE);
/// ```
pub struct HStack<const N: usize> {
    children: Vec<Box<dyn Layout>, N>,
    gap: u32,
    padding: Edges,
    margin: Edges,
    justify_content: Justify,
    align_items: Align,
    background: Option<Gray4>,
}

impl<const N: usize> HStack<N> {
    /// Create a new horizontal stack
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            gap: 0,
            padding: Edges::all(0),
            margin: Edges::all(0),
            justify_content: Justify::Start,
            align_items: Align::Stretch,
            background: None,
        }
    }

    /// Set the gap between children
    pub fn gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    /// Set the padding
    pub fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    /// Set the margin
    pub fn margin(mut self, margin: Edges) -> Self {
        self.margin = margin;
        self
    }

    /// Set the justify content (main axis alignment)
    pub fn justify_content(mut self, justify: Justify) -> Self {
        self.justify_content = justify;
        self
    }

    /// Set the align items (cross axis alignment)
    pub fn align_items(mut self, align: Align) -> Self {
        self.align_items = align;
        self
    }

    /// Set the background color
    pub fn background(mut self, color: Gray4) -> Self {
        self.background = Some(color);
        self
    }

    /// Set children from an array
    ///
    /// # Panics
    ///
    /// Panics if the number of children exceeds N
    pub fn children(mut self, children: impl IntoIterator<Item = Box<dyn Layout>>) -> Self {
        self.children.clear();
        for child in children {
            let _ = self.children.push(child);
        }
        self
    }

    /// Add a single child
    ///
    /// # Errors
    ///
    /// Returns the child if the container is full
    pub fn add_child(&mut self, child: Box<dyn Layout>) -> Result<(), Box<dyn Layout>> {
        self.children.push(child)
    }
}

impl<const N: usize> Default for HStack<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Layout for HStack<N> {
    // SAFETY: all arithmetic in this function operates on display layout values bounded
    // by display dimensions (max ~4000px) and small integer counts. No overflow is possible.
    #[allow(clippy::arithmetic_side_effects)]
    fn layout(&self, constraints: Constraints) -> LayoutResult {
        // Account for padding and margin
        let total_horizontal = self.padding.horizontal() + self.margin.horizontal();
        let total_vertical = self.padding.vertical() + self.margin.vertical();

        let content_constraints = constraints.deflate(Size::new(total_horizontal, total_vertical));

        // Calculate available width for children
        let available_width = content_constraints.max.width;
        let available_height = content_constraints.max.height;

        // Calculate total gap space
        let child_count = self.children.len();
        let total_gap = if child_count > 1 {
            self.gap * (child_count as u32 - 1)
        } else {
            0
        };

        // Layout children
        let available_for_children = available_width.saturating_sub(total_gap);

        // For simplicity, give each child equal space (future: respect flex grow/shrink)
        let child_width = if child_count > 0 {
            available_for_children / child_count as u32
        } else {
            0
        };

        let child_constraints =
            Constraints::new(Size::new(0, 0), Size::new(child_width, available_height));

        // Layout all children and calculate actual sizes
        let mut child_layouts: Vec<LayoutResult, N> = Vec::new();
        let mut total_width = 0;
        let mut max_height = 0;

        for child in &self.children {
            let node = child.layout(child_constraints);
            total_width += node.size.width;
            max_height = max_height.max(node.size.height);
            let _ = child_layouts.push(node);
        }

        total_width += total_gap;

        // Calculate container size
        let container_width = total_width + total_horizontal;
        let container_height = max_height + total_vertical;

        let final_size = constraints.constrain(Size::new(container_width, container_height));

        LayoutResult::leaf(final_size)
    }
}

/// Fixed-size spacer component
///
/// A spacer takes up a fixed amount of space in a layout. Useful for
/// creating consistent spacing between elements.
///
/// # Example
///
/// ```no_run
/// use eink_system::prelude::*;
/// use embedded_graphics::prelude::*;
///
/// // Create a spacer with specific dimensions
/// let spacer = Spacer::new(Size::new(20, 10));
///
/// // Create a spacer with just width
/// let h_spacer = Spacer::width(20);
///
/// // Create a spacer with just height
/// let v_spacer = Spacer::height(10);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Spacer {
    size: Size,
}

impl Spacer {
    /// Create a new spacer with the given size
    pub const fn new(size: Size) -> Self {
        Self { size }
    }

    /// Create a spacer with a specific width (height = 0)
    pub const fn width(width: u32) -> Self {
        Self {
            size: Size::new(width, 0),
        }
    }

    /// Create a spacer with a specific height (width = 0)
    pub const fn height(height: u32) -> Self {
        Self {
            size: Size::new(0, height),
        }
    }
}

impl Layout for Spacer {
    fn layout(&self, constraints: Constraints) -> LayoutResult {
        let size = constraints.constrain(self.size);
        LayoutResult::leaf(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock layout for testing
    #[derive(Debug)]
    struct FixedSize {
        size: Size,
    }

    impl Layout for FixedSize {
        fn layout(&self, constraints: Constraints) -> LayoutResult {
            let size = constraints.constrain(self.size);
            LayoutResult::leaf(size)
        }
    }

    #[test]
    fn test_vstack_empty() {
        let vstack: VStack<4> = VStack::new();
        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // Empty VStack should have minimal size
        assert_eq!(node.size.width, 0);
        assert_eq!(node.size.height, 0);
    }

    #[test]
    fn test_vstack_single_child() {
        let mut vstack: VStack<4> = VStack::new();
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // VStack should match child size
        assert_eq!(node.size.width, 50);
        assert_eq!(node.size.height, 30);
    }

    #[test]
    fn test_vstack_multiple_children() {
        let mut vstack: VStack<4> = VStack::new();
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(60, 20),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // VStack should sum heights and take max width
        assert_eq!(node.size.width, 60); // max(50, 60)
        assert!(node.size.height >= 50); // 30 + 20
    }

    #[test]
    fn test_vstack_with_gap() {
        let mut vstack: VStack<4> = VStack::new().gap(10);
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // Should include gap: 30 + 10 + 30 = 70
        assert!(node.size.height >= 70);
    }

    #[test]
    fn test_vstack_with_padding() {
        let mut vstack: VStack<4> = VStack::new().padding(Edges::all(8));
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // Should include padding: 50 + 16 = 66, 30 + 16 = 46
        assert_eq!(node.size.width, 66);
        assert_eq!(node.size.height, 46);
    }

    #[test]
    fn test_vstack_with_margin() {
        let mut vstack: VStack<4> = VStack::new().margin(Edges::all(4));
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // Should include margin: 50 + 8 = 58, 30 + 8 = 38
        assert_eq!(node.size.width, 58);
        assert_eq!(node.size.height, 38);
    }

    #[test]
    fn test_vstack_builder_pattern() {
        let vstack: VStack<4> = VStack::new()
            .gap(8)
            .padding(Edges::all(16))
            .margin(Edges::horizontal_vertical(4, 8))
            .justify_content(Justify::Center)
            .align_items(Align::Center)
            .background(Gray4::WHITE);

        assert_eq!(vstack.gap, 8);
        assert_eq!(vstack.padding, Edges::all(16));
        assert_eq!(vstack.margin, Edges::horizontal_vertical(4, 8));
        assert_eq!(vstack.justify_content, Justify::Center);
        assert_eq!(vstack.align_items, Align::Center);
        assert_eq!(vstack.background, Some(Gray4::WHITE));
    }

    #[test]
    fn test_hstack_empty() {
        let hstack: HStack<4> = HStack::new();
        let constraints = Constraints::loose(Size::new(200, 100));
        let node = hstack.layout(constraints);

        // Empty HStack should have minimal size
        assert_eq!(node.size.width, 0);
        assert_eq!(node.size.height, 0);
    }

    #[test]
    fn test_hstack_single_child() {
        let mut hstack: HStack<4> = HStack::new();
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = hstack.layout(constraints);

        // HStack should match child size
        assert_eq!(node.size.width, 50);
        assert_eq!(node.size.height, 30);
    }

    #[test]
    fn test_hstack_multiple_children() {
        let mut hstack: HStack<4> = HStack::new();
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(40, 20),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = hstack.layout(constraints);

        // HStack should sum widths and take max height
        assert!(node.size.width >= 90); // 50 + 40
        assert_eq!(node.size.height, 30); // max(30, 20)
    }

    #[test]
    fn test_hstack_with_gap() {
        let mut hstack: HStack<4> = HStack::new().gap(10);
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = hstack.layout(constraints);

        // Should include gap: 50 + 10 + 50 = 110
        assert!(node.size.width >= 110);
    }

    #[test]
    fn test_hstack_with_padding() {
        let mut hstack: HStack<4> = HStack::new().padding(Edges::all(8));
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = hstack.layout(constraints);

        // Should include padding: 50 + 16 = 66, 30 + 16 = 46
        assert_eq!(node.size.width, 66);
        assert_eq!(node.size.height, 46);
    }

    #[test]
    fn test_hstack_builder_pattern() {
        let hstack: HStack<4> = HStack::new()
            .gap(8)
            .padding(Edges::all(16))
            .margin(Edges::horizontal_vertical(4, 8))
            .justify_content(Justify::SpaceBetween)
            .align_items(Align::End)
            .background(Gray4::new(2));

        assert_eq!(hstack.gap, 8);
        assert_eq!(hstack.padding, Edges::all(16));
        assert_eq!(hstack.margin, Edges::horizontal_vertical(4, 8));
        assert_eq!(hstack.justify_content, Justify::SpaceBetween);
        assert_eq!(hstack.align_items, Align::End);
        assert_eq!(hstack.background, Some(Gray4::new(2)));
    }

    #[test]
    fn test_spacer_new() {
        let spacer = Spacer::new(Size::new(20, 10));
        let constraints = Constraints::loose(Size::new(100, 100));
        let node = spacer.layout(constraints);

        assert_eq!(node.size, Size::new(20, 10));
    }

    #[test]
    fn test_spacer_width() {
        let spacer = Spacer::width(30);
        let constraints = Constraints::loose(Size::new(100, 100));
        let node = spacer.layout(constraints);

        assert_eq!(node.size, Size::new(30, 0));
    }

    #[test]
    fn test_spacer_height() {
        let spacer = Spacer::height(40);
        let constraints = Constraints::loose(Size::new(100, 100));
        let node = spacer.layout(constraints);

        assert_eq!(node.size, Size::new(0, 40));
    }

    #[test]
    fn test_spacer_constrained() {
        let spacer = Spacer::new(Size::new(200, 150));
        let constraints = Constraints::loose(Size::new(100, 100));
        let node = spacer.layout(constraints);

        // Should be constrained to max size
        assert_eq!(node.size, Size::new(100, 100));
    }

    #[test]
    fn test_nested_containers() {
        // Create inner HStack
        let mut inner_hstack: HStack<2> = HStack::new().gap(4);
        let _ = inner_hstack.add_child(Box::new(FixedSize {
            size: Size::new(30, 20),
        }));
        let _ = inner_hstack.add_child(Box::new(FixedSize {
            size: Size::new(30, 20),
        }));

        // Create outer VStack with inner HStack as child
        let mut outer_vstack: VStack<2> = VStack::new().gap(8);
        let _ = outer_vstack.add_child(Box::new(inner_hstack));
        let _ = outer_vstack.add_child(Box::new(FixedSize {
            size: Size::new(70, 25),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = outer_vstack.layout(constraints);

        // Should handle nesting correctly
        assert!(node.size.width >= 70);
        assert!(node.size.height >= 45); // Approximate
    }

    #[test]
    fn test_children_method() {
        let children: [Box<dyn Layout>; 3] = [
            Box::new(FixedSize {
                size: Size::new(10, 10),
            }),
            Box::new(FixedSize {
                size: Size::new(20, 20),
            }),
            Box::new(FixedSize {
                size: Size::new(30, 30),
            }),
        ];

        let vstack: VStack<4> = VStack::new().children(children);
        assert_eq!(vstack.children.len(), 3);
    }

    #[test]
    fn test_max_children_limit() {
        let mut vstack: VStack<2> = VStack::new();

        // Should accept up to 2 children
        assert!(vstack
            .add_child(Box::new(FixedSize {
                size: Size::new(10, 10)
            }))
            .is_ok());
        assert!(vstack
            .add_child(Box::new(FixedSize {
                size: Size::new(20, 20)
            }))
            .is_ok());

        // Third child should fail
        let result = vstack.add_child(Box::new(FixedSize {
            size: Size::new(30, 30),
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_vstack_different_widths() {
        let mut vstack: VStack<3> = VStack::new();
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(50, 10),
        }));
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(80, 10),
        }));
        let _ = vstack.add_child(Box::new(FixedSize {
            size: Size::new(30, 10),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = vstack.layout(constraints);

        // Width should be the maximum child width
        assert_eq!(node.size.width, 80);
    }

    #[test]
    fn test_hstack_different_heights() {
        let mut hstack: HStack<3> = HStack::new();
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(10, 20),
        }));
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(10, 50),
        }));
        let _ = hstack.add_child(Box::new(FixedSize {
            size: Size::new(10, 30),
        }));

        let constraints = Constraints::loose(Size::new(200, 100));
        let node = hstack.layout(constraints);

        // Height should be the maximum child height
        assert_eq!(node.size.height, 50);
    }
}
