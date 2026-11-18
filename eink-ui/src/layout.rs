//! The flexbox layout engine: axis-relative geometry, the heterogeneous
//! [`Children`] composition trait, and the main-axis distribution algorithm
//! shared by [`Row`](crate::widgets::Row) / [`Col`](crate::widgets::Col).
//!
//! # Why a `Children` trait
//!
//! [`Widget::draw`] is generic over the `DrawTarget`, so `Widget` is not
//! object-safe and a flex container cannot hold `&[&dyn Widget]`. Instead a
//! container is generic over its children (`Flex<C: Children>`), and `Children`
//! is implemented for **tuples** of differing widget types (heterogeneous, no
//! heap, no boxing) and for `&[W]` slices (homogeneous lists). Composition is
//! fully monomorphized — the no-alloc core never allocates.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::primitives::Rectangle;

use crate::theme::Theme;
use crate::widget::{Constraints, DrawError, Size, Widget};

/// The primary (main) axis a flex container lays its children along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Children flow left-to-right; the main axis is the x/width axis.
    Horizontal,
    /// Children flow top-to-bottom; the main axis is the y/height axis.
    Vertical,
}

impl Axis {
    /// The main-axis extent of `size` (width for [`Axis::Horizontal`]).
    #[must_use]
    pub const fn main(self, size: Size) -> u32 {
        match self {
            Axis::Horizontal => size.w,
            Axis::Vertical => size.h,
        }
    }

    /// The cross-axis extent of `size` (height for [`Axis::Horizontal`]).
    #[must_use]
    pub const fn cross(self, size: Size) -> u32 {
        match self {
            Axis::Horizontal => size.h,
            Axis::Vertical => size.w,
        }
    }

    /// Build a [`Size`] from a main-axis and cross-axis extent.
    #[must_use]
    pub const fn size(self, main: u32, cross: u32) -> Size {
        match self {
            Axis::Horizontal => Size::new(main, cross),
            Axis::Vertical => Size::new(cross, main),
        }
    }

    /// Build a [`Point`] from a main-axis and cross-axis offset.
    #[must_use]
    pub const fn point(self, main: i32, cross: i32) -> Point {
        match self {
            Axis::Horizontal => Point::new(main, cross),
            Axis::Vertical => Point::new(cross, main),
        }
    }

    /// The main-axis coordinate of `p` (x for [`Axis::Horizontal`]).
    #[must_use]
    pub const fn main_pt(self, p: Point) -> i32 {
        match self {
            Axis::Horizontal => p.x,
            Axis::Vertical => p.y,
        }
    }

    /// The cross-axis coordinate of `p` (y for [`Axis::Horizontal`]).
    #[must_use]
    pub const fn cross_pt(self, p: Point) -> i32 {
        match self {
            Axis::Horizontal => p.y,
            Axis::Vertical => p.x,
        }
    }
}

/// How leftover main-axis space is distributed when no child is flexible
/// (CSS `justify-content`). Ignored when any child has a non-zero
/// [`Widget::flex`] factor — flexible children absorb the slack instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Justify {
    /// Pack children at the main-axis start; slack trails at the end.
    Start,
    /// Center children along the main axis; slack splits to both ends.
    Center,
    /// Pack children at the main-axis end; slack leads at the start.
    End,
    /// First child at the start, last at the end, slack split *between*.
    SpaceBetween,
    /// Equal slack around every child (half-slack at the two ends).
    SpaceAround,
}

/// How each child is placed on the cross axis (CSS `align-items`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    /// Place each child at the cross-axis start.
    Start,
    /// Center each child on the cross axis.
    Center,
    /// Place each child at the cross-axis end.
    End,
    /// Stretch each child to fill the container's cross-axis extent.
    Stretch,
}

/// A fixed, heterogeneous or homogeneous list of child [`Widget`]s a flex
/// container lays out. Implemented for tuples `(A,)` … `(A, …, H)` of differing
/// widget types and for `&[W]` slices. See the [module docs](self) for why this
/// exists instead of `&[&dyn Widget]`.
pub trait Children {
    /// The number of children.
    fn len(&self) -> usize;

    /// Whether there are no children.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Measure the `index`-th child under `limits`. Out-of-range → [`Size::ZERO`].
    fn measure_at(&self, index: usize, limits: Constraints, theme: &Theme) -> Size;

    /// The `index`-th child's [`Widget::flex`] grow factor. Out-of-range → `0`.
    fn flex_at(&self, index: usize) -> u32;

