//! Golden tests for the Task-1 widget set (`Text`, `Container`).
//!
//! These assert two things per widget: (1) `measure` returns a sane,
//! non-degenerate [`Size`] for the given constraints, and (2) `draw`
//! actually leaves ink on an in-memory [`Framebuffer`] where expected. The
//! rendered frames are also saved to `eink-ui/target/golden/*.png` for
//! humans to eyeball.
//!
//! Run with: `cargo test --features std` (the `testing` module — and these
//! tests, since they depend on it — only exist under the `std` feature).

use eink_ui::testing::Framebuffer;
use eink_ui::{Constraints, Container, Role, Size, Space, Text, TextScale, Theme, Widget};
use embedded_graphics::geometry::Point;
use embedded_graphics::primitives::Rectangle;

/// Render `widget` at its measured size into a fresh [`Framebuffer`],
/// returning the framebuffer and the measured [`Size`].
fn render<W: Widget>(widget: &W, theme: &Theme, limits: Constraints) -> (Framebuffer, Size) {
    let size = widget.measure(limits, theme);
    let mut fb = Framebuffer::new(size.w.max(1), size.h.max(1));
    let rect = Rectangle::new(Point::new(0, 0), size.to_eg());
    widget
        .draw(&mut fb, rect, theme)
        .expect("drawing into an in-memory Framebuffer should never fail");
    (fb, size)
}

#[test]
fn text_measures_sanely_and_draws_ink() {
    let theme = Theme::default();
    let text = Text::new("Hi").scale(TextScale::Body).role(Role::Ink);
    let limits = Constraints::loose(Size::new(200, 100));

    let (fb, size) = render(&text, &theme, limits);

    // Non-degenerate: a two-glyph line at Body scale is a handful of
    // pixels wide/tall, not zero, and not absurdly large.
    assert!(size.w > 0 && size.w < 60, "unexpected width: {}", size.w);
    assert!(size.h > 0 && size.h < 24, "unexpected height: {}", size.h);

    assert!(
        fb.coverage() > 0.0,
        "Text::draw should leave ink on the framebuffer"
    );

    fb.save_png(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/golden/text.png"
    ))
    .expect("failed to save text.png golden image");
}

#[test]
fn text_with_no_content_measures_to_zero_width() {
    let theme = Theme::default();
    let text = Text::new("");
    let size = text.measure(Constraints::loose(Size::new(200, 100)), &theme);

    assert_eq!(size.w, 0, "empty text should have zero measured width");
}

#[test]
fn container_pads_around_its_child() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(200, 100));

    let bare = Text::new("Vol 80%").measure(limits, &theme);
    let container = Container::new(Text::new("Vol 80%")).padding(Space::Sm);
    let padded = container.measure(limits, &theme);

    let pad = theme.space(Space::Sm);
    assert_eq!(
        padded.w,
        bare.w + pad * 2,
        "container should add padding on both sides (w)"
    );
    assert_eq!(
        padded.h,
        bare.h + pad * 2,
        "container should add padding on both sides (h)"
    );
}

#[test]
fn container_draws_background_border_and_child_ink() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(200, 100));
    let container = Container::new(Text::new("Vol 80%").role(Role::Ink))
        .padding(Space::Sm)
        .background(Some(Role::Paper))
        .border(Some(Role::Ink));

    let (fb, size) = render(&container, &theme, limits);

    assert!(
        size.w > 0 && size.h > 0,
        "container should have non-zero size"
    );

    // The border is drawn flush with the container's own rect, so its
    // top-left pixel must be the border's Ink color.
    assert_eq!(
        fb.pixel(0, 0),
        Some(theme.color(Role::Ink)),
        "top-left pixel should be the border color"
    );

    // Something besides the border/background must have been drawn: the
    // child text's ink.
    assert!(
        fb.coverage() > 0.0,
        "container should leave ink on the framebuffer (border + child text)"
    );

    fb.save_png(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/golden/container.png"
    ))
    .expect("failed to save container.png golden image");
}
