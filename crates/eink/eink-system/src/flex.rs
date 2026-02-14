//! Flexbox layout engine for e-ink displays
//!
//! This module implements a complete flexbox layout algorithm based on the CSS Flexbox
//! specification (https://www.w3.org/TR/css-flexbox-1/), adapted for embedded systems
//! and e-ink displays.
//!
//! # Layout Algorithm
//!
//! The flexbox algorithm follows these steps:
//!
//! 1. **Resolve dimensions**: Convert Auto, Px, and Percent dimensions to concrete sizes
//! 2. **Calculate main axis sizes**: Determine how much space each child needs
//! 3. **Distribute remaining space**: Apply flex-grow and flex-shrink
//! 4. **Apply justification**: Position children along main axis (justify-content)
//! 5. **Apply alignment**: Position children along cross axis (align-items)
//! 6. **Apply gaps**: Insert spacing between children
//! 7. **Apply margins and padding**: Final positioning adjustments
//!
//! # Examples
//!
//! ```
//! use eink_system::flex::{FlexLayout, ChildLayout};
//! use eink_system::style::*;
//! use eink_system::layout::Constraints;
//! use embedded_graphics::geometry::Size;
//!
//! // Create a horizontal layout with space-between justification
//! let mut style = Style::default();
//! style.flex_direction = FlexDirection::Row;
//! style.justify_content = JustifyContent::SpaceBetween;
//! style.gap = 8;
//!
//! let children = vec![
//!     ChildLayout::new(Style::default(), Size::new(50, 50)),
//!     ChildLayout::new(Style::default(), Size::new(50, 50)),
//! ];
//!
//! let layout = FlexLayout::new(style);
//! let result = layout.layout(Constraints::tight(Size::new(200, 100)), &children);
//! ```

use crate::layout::Constraints;
use crate::style::{Align, Dimension, Edges, FlexDirection, Justify, Style};
use embedded_graphics::prelude::*;
use heapless::Vec as HeaplessVec;

/// Maximum number of children in a flex container (for no_std)
pub const MAX_FLEX_CHILDREN: usize = 32;

const MAX_CHILDREN: usize = 32;

/// Represents a child element in flexbox layout with its style and size
#[derive(Debug, Clone)]
pub struct ChildLayout {
    pub style: Style,
    pub intrinsic_size: Size,
}

impl ChildLayout {
    /// Creates a new child layout
    pub fn new(style: Style, intrinsic_size: Size) -> Self {
        Self {
            style,
            intrinsic_size,
        }
    }
}

/// Result of laying out a single child
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildLayoutResult {
    pub position: Point,
    pub size: Size,
}

/// Flexbox layout engine
///
/// Implements the flexbox layout algorithm for positioning and sizing children
/// within a container.
#[derive(Debug, Clone)]
pub struct FlexLayout {
    style: Style,
}

/// Internal representation of a flex item during layout computation
#[derive(Debug, Clone, Copy)]
struct FlexItem {
    /// Target size on main axis (after flex grow/shrink)
    main_size: u32,
    /// Size on cross axis
    cross_size: u32,
    /// Original intrinsic size
    intrinsic_size: Size,
    /// Style of this item
    style: Style,
    /// Whether this item can grow
    can_grow: bool,
    /// Whether this item can shrink
    can_shrink: bool,
}

impl FlexLayout {
    /// Creates a new flexbox layout with the given style
    pub fn new(style: Style) -> Self {
        Self { style }
    }

    /// Computes layout for children within the given constraints
    ///
    /// # Arguments
    ///
    /// * `constraints` - The available space constraints
    /// * `children` - Slice of child layouts to position
    ///
    /// # Returns
    ///
    /// A vector of ChildLayoutResults with final positions and sizes for each child
    pub fn layout(&self, constraints: Constraints, children: &[ChildLayout]) -> HeaplessVec<ChildLayoutResult, MAX_CHILDREN> {
        if children.is_empty() {
            return HeaplessVec::new();
        }

        // Determine main and cross axis sizes from constraints
        let (container_main, container_cross) = match self.style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => (constraints.max.width, constraints.max.height),
            FlexDirection::Column | FlexDirection::ColumnReverse => (constraints.max.height, constraints.max.width),
        };

