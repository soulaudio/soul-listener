//! Structural + golden tests for the flexbox layer (`row`/`col`/`Spacer`/
//! `Stack`). These assert the layout *algorithm* (measured sizes and where
//! children land) by rendering into an in-memory [`Framebuffer`] and inspecting
//! the ink, independent of any real display.
//!
//! Run with: `cargo test --features std`.

use eink_ui::layout::{Align, Justify};
use eink_ui::testing::Framebuffer;
use eink_ui::{col, row, Constraints, Size, Space, Spacer, Stack, Text, Theme, Widget};
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::GrayColor;
use embedded_graphics::primitives::Rectangle;

const PAPER: u8 = 0xF;

/// Draw `widget` into a fresh `w`x`h` framebuffer, filling the whole box.
fn render_in<W: Widget>(widget: &W, theme: &Theme, w: u32, h: u32) -> Framebuffer {
    let mut fb = Framebuffer::new(w, h);
    let rect = Rectangle::new(Point::new(0, 0), Size::new(w, h).to_eg());
    widget
        .draw(&mut fb, rect, theme)
        .expect("drawing into an in-memory Framebuffer never fails");
    fb
}

/// The `(leftmost, rightmost)` inked (non-paper) column, or `None` if blank.
fn ink_x_span(fb: &Framebuffer) -> Option<(u32, u32)> {
    let mut span: Option<(u32, u32)> = None;
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if fb.pixel(x, y).is_some_and(|p| p.luma() != PAPER) {
                span = Some(span.map_or((x, x), |(lo, hi)| (lo.min(x), hi.max(x))));
            }
        }
    }
    span
}

/// The `(topmost, bottommost)` inked row, or `None` if blank.
fn ink_y_span(fb: &Framebuffer) -> Option<(u32, u32)> {
    let mut span: Option<(u32, u32)> = None;
    for y in 0..fb.height() {
        for x in 0..fb.width() {
            if fb.pixel(x, y).is_some_and(|p| p.luma() != PAPER) {
                span = Some(span.map_or((y, y), |(lo, hi)| (lo.min(y), hi.max(y))));
            }
        }
    }
    span
}

#[test]
fn row_measures_to_sum_of_children_plus_gap() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(500, 100));

    let a = Text::new("AB").measure(limits, &theme);
    let b = Text::new("CD").measure(limits, &theme);
    let r = row((Text::new("AB"), Text::new("CD"))).gap(Space::Sm);
    let m = r.measure(limits, &theme);

    assert_eq!(
        m.w,
        a.w + b.w + theme.space(Space::Sm),
        "row main-axis width = child widths + one gap"
    );
    assert_eq!(m.h, a.h.max(b.h), "row cross-axis height = tallest child");
}

#[test]
fn col_measures_to_sum_of_heights_plus_gap() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(200, 500));

    let one = Text::new("Line").measure(limits, &theme);
    let c = col((Text::new("Line"), Text::new("Line"))).gap(Space::Xs);
    let m = c.measure(limits, &theme);

    assert_eq!(
        m.h,
        one.h * 2 + theme.space(Space::Xs),
        "col main-axis height = child heights + one gap"
    );
    assert_eq!(m.w, one.w, "col cross-axis width = widest child");
}

#[test]
fn spacer_pushes_trailing_child_to_the_end() {
    let theme = Theme::default();
    // "L" at the start, Spacer eats the middle, "R" shoved to the right edge.
    let r = row((Text::new("L"), Spacer::new(), Text::new("R")));
    let fb = render_in(&r, &theme, 200, 20);

    let (lo, hi) = ink_x_span(&fb).expect("row should draw both labels");
    assert!(lo < 20, "left label hugs the start edge (got {lo})");
    assert!(hi > 180, "right label is pushed to the end edge (got {hi})");

    fb.save_png(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/golden/row_spacer.png"
    ))
    .expect("save row_spacer.png");
}

#[test]
fn two_spacers_center_a_lone_child() {
    let theme = Theme::default();
    // Spacer on each side => the glyph ends up centered.
    let r = row((Spacer::new(), Text::new("X"), Spacer::new()));
    let fb = render_in(&r, &theme, 200, 20);

    let (lo, hi) = ink_x_span(&fb).expect("the centered glyph should draw");
    let mid = (lo + hi) / 2;
    assert!(
        (90..=110).contains(&mid),
        "glyph centered near x=100 (got midpoint {mid})"
    );
}

#[test]
fn justify_center_centers_without_spacers() {
    let theme = Theme::default();
    let r = row((Text::new("X"),)).justify(Justify::Center);
    let fb = render_in(&r, &theme, 200, 20);

    let (lo, hi) = ink_x_span(&fb).expect("the glyph should draw");
    let mid = (lo + hi) / 2;
    assert!(
        (90..=110).contains(&mid),
        "justify-center puts the glyph near x=100 (got midpoint {mid})"
    );
}

#[test]
fn align_center_centers_on_the_cross_axis() {
    let theme = Theme::default();
    // A short Body-height glyph in a tall row, vertically centered.
    let r = row((Text::new("A"),)).align(Align::Center);
    let fb = render_in(&r, &theme, 40, 40);

    let (top, bottom) = ink_y_span(&fb).expect("the glyph should draw");
    let mid = (top + bottom) / 2;
    assert!(
        (14..=26).contains(&mid),
        "align-center puts the glyph near y=20 (got midpoint {mid})"
    );
}

#[test]
fn stack_centers_its_child() {
    let theme = Theme::default();
    let s = Stack::new((Text::new("M"),));
    let fb = render_in(&s, &theme, 100, 40);

    let (lo, hi) = ink_x_span(&fb).expect("stacked glyph should draw");
    let mid = (lo + hi) / 2;
    assert!(
        (40..=60).contains(&mid),
        "stack centers its child near x=50 (got midpoint {mid})"
    );

    fb.save_png(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/golden/stack_center.png"
    ))
    .expect("save stack_center.png");
}

#[test]
fn grow_weights_split_space_proportionally() {
    let theme = Theme::default();
    // grow(1) before the lone divider glyph, grow(3) after => the divider sits
    // at ~1/4 of the free width. The divider is the only ink, so its position is
    // unambiguous.
    let r = row((Spacer::grow(1), Text::new("|"), Spacer::grow(3)));
    let fb = render_in(&r, &theme, 200, 20);

    let (lo, hi) = ink_x_span(&fb).expect("the divider should draw");
    let mid = (lo + hi) / 2;
    assert!(
        (35..=65).contains(&mid),
        "grow 1:3 puts the divider near x=50 (got midpoint {mid})"
    );
}
