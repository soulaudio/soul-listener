//! Core layout system for e-ink displays.
//!
//! This module provides the fundamental types for constraint-based layout,
//! inspired by Flutter's box constraints and CSS flexbox.
//!
//! # Layout Algorithm
//!
//! The layout system uses a two-pass constraint-based approach:
//!
//! 1. **Constraint Pass (Down)**: Parent passes constraints to children
//! 2. **Size Pass (Up)**: Children return their size and children positions
//!
//! ## Example
//!
//! ```rust
//! use eink_system::layout::{Constraints, Layout, LayoutResult};
//! use embedded_graphics::prelude::{Size, Point};
//!
//! struct MyComponent;
//!
//! impl Layout for MyComponent {
//!     fn layout(&self, constraints: Constraints) -> LayoutResult {
//!         // Choose size within constraints
//!         let size = constraints.constrain(Size::new(100, 50));
//!
//!         LayoutResult::leaf(size)
//!     }
//! }
//!
//! // Usage
//! let component = MyComponent;
//! let constraints = Constraints::tight(Size::new(100, 50));
//! let result = component.layout(constraints);
//! assert_eq!(result.size, Size::new(100, 50));
//! ```
//!
//! # Constraints
//!
//! Constraints define the valid range of sizes a component can have:
//!
//! - **Tight**: Exact size (min == max)
//! - **Loose**: Range from zero to max
//! - **Bounded**: Range from min to max
//! - **Unbounded**: Infinite maximum (use with care!)
//!
//! ```rust
//! use eink_system::layout::Constraints;
//! use embedded_graphics::prelude::Size;
//!
//! // Exact size
//! let tight = Constraints::tight(Size::new(100, 50));
//!
//! // Zero to max
//! let loose = Constraints::loose(Size::new(200, 100));
//!
//! // Custom range
//! let custom = Constraints::new(
//!     Size::new(50, 25),   // min
//!     Size::new(200, 100), // max
//! );
//! ```

#[cfg(feature = "debug")]
use crate::debug::DebugInfo;
use embedded_graphics::prelude::{Point, Size};
use heapless::Vec;

/// Maximum number of children in a layout result.
///
/// This is a compile-time constant to enable `no_std` compatibility.
/// Most UI components have far fewer children than this limit.
pub const MAX_CHILDREN: usize = 32;

/// Constraints define the valid range of sizes for layout.
///
/// Constraints flow down the component tree from parent to child,
/// allowing parents to control the size of their children.
///
/// # Fields
///
/// - `min`: Minimum allowed size (inclusive)
/// - `max`: Maximum allowed size (inclusive)
///
/// # Invariants
///
/// - `min.width <= max.width`
/// - `min.height <= max.height`
///
/// These invariants are maintained by the constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Constraints {
    /// Minimum allowed size (inclusive).
    pub min: Size,
    /// Maximum allowed size (inclusive).
    pub max: Size,
}

impl Constraints {
    /// Create new constraints with explicit min and max bounds.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if min > max on either axis.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::new(
    ///     Size::new(50, 25),
    ///     Size::new(200, 100),
    /// );
    /// ```
    pub fn new(min: Size, max: Size) -> Self {
        debug_assert!(
            min.width <= max.width,
            "min.width ({}) must be <= max.width ({})",
            min.width,
            max.width
        );
        debug_assert!(
            min.height <= max.height,
            "min.height ({}) must be <= max.height ({})",
            min.height,
            max.height
        );

        Self { min, max }
    }

    /// Create tight constraints (exact size).
    ///
    /// The component must be exactly this size. This is equivalent to
    /// setting min == max.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::tight(Size::new(100, 50));
    /// assert!(constraints.is_tight());
    /// assert_eq!(constraints.min, constraints.max);
    /// ```
    pub fn tight(size: Size) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    /// Create loose constraints (zero to max).
    ///
    /// The component can be any size from zero up to the given maximum.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::loose(Size::new(200, 100));
    /// assert_eq!(constraints.min, Size::zero());
    /// assert_eq!(constraints.max, Size::new(200, 100));
    /// ```
    pub fn loose(max: Size) -> Self {
        Self {
            min: Size::zero(),
            max,
        }
    }

