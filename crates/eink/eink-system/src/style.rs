//! Styling system for e-ink layouts.
//!
//! This module provides CSS-inspired styling primitives for flexbox-based layouts.
//! All types are `no_std` compatible and designed for embedded use.
//!
//! # Core Types
//!
//! - [`Dimension`] - Size values (Auto, Px, Percent)
//! - [`Edges`] - Edge insets for margins/padding (top, right, bottom, left)
//! - [`FlexDirection`] - Flexbox main axis direction
//! - [`Justify`] - Main axis alignment
//! - [`Align`] - Cross axis alignment
//! - [`Style`] - Complete styling configuration
//!
//! # Example
//!
//! ```
//! use eink_system::style::*;
//! use embedded_graphics::pixelcolor::Gray4;
//! use embedded_graphics::pixelcolor::GrayColor;
//!
//! let style = Style {
//!     width: Dimension::Px(200),
//!     height: Dimension::Auto,
//!     margin: Edges::all(8),
//!     padding: Edges::uniform(16),
//!     flex_direction: FlexDirection::Column,
//!     justify_content: Justify::SpaceBetween,
//!     align_items: Align::Stretch,
//!     gap: 12,
//!     flex_grow: 0.0,
//!     flex_shrink: 1.0,
//!     flex_basis: Dimension::Auto,
//!     background: Some(Gray4::WHITE),
//! };
//! ```

use embedded_graphics::pixelcolor::Gray4;

/// Size dimension with support for automatic sizing, pixels, and percentages.
///
/// # Examples
///
/// ```
/// use eink_system::style::Dimension;
///
/// let auto = Dimension::Auto;
/// let fixed = Dimension::Px(100);
/// let responsive = Dimension::Percent(50.0);
///
/// // Resolve to concrete pixel value
/// assert_eq!(auto.resolve(200), 0);
/// assert_eq!(fixed.resolve(200), 100);
/// assert_eq!(responsive.resolve(200), 100);
/// ```
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub enum Dimension {
    /// Automatic sizing based on content.
    #[default]
    Auto,

    /// Fixed pixel size.
    Px(u32),

    /// Percentage of available space (0.0 to 100.0).
    Percent(f32),
}

impl Dimension {
    /// Resolves the dimension to a concrete pixel value.
    ///
    /// # Arguments
    ///
    /// * `available_space` - The available space in pixels
    ///
    /// # Returns
    ///
    /// The resolved pixel value. For `Auto`, returns 0 (caller must compute based on content).
    ///
    /// # Examples
    ///
    /// ```
    /// use eink_system::style::Dimension;
    ///
    /// assert_eq!(Dimension::Px(50).resolve(100), 50);
    /// assert_eq!(Dimension::Percent(75.0).resolve(100), 75);
    /// assert_eq!(Dimension::Auto.resolve(100), 0);
    /// ```
    pub fn resolve(self, available_space: u32) -> u32 {
        match self {
            Dimension::Auto => 0,
            Dimension::Px(px) => px,
            Dimension::Percent(pct) => {
                let pct = pct.clamp(0.0, 100.0);
                ((available_space as f32) * (pct / 100.0)) as u32
            }
        }
    }

    /// Returns `true` if this dimension is `Auto`.
    pub fn is_auto(self) -> bool {
        matches!(self, Dimension::Auto)
    }

    /// Returns `true` if this dimension is a fixed pixel value.
    pub fn is_fixed(self) -> bool {
        matches!(self, Dimension::Px(_))
    }

    /// Returns `true` if this dimension is a percentage.
    pub fn is_percent(self) -> bool {
        matches!(self, Dimension::Percent(_))
    }
}

/// Edge insets for margins, padding, and borders.
///
/// Follows CSS box model convention: top, right, bottom, left.
///
/// # Examples
///
/// ```
/// use eink_system::style::Edges;
///
/// // All edges the same
/// let uniform = Edges::all(16);
/// assert_eq!(uniform.top, 16);
/// assert_eq!(uniform.right, 16);
///
/// // Uniform (same as all)
/// let uniform2 = Edges::uniform(8);
///
/// // Individual edges
/// let custom = Edges::new(10, 20, 10, 20);
///
/// // Horizontal and vertical shortcuts
/// let hv = Edges::horizontal_vertical(24, 12);
/// assert_eq!(hv.left, 24);
/// assert_eq!(hv.right, 24);
/// assert_eq!(hv.top, 12);
/// assert_eq!(hv.bottom, 12);
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Edges {
    /// Top edge inset in pixels.
    pub top: u32,

    /// Right edge inset in pixels.
    pub right: u32,

    /// Bottom edge inset in pixels.
    pub bottom: u32,

    /// Left edge inset in pixels.
    pub left: u32,
}