    /// Draw the `index`-th child into `rect`. Out-of-range → no-op `Ok(())`.
    fn draw_at<D: DrawTarget<Color = Gray4>>(
        &self,
        index: usize,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>>;
}

macro_rules! impl_children_tuple {
    ($n:literal; $($idx:tt : $T:ident),+) => {
        impl<$($T: Widget),+> Children for ($($T,)+) {
            fn len(&self) -> usize { $n }

            fn measure_at(&self, index: usize, limits: Constraints, theme: &Theme) -> Size {
                match index {
                    $($idx => self.$idx.measure(limits, theme),)+
                    _ => Size::ZERO,
                }
            }

            fn flex_at(&self, index: usize) -> u32 {
                match index {
                    $($idx => self.$idx.flex(),)+
                    _ => 0,
                }
            }

            fn draw_at<Tgt: DrawTarget<Color = Gray4>>(
                &self,
                index: usize,
                target: &mut Tgt,
                rect: Rectangle,
                theme: &Theme,
            ) -> Result<(), DrawError<Tgt::Error>> {
                match index {
                    $($idx => self.$idx.draw(target, rect, theme),)+
                    _ => Ok(()),
                }
            }
        }
    };
}

impl_children_tuple!(1; 0: A);
impl_children_tuple!(2; 0: A, 1: B);
impl_children_tuple!(3; 0: A, 1: B, 2: C);
impl_children_tuple!(4; 0: A, 1: B, 2: C, 3: D);
impl_children_tuple!(5; 0: A, 1: B, 2: C, 3: D, 4: E);
impl_children_tuple!(6; 0: A, 1: B, 2: C, 3: D, 4: E, 5: F);
impl_children_tuple!(7; 0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G);
impl_children_tuple!(8; 0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H);

/// Homogeneous children: a borrowed slice of a single widget type (the browse
/// primitive for `List`). Unbounded length without `alloc` because the slice is
/// borrowed, not owned.
impl<W: Widget> Children for &[W] {
    fn len(&self) -> usize {
        <[W]>::len(self)
    }

    fn measure_at(&self, index: usize, limits: Constraints, theme: &Theme) -> Size {
        self.get(index)
            .map_or(Size::ZERO, |w| w.measure(limits, theme))
    }

    fn flex_at(&self, index: usize) -> u32 {
        self.get(index).map_or(0, Widget::flex)
    }