        // Account for padding
        let padding_main = match self.style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.style.padding.horizontal(),
            FlexDirection::Column | FlexDirection::ColumnReverse => self.style.padding.vertical(),
        };
        let padding_cross = match self.style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.style.padding.vertical(),
            FlexDirection::Column | FlexDirection::ColumnReverse => self.style.padding.horizontal(),
        };

        let available_main = container_main.saturating_sub(padding_main);
        let available_cross = container_cross.saturating_sub(padding_cross);

        // Step 1: Resolve child dimensions and create flex items
        let mut flex_items = HeaplessVec::<FlexItem, MAX_CHILDREN>::new();
        for child in children {
            let item = self.create_flex_item(child, available_main, available_cross);
            flex_items.push(item).ok();
        }

        // Step 2: Calculate total space used by gaps
        let gap_space = if children.len() > 1 {
            self.style.gap * (children.len() as u32 - 1)
        } else {
            0
        };

        // Step 3: Calculate total intrinsic size on main axis
        let total_intrinsic: u32 = flex_items.iter().map(|item| item.main_size).sum();
        let total_used = total_intrinsic + gap_space;

        // Step 4: Distribute remaining space (flex-grow) or shrink (flex-shrink)
        let remaining_space = available_main.saturating_sub(total_used);
        self.apply_flex_sizing(&mut flex_items, remaining_space as i32);

        // Step 5: Apply justification on main axis
        let positions_main = self.calculate_main_axis_positions(&flex_items, available_main, gap_space);

        // Step 6: Apply alignment on cross axis
        let positions_cross = self.calculate_cross_axis_positions(&flex_items, available_cross);

        // Step 7: Create final layout results
        let mut result = HeaplessVec::new();
        for (i, item) in flex_items.iter().enumerate() {
            let (x, y, width, height) = match self.style.flex_direction {
                FlexDirection::Row | FlexDirection::RowReverse => (
                    positions_main[i] + self.style.padding.left,
                    positions_cross[i] + self.style.padding.top,
                    item.main_size,
                    item.cross_size,
                ),
                FlexDirection::Column | FlexDirection::ColumnReverse => (
                    positions_cross[i] + self.style.padding.left,
                    positions_main[i] + self.style.padding.top,
                    item.cross_size,
                    item.main_size,
                ),
            };

            // Apply child's margin
            let margin = &item.style.margin;
            let final_x = x + margin.left;
            let final_y = y + margin.top;
            let final_width = width.saturating_sub(margin.horizontal());
            let final_height = height.saturating_sub(margin.vertical());

            result.push(ChildLayoutResult {
                position: Point::new(final_x as i32, final_y as i32),
                size: Size::new(final_width, final_height),
            }).ok();
        }

        result
    }

    /// Creates a flex item from a child layout
    fn create_flex_item(&self, child: &ChildLayout, available_main: u32, available_cross: u32) -> FlexItem {
        let (intrinsic_main, intrinsic_cross) = match self.style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => (child.intrinsic_size.width, child.intrinsic_size.height),
            FlexDirection::Column | FlexDirection::ColumnReverse => (child.intrinsic_size.height, child.intrinsic_size.width),
        };

        // Resolve main axis size
        let main_size = match self.style.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                let width_resolved = child.style.width.resolve(available_main);
                if width_resolved == 0 {
                    let basis_resolved = child.style.flex_basis.resolve(available_main);
                    if basis_resolved == 0 {
                        intrinsic_main
                    } else {
                        basis_resolved
                    }
                } else {
                    width_resolved
                }
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                let height_resolved = child.style.height.resolve(available_main);
                if height_resolved == 0 {
                    let basis_resolved = child.style.flex_basis.resolve(available_main);
                    if basis_resolved == 0 {
                        intrinsic_main
                    } else {
                        basis_resolved
                    }
                } else {
                    height_resolved
                }
            }
        };

        // Resolve cross axis size (handle stretch)
        let cross_size = match self.style.align_items {
            Align::Stretch => {
                // For stretch, use full available size unless explicitly sized
                match self.style.flex_direction {
                    FlexDirection::Row | FlexDirection::RowReverse => {
                        let height_resolved = child.style.height.resolve(available_cross);
                        if height_resolved == 0 {
                            available_cross
                        } else {
                            height_resolved
                        }
                    }
                    FlexDirection::Column | FlexDirection::ColumnReverse => {
                        let width_resolved = child.style.width.resolve(available_cross);
                        if width_resolved == 0 {
                            available_cross
                        } else {
                            width_resolved
                        }
                    }
                }
            }
            _ => {
                match self.style.flex_direction {
                    FlexDirection::Row | FlexDirection::RowReverse => {
                        let height_resolved = child.style.height.resolve(available_cross);
                        if height_resolved == 0 {
                            intrinsic_cross
                        } else {
                            height_resolved
                        }
                    }
                    FlexDirection::Column | FlexDirection::ColumnReverse => {
                        let width_resolved = child.style.width.resolve(available_cross);
                        if width_resolved == 0 {
                            intrinsic_cross
                        } else {
                            width_resolved
                        }
                    }
                }
            }
        };

        FlexItem {
            main_size,
            cross_size,
            intrinsic_size: child.intrinsic_size,
            style: child.style,
            can_grow: child.style.flex_grow > 0.0,
            can_shrink: child.style.flex_shrink > 0.0,
        }
    }

    /// Applies flex-grow or flex-shrink to distribute remaining space
    fn apply_flex_sizing(&self, items: &mut [FlexItem], remaining_space: i32) {
        if remaining_space > 0 {
            // Flex-grow: distribute extra space
            let total_grow: f32 = items.iter()
                .filter(|item| item.can_grow)
                .map(|item| item.style.flex_grow)
                .sum();

            if total_grow > 0.0 {
                let extra_space = remaining_space as f32;
                for item in items.iter_mut() {
                    if item.can_grow {
                        let grow_share = (extra_space * item.style.flex_grow / total_grow) as u32;
                        item.main_size += grow_share;
                    }
                }
            }
        } else if remaining_space < 0 {
            // Flex-shrink: reduce size to fit
            let total_shrink: f32 = items.iter()
                .filter(|item| item.can_shrink)
                .map(|item| item.style.flex_shrink)
                .sum();

            if total_shrink > 0.0 {
                let deficit = (-remaining_space) as f32;
                for item in items.iter_mut() {
                    if item.can_shrink {
                        let shrink_share = (deficit * item.style.flex_shrink / total_shrink) as u32;
                        item.main_size = item.main_size.saturating_sub(shrink_share);
                    }
                }
            }
        }
    }

    /// Calculates positions along the main axis based on justify-content
    fn calculate_main_axis_positions(
        &self,
        items: &[FlexItem],
        container_size: u32,
        gap_space: u32,
    ) -> HeaplessVec<u32, MAX_CHILDREN> {
        let mut positions = HeaplessVec::new();
        let total_size: u32 = items.iter().map(|item| item.main_size).sum();
        let remaining = container_size.saturating_sub(total_size + gap_space);

        match self.style.justify_content {
            Justify::Start => {
                let mut pos = 0;
                for item in items {
                    positions.push(pos).ok();
                    pos += item.main_size + self.style.gap;
                }
            }
            Justify::End => {
                let mut pos = remaining;
                for item in items {
                    positions.push(pos).ok();
                    pos += item.main_size + self.style.gap;
                }
            }
            Justify::Center => {
                let mut pos = remaining / 2;
                for item in items {
                    positions.push(pos).ok();
                    pos += item.main_size + self.style.gap;
                }
            }
            Justify::SpaceBetween => {
                if items.len() == 1 {
                    positions.push(0).ok();
                } else {
                    let gap = remaining / (items.len() as u32 - 1);
                    let mut pos = 0;
                    for item in items {
                        positions.push(pos).ok();
                        pos += item.main_size + gap;
                    }
                }
            }
            Justify::SpaceAround => {
                let gap = remaining / items.len() as u32;
                let mut pos = gap / 2;
                for item in items {
                    positions.push(pos).ok();
                    pos += item.main_size + gap;
                }
            }
            Justify::SpaceEvenly => {
                let gap = remaining / (items.len() as u32 + 1);
                let mut pos = gap;
                for item in items {
                    positions.push(pos).ok();
                    pos += item.main_size + gap;
                }
            }
        }

        positions
    }

    /// Calculates positions along the cross axis based on align-items
    fn calculate_cross_axis_positions(
        &self,
        items: &[FlexItem],
        container_size: u32,
    ) -> HeaplessVec<u32, MAX_CHILDREN> {
        let mut positions = HeaplessVec::new();

        for item in items {
            let pos = match self.style.align_items {
                Align::Start => 0,
                Align::End => container_size.saturating_sub(item.cross_size),
                Align::Center => (container_size.saturating_sub(item.cross_size)) / 2,
                Align::Stretch => 0, // Position at start, size handled in cross_size
                Align::Baseline => 0, // TODO: Implement baseline alignment
            };
            positions.push(pos).ok();
        }

        positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec;
    use std::vec::Vec;

    fn create_child(width: u32, height: u32) -> ChildLayout {
        ChildLayout {
            style: Style::default(),
            intrinsic_size: Size::new(width, height),
        }
    }

    fn create_child_with_style(width: u32, height: u32, mut style: Style) -> ChildLayout {
        style.width = Dimension::Px(width);
        style.height = Dimension::Px(height);
        ChildLayout {
            style,
            intrinsic_size: Size::new(width, height),
        }
    }

    #[test]
    fn test_flex_row_start() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::Start;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].position, Point::new(0, 0));
        assert_eq!(result[1].position, Point::new(50, 0));
        assert_eq!(result[2].position, Point::new(100, 0));
    }

    #[test]
    fn test_flex_row_end() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::End;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // 300 - 100 (total width) = 200 remaining, start at 200
        assert_eq!(result[0].position, Point::new(200, 0));
        assert_eq!(result[1].position, Point::new(250, 0));
    }

    #[test]
    fn test_flex_row_center() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::Center;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // 300 - 100 = 200 remaining, center = 100
        assert_eq!(result[0].position, Point::new(100, 0));
        assert_eq!(result[1].position, Point::new(150, 0));
    }

    #[test]
    fn test_flex_row_space_between() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::SpaceBetween;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 3);
        // Total width: 150, remaining: 150, gap between 3 items = 75
        assert_eq!(result[0].position, Point::new(0, 0));
        assert_eq!(result[1].position, Point::new(125, 0));
        assert_eq!(result[2].position, Point::new(250, 0));
    }

    #[test]
    fn test_flex_row_space_around() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::SpaceAround;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Remaining: 200, gap per item: 100, half-gap at start: 50
        assert_eq!(result[0].position, Point::new(50, 0));
        assert_eq!(result[1].position, Point::new(200, 0));
    }

    #[test]
    fn test_flex_row_space_evenly() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::SpaceEvenly;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Remaining: 200, 3 gaps (before, between, after) = 66.66 each ≈ 66
        assert_eq!(result[0].position.x, 66);
        assert_eq!(result[1].position.x, 182);
    }

    #[test]
    fn test_flex_column_start() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Column;
        style.justify_content = Justify::Start;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 30),
            create_child(50, 30),
            create_child(50, 30),
        ];

        let result = layout.layout(Constraints::tight(Size::new(100, 300)), &children);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].position, Point::new(0, 0));
        assert_eq!(result[1].position, Point::new(0, 30));
        assert_eq!(result[2].position, Point::new(0, 60));
    }

    #[test]
    fn test_flex_with_gap() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.justify_content = Justify::Start;
        style.gap = 10;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].position, Point::new(0, 0));
        assert_eq!(result[1].position, Point::new(60, 0)); // 50 + 10 gap
        assert_eq!(result[2].position, Point::new(120, 0)); // 50 + 10 + 50 + 10
    }

    #[test]
    fn test_align_items_start() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.align_items = Align::Start;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 30),
            create_child(50, 40),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 3);
        // All should be at y=0 (start)
        assert_eq!(result[0].position.y, 0);
        assert_eq!(result[1].position.y, 0);
        assert_eq!(result[2].position.y, 0);
    }

    #[test]
    fn test_align_items_end() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.align_items = Align::End;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 30),
            create_child(50, 40),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 3);
        // Should be aligned to bottom
        assert_eq!(result[0].position.y, 70); // 100 - 30
        assert_eq!(result[1].position.y, 60); // 100 - 40
        assert_eq!(result[2].position.y, 50); // 100 - 50
    }

    #[test]
    fn test_align_items_center() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.align_items = Align::Center;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 30),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Centered vertically
        assert_eq!(result[0].position.y, 35); // (100 - 30) / 2
        assert_eq!(result[1].position.y, 25); // (100 - 50) / 2
    }

    #[test]
    fn test_align_items_stretch() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.align_items = Align::Stretch;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 30),
            create_child(50, 40),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Both children should be stretched to container height
        assert_eq!(result[0].size.height, 100);
        assert_eq!(result[1].size.height, 100);
    }

    #[test]
    fn test_padding() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.padding = Edges::all(10);

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 50),
            create_child(50, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Should be offset by padding
        assert_eq!(result[0].position, Point::new(10, 10));
        assert_eq!(result[1].position, Point::new(60, 10));
    }

    #[test]
    fn test_child_margin() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        let mut child_style = Style::default();
        child_style.margin = Edges::all(5);
        child_style.width = Dimension::Px(50);
        child_style.height = Dimension::Px(50);

        let children = vec![
            ChildLayout {
                style: child_style,
                intrinsic_size: Size::new(50, 50),
            },
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 1);
        // Position includes margin
        assert_eq!(result[0].position, Point::new(5, 5));
        // Size excludes margin
        assert_eq!(result[0].size, Size::new(40, 40)); // 50 - 2*5
    }

    #[test]
    fn test_percentage_dimensions() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        let mut child_style = Style::default();
        child_style.width = Dimension::Percent(0.5); // 50% of parent
        child_style.height = Dimension::Px(50);

        let children = vec![
            ChildLayout {
                style: child_style,
                intrinsic_size: Size::new(100, 50),
            },
        ];

        let result = layout.layout(Constraints::tight(Size::new(200, 100)), &children);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].size.width, 100); // 50% of 200
    }

    #[test]
    fn test_flex_grow() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        let mut child1_style = Style::default();
        child1_style.flex_grow = 1.0;

        let mut child2_style = Style::default();
        child2_style.flex_grow = 2.0;

        let children = vec![
            create_child_with_style(50, 50, child1_style),
            create_child_with_style(50, 50, child2_style),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Remaining: 200 pixels
        // Child 1 gets 1/3 (66), total = 116
        // Child 2 gets 2/3 (133), total = 183
        assert!(result[0].size.width > 50);
        assert!(result[1].size.width > result[0].size.width);
    }

    #[test]
    fn test_flex_shrink() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        let mut child1_style = Style::default();
        child1_style.flex_shrink = 1.0;

        let mut child2_style = Style::default();
        child2_style.flex_shrink = 2.0;

        // Children want 100 each, but container is only 150
        let children = vec![
            create_child_with_style(100, 50, child1_style),
            create_child_with_style(100, 50, child2_style),
        ];

        let result = layout.layout(Constraints::tight(Size::new(150, 100)), &children);

        assert_eq!(result.len(), 2);
        // Need to shrink by 50 total
        // Child 1 shrinks by 1/3 (16), final = 84
        // Child 2 shrinks by 2/3 (33), final = 67
        assert!(result[0].size.width < 100);
        assert!(result[1].size.width < result[0].size.width);
    }

    #[test]
    fn test_empty_children() {
        let style = Style::default();
        let layout = FlexLayout::new(style);
        let children: Vec<ChildLayout> = vec![];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_single_child_space_between() {
        let mut style = Style::default();
        style.justify_content = Justify::SpaceBetween;

        let layout = FlexLayout::new(style);
        let children = vec![create_child(50, 50)];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 1);
        // Single child should be at start
        assert_eq!(result[0].position, Point::new(0, 0));
    }

    #[test]
    fn test_complex_layout() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Column;
        style.justify_content = Justify::SpaceBetween;
        style.align_items = Align::Center;
        style.padding = Edges::all(16);
        style.gap = 8;

        let layout = FlexLayout::new(style);

        let mut child_style = Style::default();
        child_style.margin = Edges::horizontal_vertical(4, 8);

        let children = vec![
            create_child_with_style(100, 40, child_style),
            create_child_with_style(100, 40, child_style),
            create_child_with_style(100, 40, child_style),
        ];

        let result = layout.layout(Constraints::tight(Size::new(200, 300)), &children);

        assert_eq!(result.len(), 3);

        // Verify all children are centered horizontally
        for node in &result {
            assert!(node.position.x > 0);
            assert!(node.position.x < 100);
        }

        // Verify vertical spacing
        assert!(result[1].position.y > result[0].position.y);
        assert!(result[2].position.y > result[1].position.y);
    }

    #[test]
    fn test_overflow_handling() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        // Children want more space than available
        let children = vec![
            create_child(200, 50),
            create_child(200, 50),
        ];

        let result = layout.layout(Constraints::tight(Size::new(300, 100)), &children);

        assert_eq!(result.len(), 2);
        // Children should shrink to fit (flex-shrink defaults to 1.0)
        let total_width = result[0].size.width + result[1].size.width;
        assert!(total_width <= 300);
    }

    #[test]
    fn test_column_with_gap() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Column;
        style.gap = 12;

        let layout = FlexLayout::new(style);
        let children = vec![
            create_child(50, 30),
            create_child(50, 40),
        ];

        let result = layout.layout(Constraints::tight(Size::new(100, 200)), &children);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].position, Point::new(0, 0));
        assert_eq!(result[1].position, Point::new(0, 42)); // 30 + 12 gap
    }

    #[test]
    fn test_flex_basis() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        let mut child_style = Style::default();
        child_style.flex_basis = Dimension::Px(80);

        let children = vec![
            create_child_with_style(50, 50, child_style),
        ];

        let result = layout.layout(Constraints::tight(Size::new(200, 100)), &children);

        assert_eq!(result.len(), 1);
        // flex_basis should override intrinsic size
        assert_eq!(result[0].size.width, 80);
    }

    #[test]
    fn test_asymmetric_padding() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;
        style.padding = Edges::new(5, 10, 15, 20);

        let layout = FlexLayout::new(style);
        let children = vec![create_child(50, 50)];

        let result = layout.layout(Constraints::tight(Size::new(200, 100)), &children);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].position, Point::new(20, 5)); // left padding, top padding
    }

    #[test]
    fn test_asymmetric_margin() {
        let mut style = Style::default();
        style.flex_direction = FlexDirection::Row;

        let layout = FlexLayout::new(style);

        let mut child_style = Style::default();
        child_style.margin = Edges::new(2, 4, 6, 8);
        child_style.width = Dimension::Px(50);
        child_style.height = Dimension::Px(50);

        let children = vec![
            ChildLayout {
                style: child_style,
                intrinsic_size: Size::new(50, 50),
            },
        ];

        let result = layout.layout(Constraints::tight(Size::new(200, 100)), &children);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].position, Point::new(8, 2)); // left margin, top margin
        assert_eq!(result[0].size, Size::new(38, 42)); // 50 - (8+4), 50 - (2+6)
    }
}