    /// Create constraints that expand to fill available space.
    ///
    /// Both min and max are set to the given size, forcing the component
    /// to expand to fill the available space.
    ///
    /// This is an alias for [`tight`](Self::tight).
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::expand(Size::new(200, 100));
    /// assert_eq!(Constraints::tight(Size::new(200, 100)), constraints);
    /// ```
    pub fn expand(size: Size) -> Self {
        Self::tight(size)
    }

    /// Constrain a size to be within these constraints.
    ///
    /// Clamps the given size to the valid range defined by min and max.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::new(
    ///     Size::new(50, 25),
    ///     Size::new(200, 100),
    /// );
    ///
    /// // Too small - clamped to min
    /// assert_eq!(constraints.constrain(Size::new(10, 10)), Size::new(50, 25));
    ///
    /// // Too large - clamped to max
    /// assert_eq!(constraints.constrain(Size::new(300, 300)), Size::new(200, 100));
    ///
    /// // Within range - unchanged
    /// assert_eq!(constraints.constrain(Size::new(100, 50)), Size::new(100, 50));
    /// ```
    pub fn constrain(&self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min.width, self.max.width),
            size.height.clamp(self.min.height, self.max.height),
        )
    }

    /// Check if these constraints are tight (min == max).
    ///
    /// Tight constraints force the component to be an exact size.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// assert!(Constraints::tight(Size::new(100, 50)).is_tight());
    /// assert!(!Constraints::loose(Size::new(100, 50)).is_tight());
    /// ```
    pub fn is_tight(&self) -> bool {
        self.min == self.max
    }

    /// Check if these constraints have zero minimum size.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// assert!(Constraints::loose(Size::new(100, 50)).has_zero_min());
    /// assert!(!Constraints::tight(Size::new(100, 50)).has_zero_min());
    /// ```
    pub fn has_zero_min(&self) -> bool {
        self.min == Size::zero()
    }

    /// Get the biggest size allowed by these constraints.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::new(
    ///     Size::new(50, 25),
    ///     Size::new(200, 100),
    /// );
    /// assert_eq!(constraints.biggest(), Size::new(200, 100));
    /// ```
    pub fn biggest(&self) -> Size {
        self.max
    }

    /// Get the smallest size allowed by these constraints.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::new(
    ///     Size::new(50, 25),
    ///     Size::new(200, 100),
    /// );
    /// assert_eq!(constraints.smallest(), Size::new(50, 25));
    /// ```
    pub fn smallest(&self) -> Size {
        self.min
    }

    /// Tighten the minimum constraints.
    ///
    /// Updates the minimum size while preserving the maximum.
    /// The new minimum is clamped to not exceed the maximum.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::loose(Size::new(200, 100))
    ///     .tighten_min(Size::new(50, 25));
    ///
    /// assert_eq!(constraints.min, Size::new(50, 25));
    /// assert_eq!(constraints.max, Size::new(200, 100));
    /// ```
    pub fn tighten_min(mut self, min: Size) -> Self {
        self.min = self.constrain(min);
        self
    }

    /// Loosen the maximum constraints.
    ///
    /// Updates the maximum size while preserving the minimum.
    /// The new maximum is clamped to not be less than the minimum.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::tight(Size::new(100, 50))
    ///     .loosen_max(Size::new(200, 100));
    ///
    /// assert_eq!(constraints.min, Size::new(100, 50));
    /// assert_eq!(constraints.max, Size::new(200, 100));
    /// ```
    pub fn loosen_max(mut self, max: Size) -> Self {
        self.max = Size::new(
            max.width.max(self.min.width),
            max.height.max(self.min.height),
        );
        self
    }

    /// Shrink the constraints by the given edges.
    ///
    /// Reduces both min and max by the given amount, clamping to zero
    /// if the result would be negative.
    ///
    /// Useful for implementing padding or margins.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::tight(Size::new(100, 50));
    /// let shrunk = constraints.deflate(Size::new(20, 10));
    ///
    /// assert_eq!(shrunk.max, Size::new(80, 40));
    /// ```
    pub fn deflate(&self, amount: Size) -> Self {
        Self {
            min: Size::new(
                self.min.width.saturating_sub(amount.width),
                self.min.height.saturating_sub(amount.height),
            ),
            max: Size::new(
                self.max.width.saturating_sub(amount.width),
                self.max.height.saturating_sub(amount.height),
            ),
        }
    }

    /// Expand the constraints by the given edges.
    ///
    /// Increases both min and max by the given amount.
    ///
    /// Useful for implementing padding or margins.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::Constraints;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let constraints = Constraints::tight(Size::new(100, 50));
    /// let expanded = constraints.inflate(Size::new(20, 10));
    ///
    /// assert_eq!(expanded.max, Size::new(120, 60));
    /// ```
    pub fn inflate(&self, amount: Size) -> Self {
        Self {
            min: Size::new(
                self.min.width.saturating_add(amount.width),
                self.min.height.saturating_add(amount.height),
            ),
            max: Size::new(
                self.max.width.saturating_add(amount.width),
                self.max.height.saturating_add(amount.height),
            ),
        }
    }
}