    fn draw_at<D: DrawTarget<Color = Gray4>>(
        &self,
        index: usize,
        target: &mut D,
        rect: Rectangle,
        theme: &Theme,
    ) -> Result<(), DrawError<D::Error>> {
        self.get(index)
            .map_or(Ok(()), |w| w.draw(target, rect, theme))
    }
}

/// Loose constraints letting a child take any size up to `cross` on the cross
/// axis and anything on the main axis — the box children are measured under.
fn child_limits(axis: Axis, cross: u32) -> Constraints {
    Constraints::loose(axis.size(u32::MAX, cross))
}

/// Sum of all children's measured main-axis extents plus the inter-child gaps —
/// a flex container's intrinsic main-axis size. Flexible children (e.g.
/// [`Spacer`](crate::widgets::Spacer)) measure to ~0 and so add nothing here;
/// they only grow when drawn into a concrete rect with slack.
pub(crate) fn measure_flex<C: Children>(
    children: &C,
    axis: Axis,
    gap: u32,
    inner: Constraints,
    theme: &Theme,
) -> Size {
    let n = children.len();
    if n == 0 {
        return Size::ZERO;
    }
    let limits = child_limits(axis, axis.cross(inner.max));
    let mut main_total: u32 = 0;
    let mut cross_max: u32 = 0;
    for i in 0..n {
        let s = children.measure_at(i, limits, theme);
        main_total = main_total.saturating_add(axis.main(s));
        cross_max = cross_max.max(axis.cross(s));
    }
    let gaps = gap.saturating_mul((n as u32).saturating_sub(1));
    axis.size(main_total.saturating_add(gaps), cross_max)
}

/// The leading offset and inter-child spacing a [`Justify`] produces for `n`
/// children with `free` main-axis slack and base `gap`. Only consulted when no
/// child is flexible.
fn justify_spacing(justify: Justify, n: usize, gap: u32, free: u32) -> (u32, u32) {
    let n_u32 = n as u32;
    match justify {
        Justify::Start => (0, gap),
        Justify::Center => (free / 2, gap),
        Justify::End => (free, gap),
        Justify::SpaceBetween => {
            // Slack split into n-1 gaps; with one child it behaves like Start.
            let extra = free.checked_div(n_u32.saturating_sub(1)).unwrap_or(0);
            (0, gap.saturating_add(extra))
        }
        Justify::SpaceAround => {
            // Equal slack around each child: a half-unit at each end, a full
            // unit between neighbours.
            let unit = free.checked_div(n_u32).unwrap_or(0);
            (unit / 2, gap.saturating_add(unit))
        }
    }
}

/// Cross-axis offset that places a `child_cross`-wide child within an
/// `avail_cross`-wide track under `align`.
#[allow(clippy::cast_possible_wrap)] // offsets are display coordinates, << i32::MAX
fn align_offset(align: Align, avail_cross: u32, child_cross: u32) -> i32 {
    let slack = avail_cross.saturating_sub(child_cross);
    match align {
        Align::Start | Align::Stretch => 0,
        Align::Center => (slack / 2) as i32,
        Align::End => slack as i32,
    }
}

/// Lay `children` out along `axis` within `inner` and draw each into its
/// computed rectangle. Inflexible children take their measured main size;
/// leftover main space is either distributed to flexible children (by their
/// [`Widget::flex`] factor) or, if none are flexible, positioned per `justify`.
/// Each child is placed on the cross axis per `align` (and stretched to the full
/// cross extent under [`Align::Stretch`]).
// The eight parameters are the full flex configuration (axis/justify/align/gap)
// plus the draw context (inner rect, theme, target) — all distinct and
// well-named; bundling them into a struct would not aid readability here.
#[allow(clippy::cast_possible_wrap, clippy::too_many_arguments)]
pub(crate) fn draw_flex<C, D>(
    children: &C,
    axis: Axis,
    justify: Justify,
    align: Align,
    gap: u32,
    inner: Rectangle,
    theme: &Theme,
    target: &mut D,
) -> Result<(), DrawError<D::Error>>
where
    C: Children,
    D: DrawTarget<Color = Gray4>,
{
    let n = children.len();
    if n == 0 {
        return Ok(());
    }

    let avail = Size::from_eg(inner.size);
    let avail_main = axis.main(avail);
    let avail_cross = axis.cross(avail);
    let limits = child_limits(axis, avail_cross);

    // Pass 1: sum inflexible main extents and total flex factor.
    let mut inflexible_main: u32 = 0;
    let mut total_flex: u32 = 0;
    for i in 0..n {
        let f = children.flex_at(i);
        if f == 0 {
            inflexible_main =
                inflexible_main.saturating_add(axis.main(children.measure_at(i, limits, theme)));
        } else {
            total_flex = total_flex.saturating_add(f);
        }
    }

    let gaps = gap.saturating_mul((n as u32).saturating_sub(1));
    let free = avail_main
        .saturating_sub(inflexible_main)
        .saturating_sub(gaps);

    let (lead, spacing) = if total_flex > 0 {
        (0, gap)
    } else {
        justify_spacing(justify, n, gap, free)
    };

    // Pass 2: place and draw each child. `distributed` tracks flex space already
    // handed out so the last flexible child absorbs any integer remainder.
    let main_start = axis.main_pt(inner.top_left);
    let cross_start = axis.cross_pt(inner.top_left);
    let mut cursor_main = main_start.saturating_add(lead as i32);
    let mut distributed: u32 = 0;
    let mut flex_seen: u32 = 0;

    for i in 0..n {
        let measured = children.measure_at(i, limits, theme);
        let f = children.flex_at(i);
        let child_main = if f == 0 {
            axis.main(measured)
        } else {
            flex_seen = flex_seen.saturating_add(f);
            // Proportional share; the final flexible child takes the remainder.
            let target_cumulative = u64::from(free)
                .saturating_mul(u64::from(flex_seen))
                .checked_div(u64::from(total_flex))
                .unwrap_or(0) as u32;
            let share = target_cumulative.saturating_sub(distributed);
            distributed = distributed.saturating_add(share);
            share
        };

        let child_cross = match align {
            Align::Stretch => avail_cross,
            _ => axis.cross(measured),
        };
        let cross_off = cross_start.saturating_add(align_offset(align, avail_cross, child_cross));

        let rect = Rectangle::new(
            axis.point(cursor_main, cross_off),
            axis.size(child_main, child_cross).to_eg(),
        );
        children.draw_at(i, target, rect, theme)?;

        cursor_main = cursor_main
            .saturating_add(child_main as i32)
            .saturating_add(spacing as i32);
    }

    Ok(())
}