impl Edges {
    /// Creates edges with all sides set to the same value.
    ///
    /// # Examples
    ///
    /// ```
    /// use eink_system::style::Edges;
    ///
    /// let edges = Edges::all(16);
    /// assert_eq!(edges.top, 16);
    /// assert_eq!(edges.right, 16);
    /// assert_eq!(edges.bottom, 16);
    /// assert_eq!(edges.left, 16);
    /// ```
    pub const fn all(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Creates edges with all sides set to the same value.
    ///
    /// Alias for [`all`](Self::all) for CSS-like naming.
    pub const fn uniform(value: u32) -> Self {
        Self::all(value)
    }

    /// Creates edges with individual values for each side.
    ///
    /// # Arguments
    ///
    /// * `top` - Top edge value
    /// * `right` - Right edge value
    /// * `bottom` - Bottom edge value
    /// * `left` - Left edge value
    pub const fn new(top: u32, right: u32, bottom: u32, left: u32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates edges with separate horizontal and vertical values.
    ///
    /// # Arguments
    ///
    /// * `horizontal` - Left and right edge value
    /// * `vertical` - Top and bottom edge value
    ///
    /// # Examples
    ///
    /// ```
    /// use eink_system::style::Edges;
    ///
    /// let edges = Edges::horizontal_vertical(20, 10);
    /// assert_eq!(edges.left, 20);
    /// assert_eq!(edges.right, 20);
    /// assert_eq!(edges.top, 10);
    /// assert_eq!(edges.bottom, 10);
    /// ```
    pub const fn horizontal_vertical(horizontal: u32, vertical: u32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Returns the total horizontal space (left + right).
    // SAFETY: edge values are display pixel counts (max ~4000); left + right cannot overflow u32.
    #[allow(clippy::arithmetic_side_effects)]
    pub const fn horizontal(self) -> u32 {
        self.left + self.right
    }

    /// Returns the total vertical space (top + bottom).
    // SAFETY: edge values are display pixel counts (max ~4000); top + bottom cannot overflow u32.
    #[allow(clippy::arithmetic_side_effects)]
    pub const fn vertical(self) -> u32 {
        self.top + self.bottom
    }
}

impl Default for Edges {
    fn default() -> Self {
        Self::all(0)
    }
}

/// Flexbox direction defining the main axis.
///
/// Controls the primary direction in which children are laid out.
///
/// # Examples
///
/// ```
/// use eink_system::style::FlexDirection;
///
/// // Horizontal layout (left to right)
/// let row = FlexDirection::Row;
///
/// // Vertical layout (top to bottom)
/// let column = FlexDirection::Column;
///
/// // Reversed directions
/// let row_rev = FlexDirection::RowReverse;
/// let col_rev = FlexDirection::ColumnReverse;
/// ```
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum FlexDirection {
    /// Horizontal layout, left to right.
    #[default]
    Row,

    /// Vertical layout, top to bottom.
    Column,

    /// Horizontal layout, right to left.
    RowReverse,

    /// Vertical layout, bottom to top.
    ColumnReverse,
}

impl FlexDirection {
    /// Returns `true` if this is a row direction (Row or RowReverse).
    pub fn is_row(self) -> bool {
        matches!(self, FlexDirection::Row | FlexDirection::RowReverse)
    }

    /// Returns `true` if this is a column direction (Column or ColumnReverse).
    pub fn is_column(self) -> bool {
        matches!(self, FlexDirection::Column | FlexDirection::ColumnReverse)
    }

    /// Returns `true` if this direction is reversed.
    pub fn is_reverse(self) -> bool {
        matches!(
            self,
            FlexDirection::RowReverse | FlexDirection::ColumnReverse
        )
    }
}

/// Main axis alignment (justify-content in CSS).
///
/// Controls how children are distributed along the main axis.
///
/// # Examples
///
/// ```
/// use eink_system::style::Justify;
///
/// // Pack children at start
/// let start = Justify::Start;
///
/// // Distribute evenly with space between
/// let between = Justify::SpaceBetween;
///
/// // Center all children
/// let center = Justify::Center;
/// ```
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum Justify {
    /// Pack children at the start of the main axis.
    #[default]
    Start,

    /// Pack children at the end of the main axis.
    End,

    /// Center children along the main axis.
    Center,

    /// Distribute children with space between them.
    ///
    /// First child at start, last child at end, equal spacing between.
    SpaceBetween,

    /// Distribute children with space around them.
    ///
    /// Equal spacing on both sides of each child. Edge spacing is half of inter-child spacing.
    SpaceAround,

    /// Distribute children with equal space between and around them.
    ///
    /// All spacing (edge and inter-child) is equal.
    SpaceEvenly,
}

/// Cross axis alignment (align-items in CSS).
///
/// Controls how children are aligned along the cross axis.
///
/// # Examples
///
/// ```
/// use eink_system::style::Align;
///
/// // Align to start of cross axis
/// let start = Align::Start;
///
/// // Stretch to fill cross axis
/// let stretch = Align::Stretch;
///
/// // Center on cross axis
/// let center = Align::Center;
/// ```
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum Align {
    /// Align children at the start of the cross axis.
    #[default]
    Start,

    /// Align children at the end of the cross axis.
    End,

    /// Center children along the cross axis.
    Center,

    /// Stretch children to fill the cross axis.
    Stretch,

    /// Align children along their baseline.
    Baseline,
}

/// Complete style specification for a layout node.
///
/// Contains all properties needed for flexbox layout calculation and rendering.
///
/// # Examples
///
/// ```
/// use eink_system::style::*;
/// use embedded_graphics::pixelcolor::Gray4;
/// use embedded_graphics::pixelcolor::GrayColor;
///
/// // Create a centered container with padding
/// let container = Style {
///     width: Dimension::Percent(100.0),
///     height: Dimension::Auto,
///     margin: Edges::all(0),
///     padding: Edges::all(16),
///     flex_direction: FlexDirection::Column,
///     justify_content: Justify::Center,
///     align_items: Align::Center,
///     gap: 8,
///     flex_grow: 0.0,
///     flex_shrink: 1.0,
///     flex_basis: Dimension::Auto,
///     background: Some(Gray4::WHITE),
/// };
///
/// // Create a row with space between
/// let row = Style {
///     flex_direction: FlexDirection::Row,
///     justify_content: Justify::SpaceBetween,
///     ..Style::default()
/// };
/// ```
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Style {
    /// Width of the element.
    pub width: Dimension,

    /// Height of the element.
    pub height: Dimension,

    /// Margin (outer spacing).
    pub margin: Edges,

    /// Padding (inner spacing).
    pub padding: Edges,

    /// Flexbox direction (main axis).
    pub flex_direction: FlexDirection,

    /// Main axis alignment.
    pub justify_content: Justify,

    /// Cross axis alignment.
    pub align_items: Align,

    /// Gap between children in pixels.
    pub gap: u32,

    /// Flex grow factor (for distributing extra space).
    pub flex_grow: f32,

    /// Flex shrink factor (for absorbing negative space).
    pub flex_shrink: f32,

    /// Initial main axis size before flex adjustments.
    pub flex_basis: Dimension,

    /// Background color (optional).
    pub background: Option<Gray4>,
}

impl Style {
    /// Creates a new style with all default values.
    pub const fn new() -> Self {
        Self {
            width: Dimension::Auto,
            height: Dimension::Auto,
            margin: Edges::all(0),
            padding: Edges::all(0),
            flex_direction: FlexDirection::Row,
            justify_content: Justify::Start,
            align_items: Align::Start,
            gap: 0,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Dimension::Auto,
            background: None,
        }
    }

    /// Builder method to set width.
    pub const fn width(mut self, width: Dimension) -> Self {
        self.width = width;
        self
    }

    /// Builder method to set height.
    pub const fn height(mut self, height: Dimension) -> Self {
        self.height = height;
        self
    }

    /// Builder method to set margin.
    pub const fn margin(mut self, margin: Edges) -> Self {
        self.margin = margin;
        self
    }

    /// Builder method to set padding.
    pub const fn padding(mut self, padding: Edges) -> Self {
        self.padding = padding;
        self
    }

    /// Builder method to set flex direction.
    pub const fn flex_direction(mut self, direction: FlexDirection) -> Self {
        self.flex_direction = direction;
        self
    }

    /// Builder method to set justify content.
    pub const fn justify_content(mut self, justify: Justify) -> Self {
        self.justify_content = justify;
        self
    }

    /// Builder method to set align items.
    pub const fn align_items(mut self, align: Align) -> Self {
        self.align_items = align;
        self
    }

    /// Builder method to set gap.
    pub const fn gap(mut self, gap: u32) -> Self {
        self.gap = gap;
        self
    }

    /// Builder method to set background color.
    pub const fn background(mut self, background: Gray4) -> Self {
        self.background = Some(background);
        self
    }

    /// Returns the total horizontal space taken by margin and padding.
    // SAFETY: margin and padding are pixel counts; their sum fits trivially in u32.
    #[allow(clippy::arithmetic_side_effects)]
    pub const fn horizontal_space(self) -> u32 {
        self.margin.horizontal() + self.padding.horizontal()
    }

    /// Returns the total vertical space taken by margin and padding.
    // SAFETY: margin and padding are pixel counts; their sum fits trivially in u32.
    #[allow(clippy::arithmetic_side_effects)]
    pub const fn vertical_space(self) -> u32 {
        self.margin.vertical() + self.padding.vertical()
    }

    /// Returns the content width after subtracting padding and margin.
    ///
    /// If width is Auto, returns None.
    pub fn content_width(self, available_width: u32) -> Option<u32> {
        if self.width.is_auto() {
            return None;
        }

        let total_width = self.width.resolve(available_width);
        let space = self.horizontal_space();

        Some(total_width.saturating_sub(space))
    }

    /// Returns the content height after subtracting padding and margin.
    ///
    /// If height is Auto, returns None.
    pub fn content_height(self, available_height: u32) -> Option<u32> {
        if self.height.is_auto() {
            return None;
        }

        let total_height = self.height.resolve(available_height);
        let space = self.vertical_space();

        Some(total_height.saturating_sub(space))
    }
}

impl Default for Style {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::GrayColor;

    #[test]
    fn test_dimension_auto() {
        let dim = Dimension::Auto;
        assert!(dim.is_auto());
        assert!(!dim.is_fixed());
        assert!(!dim.is_percent());
        assert_eq!(dim.resolve(100), 0);
        assert_eq!(dim.resolve(500), 0);
    }

    #[test]
    fn test_dimension_px() {
        let dim = Dimension::Px(50);
        assert!(!dim.is_auto());
        assert!(dim.is_fixed());
        assert!(!dim.is_percent());
        assert_eq!(dim.resolve(100), 50);
        assert_eq!(dim.resolve(1000), 50);
    }

    #[test]
    fn test_dimension_percent() {
        let dim = Dimension::Percent(50.0);
        assert!(!dim.is_auto());
        assert!(!dim.is_fixed());
        assert!(dim.is_percent());
        assert_eq!(dim.resolve(100), 50);
        assert_eq!(dim.resolve(200), 100);
        assert_eq!(dim.resolve(1000), 500);
    }

    #[test]
    fn test_dimension_percent_clamping() {
        // Over 100%
        let over = Dimension::Percent(150.0);
        assert_eq!(over.resolve(100), 100);

        // Negative (clamped to 0)
        let negative = Dimension::Percent(-50.0);
        assert_eq!(negative.resolve(100), 0);
    }

    #[test]
    fn test_dimension_percent_rounding() {
        let dim = Dimension::Percent(33.33);
        assert_eq!(dim.resolve(300), 99); // 33.33% of 300 = 99.99 -> 99
    }

    #[test]
    fn test_dimension_default() {
        assert_eq!(Dimension::default(), Dimension::Auto);
    }

    #[test]
    fn test_edges_all() {
        let edges = Edges::all(16);
        assert_eq!(edges.top, 16);
        assert_eq!(edges.right, 16);
        assert_eq!(edges.bottom, 16);
        assert_eq!(edges.left, 16);
    }

    #[test]
    fn test_edges_uniform() {
        let edges = Edges::uniform(8);
        assert_eq!(edges, Edges::all(8));
    }

    #[test]
    fn test_edges_new() {
        let edges = Edges::new(10, 20, 30, 40);
        assert_eq!(edges.top, 10);
        assert_eq!(edges.right, 20);
        assert_eq!(edges.bottom, 30);
        assert_eq!(edges.left, 40);
    }

    #[test]
    fn test_edges_horizontal_vertical() {
        let edges = Edges::horizontal_vertical(24, 12);
        assert_eq!(edges.top, 12);
        assert_eq!(edges.right, 24);
        assert_eq!(edges.bottom, 12);
        assert_eq!(edges.left, 24);
    }

    #[test]
    fn test_edges_horizontal() {
        let edges = Edges::new(0, 10, 0, 20);
        assert_eq!(edges.horizontal(), 30);
    }

    #[test]
    fn test_edges_vertical() {
        let edges = Edges::new(10, 0, 20, 0);
        assert_eq!(edges.vertical(), 30);
    }

    #[test]
    fn test_edges_default() {
        let edges = Edges::default();
        assert_eq!(edges, Edges::all(0));
    }

    #[test]
    fn test_flex_direction_is_row() {
        assert!(FlexDirection::Row.is_row());
        assert!(FlexDirection::RowReverse.is_row());
        assert!(!FlexDirection::Column.is_row());
        assert!(!FlexDirection::ColumnReverse.is_row());
    }

    #[test]
    fn test_flex_direction_is_column() {
        assert!(FlexDirection::Column.is_column());
        assert!(FlexDirection::ColumnReverse.is_column());
        assert!(!FlexDirection::Row.is_column());
        assert!(!FlexDirection::RowReverse.is_column());
    }

    #[test]
    fn test_flex_direction_is_reverse() {
        assert!(FlexDirection::RowReverse.is_reverse());
        assert!(FlexDirection::ColumnReverse.is_reverse());
        assert!(!FlexDirection::Row.is_reverse());
        assert!(!FlexDirection::Column.is_reverse());
    }

    #[test]
    fn test_flex_direction_default() {
        assert_eq!(FlexDirection::default(), FlexDirection::Row);
    }

    #[test]
    fn test_justify_default() {
        assert_eq!(Justify::default(), Justify::Start);
    }

    #[test]
    fn test_align_default() {
        assert_eq!(Align::default(), Align::Start);
    }

    #[test]
    fn test_style_new() {
        let style = Style::new();
        assert_eq!(style.width, Dimension::Auto);
        assert_eq!(style.height, Dimension::Auto);
        assert_eq!(style.margin, Edges::all(0));
        assert_eq!(style.padding, Edges::all(0));
        assert_eq!(style.flex_direction, FlexDirection::Row);
        assert_eq!(style.justify_content, Justify::Start);
        assert_eq!(style.align_items, Align::Start);
        assert_eq!(style.gap, 0);
        assert_eq!(style.background, None);
    }

    #[test]
    fn test_style_default() {
        assert_eq!(Style::default(), Style::new());
    }

    #[test]
    fn test_style_builder_methods() {
        let style = Style::new()
            .width(Dimension::Px(200))
            .height(Dimension::Percent(50.0))
            .margin(Edges::all(8))
            .padding(Edges::all(16))
            .flex_direction(FlexDirection::Column)
            .justify_content(Justify::Center)
            .align_items(Align::Stretch)
            .gap(12)
            .background(Gray4::WHITE);

        assert_eq!(style.width, Dimension::Px(200));
        assert_eq!(style.height, Dimension::Percent(50.0));
        assert_eq!(style.margin, Edges::all(8));
        assert_eq!(style.padding, Edges::all(16));
        assert_eq!(style.flex_direction, FlexDirection::Column);
        assert_eq!(style.justify_content, Justify::Center);
        assert_eq!(style.align_items, Align::Stretch);
        assert_eq!(style.gap, 12);
        assert_eq!(style.background, Some(Gray4::WHITE));
    }

    #[test]
    fn test_style_horizontal_space() {
        let style = Style::new()
            .margin(Edges::horizontal_vertical(10, 0))
            .padding(Edges::horizontal_vertical(5, 0));

        // Left: 10 + 5, Right: 10 + 5 = 30
        assert_eq!(style.horizontal_space(), 30);
    }

    #[test]
    fn test_style_vertical_space() {
        let style = Style::new()
            .margin(Edges::horizontal_vertical(0, 10))
            .padding(Edges::horizontal_vertical(0, 5));

        // Top: 10 + 5, Bottom: 10 + 5 = 30
        assert_eq!(style.vertical_space(), 30);
    }

    #[test]
    fn test_style_content_width_auto() {
        let style = Style::new(); // Auto width
        assert_eq!(style.content_width(100), None);
    }

    #[test]
    fn test_style_content_width_fixed() {
        let style = Style::new()
            .width(Dimension::Px(100))
            .margin(Edges::horizontal_vertical(10, 0))
            .padding(Edges::horizontal_vertical(5, 0));

        // Total: 100, Horizontal space: 30, Content: 70
        assert_eq!(style.content_width(1000), Some(70));
    }

    #[test]
    fn test_style_content_width_percent() {
        let style = Style::new()
            .width(Dimension::Percent(50.0))
            .margin(Edges::horizontal_vertical(5, 0))
            .padding(Edges::horizontal_vertical(5, 0));

        // Total: 50% of 200 = 100, Horizontal space: 20, Content: 80
        assert_eq!(style.content_width(200), Some(80));
    }

    #[test]
    fn test_style_content_width_underflow() {
        let style = Style::new()
            .width(Dimension::Px(10))
            .margin(Edges::all(10))
            .padding(Edges::all(10));

        // Total: 10, Space: 40, should saturate to 0
        assert_eq!(style.content_width(1000), Some(0));
    }

    #[test]
    fn test_style_content_height_auto() {
        let style = Style::new(); // Auto height
        assert_eq!(style.content_height(100), None);
    }

    #[test]
    fn test_style_content_height_fixed() {
        let style = Style::new()
            .height(Dimension::Px(100))
            .margin(Edges::horizontal_vertical(0, 10))
            .padding(Edges::horizontal_vertical(0, 5));

        // Total: 100, Vertical space: 30, Content: 70
        assert_eq!(style.content_height(1000), Some(70));
    }

    #[test]
    fn test_style_content_height_percent() {
        let style = Style::new()
            .height(Dimension::Percent(50.0))
            .margin(Edges::horizontal_vertical(0, 5))
            .padding(Edges::horizontal_vertical(0, 5));

        // Total: 50% of 200 = 100, Vertical space: 20, Content: 80
        assert_eq!(style.content_height(200), Some(80));
    }

    #[test]
    fn test_complex_style_scenario() {
        // Real-world scenario: a card with responsive width
        let card = Style::new()
            .width(Dimension::Percent(90.0))
            .height(Dimension::Auto)
            .margin(Edges::horizontal_vertical(8, 16))
            .padding(Edges::all(12))
            .flex_direction(FlexDirection::Column)
            .justify_content(Justify::SpaceBetween)
            .align_items(Align::Stretch)
            .gap(8)
            .background(Gray4::WHITE);

        // Container width: 320px
        // Card width: 90% = 288px
        // Horizontal space: margin(16) + padding(24) = 40px
        // Content width: 288 - 40 = 248px
        assert_eq!(card.content_width(320), Some(248));

        // Verify other properties
        assert_eq!(card.flex_direction, FlexDirection::Column);
        assert_eq!(card.gap, 8);
    }

    #[test]
    fn test_dimension_equality() {
        assert_eq!(Dimension::Auto, Dimension::Auto);
        assert_eq!(Dimension::Px(50), Dimension::Px(50));
        assert_eq!(Dimension::Percent(75.0), Dimension::Percent(75.0));

        assert_ne!(Dimension::Auto, Dimension::Px(0));
        assert_ne!(Dimension::Px(50), Dimension::Px(51));
    }

    #[test]
    fn test_edges_equality() {
        assert_eq!(Edges::all(8), Edges::all(8));
        assert_eq!(Edges::new(1, 2, 3, 4), Edges::new(1, 2, 3, 4));

        assert_ne!(Edges::all(8), Edges::all(9));
        assert_ne!(Edges::new(1, 2, 3, 4), Edges::new(1, 2, 3, 5));
    }

    #[test]
    fn test_style_clone() {
        let style1 = Style::new().width(Dimension::Px(100)).gap(16);

        let style2 = style1;
        assert_eq!(style1, style2);
    }

    #[test]
    fn test_dimension_debug() {
        let _auto = Dimension::Auto;
        let _px = Dimension::Px(100);
        let _pct = Dimension::Percent(50.0);

        // Debug is derived, just ensure it compiles
    }

    #[test]
    fn test_edges_zero() {
        let zero = Edges::all(0);
        assert_eq!(zero.horizontal(), 0);
        assert_eq!(zero.vertical(), 0);
    }

    #[test]
    fn test_style_no_spacing() {
        let style = Style::new();
        assert_eq!(style.horizontal_space(), 0);
        assert_eq!(style.vertical_space(), 0);
    }
}
