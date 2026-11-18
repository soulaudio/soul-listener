//! Tests for the e-ink layer: dirty-region tracking and the refresh policy.
//! Run with `cargo test --features std`.

use eink_ui::refresh::{DirtyRegion, DirtyTracker, RefreshKind, RefreshPolicy};
use eink_ui::testing::Framebuffer;
use eink_ui::{Constraints, Size, Text, Theme, Widget};
use embedded_graphics::geometry::Point;
use embedded_graphics::primitives::Rectangle;

#[test]
fn empty_region_is_none() {
    let region = DirtyRegion::new();
    assert!(region.is_empty());
    assert!(region.as_rectangle().is_none());
}

#[test]
fn region_bounds_the_points_included() {
    let mut region = DirtyRegion::new();
    region.include(Point::new(10, 20));
    region.include(Point::new(4, 40));
    region.include(Point::new(30, 25));

    let rect = region.as_rectangle().expect("non-empty region");
    assert_eq!(
        rect.top_left,
        Point::new(4, 20),
        "top-left = (min x, min y)"
    );
    // inclusive extents: x 4..=30 => width 27, y 20..=40 => height 21.
    assert_eq!(rect.size.width, 27);
    assert_eq!(rect.size.height, 21);
}

#[test]
fn single_pixel_is_a_one_by_one_rect() {
    let mut region = DirtyRegion::new();
    region.include(Point::new(7, 7));
    let rect = region.as_rectangle().unwrap();
    assert_eq!(
        rect,
        Rectangle::new(Point::new(7, 7), Size::new(1, 1).to_eg())
    );
}

#[test]
fn tracker_captures_and_forwards_the_drawn_region() {
    let theme = Theme::default();
    let mut fb = Framebuffer::new(128, 64);

    let dirty = {
        let mut tracker = DirtyTracker::new(&mut fb);
        let text = Text::new("Hi");
        let at = Rectangle::new(Point::new(10, 20), Size::new(40, 10).to_eg());
        text.draw(&mut tracker, at, &theme).expect("draw");
        tracker.into_dirty()
    };

    let rect = dirty.as_rectangle().expect("something was drawn");
    // The dirty box starts at/after where we positioned the text and stays within
    // the glyph band.
    assert!(
        rect.top_left.x >= 10 && rect.top_left.x <= 18,
        "x start {}",
        rect.top_left.x
    );
    assert!(
        rect.top_left.y >= 20 && rect.top_left.y <= 30,
        "y start {}",
        rect.top_left.y
    );
    assert!(rect.size.width <= 40, "width within the text rect");

    // The tracker forwarded the draw to the real framebuffer.
    assert!(
        fb.coverage() > 0.0,
        "tracker forwards draws to the inner target"
    );
}

#[test]
fn tracker_measures_sanely_against_known_glyph_metrics() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(200, 100));
    let text = Text::new("Hi");
    let size = text.measure(limits, &theme);

    let mut fb = Framebuffer::new(128, 64);
    let dirty = {
        let mut tracker = DirtyTracker::new(&mut fb);
        text.draw(
            &mut tracker,
            Rectangle::new(Point::zero(), size.to_eg()),
            &theme,
        )
        .unwrap();
        tracker.into_dirty()
    };
    let rect = dirty.as_rectangle().unwrap();
    // The inked box never exceeds the measured glyph box.
    assert!(rect.size.width <= size.w, "dirty width <= measured width");
    assert!(
        rect.size.height <= size.h,
        "dirty height <= measured height"
    );
}

#[test]
fn refresh_policy_forces_periodic_full() {
    // every(3): Full on the first frame, then two Partials, then Full again.
    let mut p = RefreshPolicy::every(3);
    let kinds: [RefreshKind; 6] = core::array::from_fn(|_| p.advance());
    assert_eq!(
        kinds,
        [
            RefreshKind::Full,
            RefreshKind::Partial,
            RefreshKind::Partial,
            RefreshKind::Full,
            RefreshKind::Partial,
            RefreshKind::Partial,
        ]
    );
}

#[test]
fn refresh_policy_every_one_and_zero_are_always_full() {
    let mut one = RefreshPolicy::every(1);
    assert_eq!(
        [one.advance(), one.advance(), one.advance()],
        [RefreshKind::Full; 3]
    );

    let mut zero = RefreshPolicy::every(0);
    assert_eq!([zero.advance(), zero.advance()], [RefreshKind::Full; 2]);
}
