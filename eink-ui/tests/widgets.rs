//! Structural + golden tests for the ui-2 widgets (`ProgressBar`, `Button`,
//! `List`). Run with `cargo test --features std`.

use eink_ui::testing::Framebuffer;
use eink_ui::{
    Button, Constraints, List, ProgressBar, Role, Size, Space, Text, TextScale, Theme, Widget,
};
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::GrayColor;
use embedded_graphics::primitives::Rectangle;

/// Draw `widget` into a fresh `w`x`h` framebuffer filling the whole box.
fn render_in<W: Widget>(widget: &W, theme: &Theme, w: u32, h: u32) -> Framebuffer {
    let mut fb = Framebuffer::new(w, h);
    let rect = Rectangle::new(Point::new(0, 0), Size::new(w, h).to_eg());
    widget.draw(&mut fb, rect, theme).expect("draw never fails");
    fb
}

/// Draw `widget` at its own measured size.
fn render_measured<W: Widget>(
    widget: &W,
    theme: &Theme,
    limits: Constraints,
) -> (Framebuffer, Size) {
    let size = widget.measure(limits, theme);
    let fb = render_in(widget, theme, size.w.max(1), size.h.max(1));
    (fb, size)
}

fn luma_at(fb: &Framebuffer, x: u32, y: u32) -> u8 {
    fb.pixel(x, y).expect("in-bounds pixel").luma()
}

const INK: u8 = 0x0;
const FAINT: u8 = 0xA;
const PAPER: u8 = 0xF;

#[test]
fn progress_bar_fills_proportionally() {
    let theme = Theme::default();
    // 50% across 100 px => left half Ink (fill), right half Faint (track).
    let bar = ProgressBar::new(50, 100).thickness(Space::Md);
    let fb = render_in(&bar, &theme, 100, theme.space(Space::Md));

    assert_eq!(luma_at(&fb, 10, 2), INK, "filled portion is Ink");
    assert_eq!(luma_at(&fb, 90, 2), FAINT, "unfilled track is Faint");
}

#[test]
fn progress_bar_full_and_empty_and_overfill() {
    let theme = Theme::default();
    let h = theme.space(Space::Sm);

    let full = render_in(&ProgressBar::new(100, 100), &theme, 40, h);
    assert_eq!(luma_at(&full, 39, 0), INK, "100% => filled to the end");

    let empty = render_in(&ProgressBar::new(0, 100), &theme, 40, h);
    assert_eq!(luma_at(&empty, 0, 0), FAINT, "0% => track only");

    // filled > total clamps to full.
    let over = render_in(&ProgressBar::new(250, 100), &theme, 40, h);
    assert_eq!(luma_at(&over, 39, 0), INK, "overfill clamps to 100%");
}

#[test]
fn button_measures_text_plus_padding_and_border() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(300, 100));

    let text = Text::new("Play").measure(limits, &theme);
    let btn = Button::new("Play").padding(Space::Sm);
    let size = btn.measure(limits, &theme);

    // padding on both sides + a 1px border on both sides.
    let span = theme.space(Space::Sm) * 2 + 2;
    assert_eq!(
        size.w,
        text.w + span,
        "button width = text + padding + border"
    );
    assert_eq!(
        size.h,
        text.h + span,
        "button height = text + padding + border"
    );
}

#[test]
fn button_unpressed_draws_border_and_ink_label() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(300, 100));
    let (fb, _size) = render_measured(&Button::new("OK"), &theme, limits);

    assert_eq!(luma_at(&fb, 0, 0), INK, "border is Ink at the corner");
    assert!(
        fb.coverage() > 0.0,
        "unpressed button draws border + label ink"
    );
}

#[test]
fn button_pressed_inverts_to_filled_dark() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(300, 100));
    let (fb, _size) = render_measured(&Button::new("OK").pressed(true), &theme, limits);

    // Just inside the border, the fill is Ink (dark) when pressed.
    assert_eq!(luma_at(&fb, 2, 2), INK, "pressed button fills Ink");
}

#[test]
fn list_measures_to_sum_of_row_heights() {
    let theme = Theme::default();
    let limits = Constraints::loose(Size::new(120, 400));

    let rows = [
        Text::new("01  Intro"),
        Text::new("02  Verse"),
        Text::new("03  Outro"),
    ];
    let row_h = Text::new("01  Intro").measure(limits, &theme).h;

    let list = List::new(&rows).gap(Space::Xs);
    let size = list.measure(limits, &theme);

    assert_eq!(
        size.h,
        row_h * 3 + theme.space(Space::Xs) * 2,
        "list height = row heights + inter-row gaps"
    );
    assert_eq!(size.w, 120, "list spans the available width");
}

#[test]
fn list_highlights_only_the_selected_row() {
    let theme = Theme::default();
    let rows = [
        Text::new("Alpha").scale(TextScale::Body).role(Role::Ink),
        Text::new("Bravo").scale(TextScale::Body).role(Role::Ink),
        Text::new("Charlie").scale(TextScale::Body).role(Role::Ink),
    ];
    let row_h = Text::new("Alpha")
        .measure(Constraints::loose(Size::new(120, 400)), &theme)
        .h;

    let list = List::new(&rows).selected(Some(1));
    let fb = render_in(&list, &theme, 120, row_h * 3);

    // Far right (past the text) of row 1 is the Ink highlight fill; row 0 there
    // is untouched Paper.
    let y_row0 = row_h / 2;
    let y_row1 = row_h + row_h / 2;
    assert_eq!(luma_at(&fb, 118, y_row1), INK, "selected row is Ink-filled");
    assert_eq!(
        luma_at(&fb, 118, y_row0),
        PAPER,
        "unselected row stays Paper"
    );

    fb.save_png(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/target/golden/list_selected.png"
    ))
    .expect("save list_selected.png");
}