/// Layout information for a single child component.
///
/// Contains the child's position relative to its parent and its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildLayout {
    /// Position offset from parent's top-left corner.
    pub offset: Point,
    /// Size of the child.
    pub size: Size,
}

impl ChildLayout {
    /// Create a new child layout.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::ChildLayout;
    /// use embedded_graphics::prelude::{Point, Size};
    ///
    /// let child = ChildLayout::new(Point::new(10, 20), Size::new(100, 50));
    /// assert_eq!(child.offset, Point::new(10, 20));
    /// assert_eq!(child.size, Size::new(100, 50));
    /// ```
    pub fn new(offset: Point, size: Size) -> Self {
        Self { offset, size }
    }

    /// Get the bounding rectangle of this child.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::ChildLayout;
    /// use embedded_graphics::prelude::{Point, Size};
    ///
    /// let child = ChildLayout::new(Point::new(10, 20), Size::new(100, 50));
    /// let bounds = child.bounds();
    ///
    /// assert_eq!(bounds.top_left, Point::new(10, 20));
    /// assert_eq!(bounds.size, Size::new(100, 50));
    /// ```
    pub fn bounds(&self) -> embedded_graphics::primitives::Rectangle {
        embedded_graphics::primitives::Rectangle::new(self.offset, self.size)
    }
}

/// Result of a layout operation.
///
/// Contains the component's size and the positions of its children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutResult {
    /// Total size of this component.
    pub size: Size,
    /// Layout information for each child.
    ///
    /// Uses `heapless::Vec` for `no_std` compatibility.
    /// Limited to [`MAX_CHILDREN`] children.
    pub children: Vec<ChildLayout, MAX_CHILDREN>,
    /// Debug metadata for this component
    #[cfg(feature = "debug")]
    pub debug_info: Option<DebugInfo>,
}

impl LayoutResult {
    /// Create a new layout result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::{LayoutResult, ChildLayout};
    /// use embedded_graphics::prelude::{Size, Point};
    /// use heapless::Vec;
    ///
    /// let mut children = Vec::new();
    /// children.push(ChildLayout::new(Point::new(0, 0), Size::new(50, 25))).unwrap();
    /// children.push(ChildLayout::new(Point::new(0, 25), Size::new(50, 25))).unwrap();
    ///
    /// let result = LayoutResult::new(Size::new(50, 50), children);
    /// assert_eq!(result.children.len(), 2);
    /// ```
    pub fn new(size: Size, children: Vec<ChildLayout, MAX_CHILDREN>) -> Self {
        Self {
            size,
            children,
            #[cfg(feature = "debug")]
            debug_info: None,
        }
    }

    /// Create a leaf layout result (no children).
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::LayoutResult;
    /// use embedded_graphics::prelude::Size;
    ///
    /// let result = LayoutResult::leaf(Size::new(100, 50));
    /// assert_eq!(result.size, Size::new(100, 50));
    /// assert_eq!(result.children.len(), 0);
    /// ```
    pub fn leaf(size: Size) -> Self {
        Self {
            size,
            children: Vec::new(),
            #[cfg(feature = "debug")]
            debug_info: None,
        }
    }

    /// Add a child to this layout result.
    ///
    /// # Errors
    ///
    /// Returns `Err(child)` if the maximum number of children ([`MAX_CHILDREN`])
    /// has been reached.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::{LayoutResult, ChildLayout};
    /// use embedded_graphics::prelude::{Size, Point};
    ///
    /// let mut result = LayoutResult::leaf(Size::new(100, 100));
    /// result.add_child(ChildLayout::new(Point::new(0, 0), Size::new(50, 50))).unwrap();
    ///
    /// assert_eq!(result.children.len(), 1);
    /// ```
    pub fn add_child(&mut self, child: ChildLayout) -> Result<(), ChildLayout> {
        self.children.push(child)
    }

    /// Get the number of children in this layout.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::{LayoutResult, ChildLayout};
    /// use embedded_graphics::prelude::{Size, Point};
    ///
    /// let mut result = LayoutResult::leaf(Size::new(100, 100));
    /// assert_eq!(result.child_count(), 0);
    ///
    /// result.add_child(ChildLayout::new(Point::new(0, 0), Size::new(50, 50))).unwrap();
    /// assert_eq!(result.child_count(), 1);
    /// ```
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Check if this layout has any children.
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::{LayoutResult, ChildLayout};
    /// use embedded_graphics::prelude::{Size, Point};
    ///
    /// let mut result = LayoutResult::leaf(Size::new(100, 100));
    /// assert!(result.is_leaf());
    ///
    /// result.add_child(ChildLayout::new(Point::new(0, 0), Size::new(50, 50))).unwrap();
    /// assert!(!result.is_leaf());
    /// ```
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// Trait for components that can participate in layout.
///
/// Implementors take constraints and return their size along with
/// the positions of their children.
///
/// # Example
///
/// ```rust
/// use eink_system::layout::{Layout, Constraints, LayoutResult};
/// use embedded_graphics::prelude::Size;
///
/// struct FixedSizeBox {
///     width: u32,
///     height: u32,
/// }
///
/// impl Layout for FixedSizeBox {
///     fn layout(&self, constraints: Constraints) -> LayoutResult {
///         let size = constraints.constrain(Size::new(self.width, self.height));
///         LayoutResult::leaf(size)
///     }
/// }
/// ```
pub trait Layout {
    /// Compute the layout for this component given constraints.
    ///
    /// # Contract
    ///
    /// - The returned size MUST satisfy the given constraints
    /// - All child positions MUST be within the returned size
    /// - Child positions are relative to the component's top-left corner (0, 0)
    ///
    /// # Example
    ///
    /// ```rust
    /// use eink_system::layout::{Layout, Constraints, LayoutResult};
    /// use embedded_graphics::prelude::Size;
    ///
    /// struct MyComponent;
    ///
    /// impl Layout for MyComponent {
    ///     fn layout(&self, constraints: Constraints) -> LayoutResult {
    ///         // Always take maximum available space
    ///         let size = constraints.biggest();
    ///         LayoutResult::leaf(size)
    ///     }
    /// }
    /// ```
    fn layout(&self, constraints: Constraints) -> LayoutResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraints_new() {
        let c = Constraints::new(Size::new(10, 20), Size::new(100, 200));
        assert_eq!(c.min, Size::new(10, 20));
        assert_eq!(c.max, Size::new(100, 200));
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn test_constraints_invalid_width() {
        // Should panic in debug mode when min.width > max.width
        Constraints::new(Size::new(100, 20), Size::new(10, 200));
    }

    #[test]
    #[should_panic]
    #[cfg(debug_assertions)]
    fn test_constraints_invalid_height() {
        // Should panic in debug mode when min.height > max.height
        Constraints::new(Size::new(10, 200), Size::new(100, 20));
    }

    #[test]
    fn test_constraints_tight() {
        let c = Constraints::tight(Size::new(100, 50));
        assert_eq!(c.min, Size::new(100, 50));
        assert_eq!(c.max, Size::new(100, 50));
        assert!(c.is_tight());
    }

    #[test]
    fn test_constraints_loose() {
        let c = Constraints::loose(Size::new(100, 50));
        assert_eq!(c.min, Size::zero());
        assert_eq!(c.max, Size::new(100, 50));
        assert!(!c.is_tight());
        assert!(c.has_zero_min());
    }

    #[test]
    fn test_constraints_expand() {
        let c = Constraints::expand(Size::new(100, 50));
        assert_eq!(c, Constraints::tight(Size::new(100, 50)));
        assert!(c.is_tight());
    }

    #[test]
    fn test_constraints_constrain() {
        let c = Constraints::new(Size::new(50, 25), Size::new(200, 100));

        // Too small - clamped to min
        assert_eq!(c.constrain(Size::new(10, 10)), Size::new(50, 25));

        // Too large - clamped to max
        assert_eq!(c.constrain(Size::new(300, 300)), Size::new(200, 100));

        // Within range - unchanged
        assert_eq!(c.constrain(Size::new(100, 50)), Size::new(100, 50));

        // Mixed - width too small, height OK
        assert_eq!(c.constrain(Size::new(10, 50)), Size::new(50, 50));
    }

    #[test]
    fn test_constraints_is_tight() {
        assert!(Constraints::tight(Size::new(100, 50)).is_tight());
        assert!(!Constraints::loose(Size::new(100, 50)).is_tight());
        assert!(!Constraints::new(Size::new(50, 25), Size::new(100, 50)).is_tight());
    }

    #[test]
    fn test_constraints_has_zero_min() {
        assert!(Constraints::loose(Size::new(100, 50)).has_zero_min());
        assert!(!Constraints::tight(Size::new(100, 50)).has_zero_min());
        assert!(!Constraints::new(Size::new(50, 25), Size::new(100, 50)).has_zero_min());
    }

    #[test]
    fn test_constraints_biggest() {
        let c = Constraints::new(Size::new(50, 25), Size::new(200, 100));
        assert_eq!(c.biggest(), Size::new(200, 100));
    }

    #[test]
    fn test_constraints_smallest() {
        let c = Constraints::new(Size::new(50, 25), Size::new(200, 100));
        assert_eq!(c.smallest(), Size::new(50, 25));
    }

    #[test]
    fn test_constraints_tighten_min() {
        let c = Constraints::loose(Size::new(200, 100)).tighten_min(Size::new(50, 25));
        assert_eq!(c.min, Size::new(50, 25));
        assert_eq!(c.max, Size::new(200, 100));

        // Clamped to max
        let c2 = Constraints::loose(Size::new(100, 50)).tighten_min(Size::new(200, 100));
        assert_eq!(c2.min, Size::new(100, 50));
        assert_eq!(c2.max, Size::new(100, 50));
    }

    #[test]
    fn test_constraints_loosen_max() {
        let c = Constraints::tight(Size::new(100, 50)).loosen_max(Size::new(200, 100));
        assert_eq!(c.min, Size::new(100, 50));
        assert_eq!(c.max, Size::new(200, 100));

        // Clamped to min
        let c2 = Constraints::tight(Size::new(100, 50)).loosen_max(Size::new(50, 25));
        assert_eq!(c2.min, Size::new(100, 50));
        assert_eq!(c2.max, Size::new(100, 50));
    }

    #[test]
    fn test_constraints_deflate() {
        let c = Constraints::tight(Size::new(100, 50));
        let deflated = c.deflate(Size::new(20, 10));

        assert_eq!(deflated.min, Size::new(80, 40));
        assert_eq!(deflated.max, Size::new(80, 40));

        // Clamped to zero
        let c2 = Constraints::tight(Size::new(10, 5));
        let deflated2 = c2.deflate(Size::new(20, 10));
        assert_eq!(deflated2.min, Size::new(0, 0));
        assert_eq!(deflated2.max, Size::new(0, 0));
    }

    #[test]
    fn test_constraints_inflate() {
        let c = Constraints::tight(Size::new(100, 50));
        let inflated = c.inflate(Size::new(20, 10));

        assert_eq!(inflated.min, Size::new(120, 60));
        assert_eq!(inflated.max, Size::new(120, 60));
    }

    #[test]
    fn test_child_layout_new() {
        let child = ChildLayout::new(Point::new(10, 20), Size::new(100, 50));
        assert_eq!(child.offset, Point::new(10, 20));
        assert_eq!(child.size, Size::new(100, 50));
    }

    #[test]
    fn test_child_layout_bounds() {
        let child = ChildLayout::new(Point::new(10, 20), Size::new(100, 50));
        let bounds = child.bounds();

        assert_eq!(bounds.top_left, Point::new(10, 20));
        assert_eq!(bounds.size, Size::new(100, 50));
    }

    #[test]
    fn test_layout_result_new() {
        let mut children = Vec::new();
        children
            .push(ChildLayout::new(Point::new(0, 0), Size::new(50, 25)))
            .unwrap();
        children
            .push(ChildLayout::new(Point::new(0, 25), Size::new(50, 25)))
            .unwrap();

        let result = LayoutResult::new(Size::new(50, 50), children);
        assert_eq!(result.size, Size::new(50, 50));
        assert_eq!(result.children.len(), 2);
    }

    #[test]
    fn test_layout_result_leaf() {
        let result = LayoutResult::leaf(Size::new(100, 50));
        assert_eq!(result.size, Size::new(100, 50));
        assert_eq!(result.children.len(), 0);
        assert!(result.is_leaf());
    }

    #[test]
    fn test_layout_result_add_child() {
        let mut result = LayoutResult::leaf(Size::new(100, 100));

        assert_eq!(result.child_count(), 0);
        assert!(result.is_leaf());

        result
            .add_child(ChildLayout::new(Point::new(0, 0), Size::new(50, 50)))
            .unwrap();

        assert_eq!(result.child_count(), 1);
        assert!(!result.is_leaf());

        result
            .add_child(ChildLayout::new(Point::new(50, 0), Size::new(50, 50)))
            .unwrap();

        assert_eq!(result.child_count(), 2);
    }

    #[test]
    fn test_layout_result_max_children() {
        let mut result = LayoutResult::leaf(Size::new(1000, 1000));

        // Add MAX_CHILDREN children
        for i in 0..MAX_CHILDREN {
            result
                .add_child(ChildLayout::new(Point::new(i as i32, 0), Size::new(10, 10)))
                .expect("Should fit within MAX_CHILDREN");
        }

        assert_eq!(result.child_count(), MAX_CHILDREN);

        // Adding one more should fail
        let overflow_result =
            result.add_child(ChildLayout::new(Point::new(0, 0), Size::new(10, 10)));
        assert!(overflow_result.is_err());
    }

    #[test]
    fn test_layout_trait_basic() {
        struct FixedBox {
            width: u32,
            height: u32,
        }

        impl Layout for FixedBox {
            fn layout(&self, constraints: Constraints) -> LayoutResult {
                let size = constraints.constrain(Size::new(self.width, self.height));
                LayoutResult::leaf(size)
            }
        }

        let component = FixedBox {
            width: 100,
            height: 50,
        };

        // Unconstrained
        let result = component.layout(Constraints::loose(Size::new(200, 100)));
        assert_eq!(result.size, Size::new(100, 50));

        // Too small - should clamp to min
        let result = component.layout(Constraints::new(Size::new(150, 75), Size::new(200, 100)));
        assert_eq!(result.size, Size::new(150, 75));

        // Exact fit
        let result = component.layout(Constraints::tight(Size::new(100, 50)));
        assert_eq!(result.size, Size::new(100, 50));
    }

    #[test]
    fn test_layout_trait_container() {
        struct HStack {
            child_sizes: &'static [Size],
        }

        impl Layout for HStack {
            fn layout(&self, constraints: Constraints) -> LayoutResult {
                let mut children = Vec::new();
                let mut x = 0;

                for &child_size in self.child_sizes {
                    children
                        .push(ChildLayout::new(Point::new(x, 0), child_size))
                        .unwrap();
                    x += child_size.width as i32;
                }

                let total_width = self.child_sizes.iter().map(|s| s.width).sum();
                let max_height = self.child_sizes.iter().map(|s| s.height).max().unwrap_or(0);

                let size = constraints.constrain(Size::new(total_width, max_height));

                LayoutResult::new(size, children)
            }
        }

        const CHILD_SIZES: &[Size] = &[Size::new(50, 30), Size::new(70, 40), Size::new(30, 20)];
        let stack = HStack {
            child_sizes: CHILD_SIZES,
        };

        let result = stack.layout(Constraints::loose(Size::new(200, 100)));

        assert_eq!(result.size, Size::new(150, 40)); // Total width, max height
        assert_eq!(result.child_count(), 3);

        assert_eq!(result.children[0].offset, Point::new(0, 0));
        assert_eq!(result.children[0].size, Size::new(50, 30));

        assert_eq!(result.children[1].offset, Point::new(50, 0));
        assert_eq!(result.children[1].size, Size::new(70, 40));

        assert_eq!(result.children[2].offset, Point::new(120, 0));
        assert_eq!(result.children[2].size, Size::new(30, 20));
    }

    #[test]
    fn test_constraints_chaining() {
        let c = Constraints::loose(Size::new(200, 100))
            .tighten_min(Size::new(50, 25))
            .deflate(Size::new(10, 5))
            .inflate(Size::new(5, 2));

        assert_eq!(c.min, Size::new(45, 22));
        assert_eq!(c.max, Size::new(195, 97));
    }

    #[test]
    fn test_zero_size_layout() {
        let result = LayoutResult::leaf(Size::zero());
        assert_eq!(result.size, Size::zero());
        assert!(result.is_leaf());
    }

    #[test]
    fn test_child_layout_at_origin() {
        let child = ChildLayout::new(Point::zero(), Size::new(100, 50));
        let bounds = child.bounds();

        assert_eq!(bounds.top_left, Point::zero());
        assert_eq!(bounds.size, Size::new(100, 50));
    }

    #[test]
    fn test_constraints_equality() {
        let c1 = Constraints::tight(Size::new(100, 50));
        let c2 = Constraints::tight(Size::new(100, 50));
        let c3 = Constraints::tight(Size::new(100, 51));

        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }

    #[test]
    fn test_layout_result_equality() {
        let r1 = LayoutResult::leaf(Size::new(100, 50));
        let r2 = LayoutResult::leaf(Size::new(100, 50));
        let r3 = LayoutResult::leaf(Size::new(100, 51));

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);
    }
}
