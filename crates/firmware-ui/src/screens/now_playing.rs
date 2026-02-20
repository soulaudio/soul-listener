//! Now Playing screen renderer
//!
//! Renders the main playback screen using embedded-graphics primitives.
//!
//! # Registered test IDs
//!
//! | test ID                   | Component type |
//! |---------------------------|----------------|
//! | `"now-playing-title"`     | `"Label"`      |
//! | `"now-playing-artist"`    | `"Label"`      |
//! | `"now-playing-progress"`  | `"ProgressBar"`|
//! | `"now-playing-play-btn"`  | `"Button"`     |

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use ui::now_playing::NowPlayingState;

/// Render the Now Playing screen onto any `DrawTarget<Color = Gray4>`.
///
/// The `register` closure is invoked for each named logical component so that
/// callers can record component positions for test assertions.
///
/// # Arguments
///
/// * `display`  – Any `DrawTarget<Color = Gray4>` (e-ink hardware, emulator, or test emulator).
/// * `state`    – Current now-playing state (track, position, volume, playing flag).
/// * `register` – Called with `(test_id, component_type, (x, y), (w, h))` for each component.
///
/// # Errors
///
/// Returns `Err(D::Error)` if any draw call fails.
pub fn render_now_playing_to<D, R>(
    display: &mut D,
    state: &NowPlayingState,
    mut register: R,
) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray4>,
    R: FnMut(&str, &str, (i32, i32), (u32, u32)),
{
    let bounds = display.bounding_box();
    let w = bounds.size.width;
    let h = bounds.size.height;

    // ── Background ─────────────────────────────────────────────────────────
    Rectangle::new(Point::zero(), bounds.size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(display)?;

    // ── Header bar (dark strip at top) ────────────────────────────────────
    let header_height = 50u32;
    Rectangle::new(Point::zero(), Size::new(w, header_height))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    let header_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("Now Playing", Point::new(20, 32), header_style).draw(display)?;

    // ── Track title ───────────────────────────────────────────────────────
    let title_y = 80i32;
    let title_text: &str = if state.title.is_empty() {
        "Unknown Track"
    } else {
        state.title.as_str()
    };
    let title_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    Text::new(title_text, Point::new(20, title_y), title_style).draw(display)?;
    register(
        "now-playing-title",
        "Label",
        (20, title_y - 20),
        (w.saturating_sub(40), 24),
    );

    // ── Artist ────────────────────────────────────────────────────────────
    let artist_y = 110i32;
    let artist_text: &str = if state.artist.is_empty() {
        "Unknown Artist"
    } else {
        state.artist.as_str()
    };
    let artist_style = MonoTextStyle::new(&FONT_6X10, Gray4::new(0x6));
    Text::new(artist_text, Point::new(20, artist_y), artist_style).draw(display)?;
    register(
        "now-playing-artist",
        "Label",
        (20, artist_y - 10),
        (w.saturating_sub(40), 14),
    );

    // ── Progress bar ──────────────────────────────────────────────────────
    let bar_y = 150i32;
    let bar_h = 12u32;
    let bar_w = w.saturating_sub(40);
    let progress = state.progress();

    // Background track
    Rectangle::new(Point::new(20, bar_y), Size::new(bar_w, bar_h))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xC)))
        .draw(display)?;

    // Filled portion
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    let filled_w = ((bar_w as f32) * progress) as u32;
    if filled_w > 0 {
        Rectangle::new(Point::new(20, bar_y), Size::new(filled_w, bar_h))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
            .draw(display)?;
    }

    // Border
    Rectangle::new(Point::new(20, bar_y), Size::new(bar_w, bar_h))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
        .draw(display)?;
    register(
        "now-playing-progress",
        "ProgressBar",
        (20, bar_y),
        (bar_w, bar_h),
    );

    // ── Play/Pause button ─────────────────────────────────────────────────
    let btn_h = 40u32;
    let btn_w = 100u32;
    // SAFETY: display dimensions (800×480) are far below i32::MAX; wrapping is impossible.
    #[allow(clippy::cast_possible_wrap)]
    let btn_y = (h as i32).saturating_sub(80);
    // SAFETY: display dimensions (800×480) are far below i32::MAX; wrapping is impossible.
    #[allow(clippy::cast_possible_wrap)]
    let btn_x = ((w as i32).saturating_sub(btn_w as i32)) / 2;
    let btn_label = if state.playing { "Pause" } else { "Play" };

    Rectangle::new(Point::new(btn_x, btn_y), Size::new(btn_w, btn_h))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;
    // Border
    Rectangle::new(Point::new(btn_x, btn_y), Size::new(btn_w, btn_h))
        .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
        .draw(display)?;
    let btn_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new(btn_label, Point::new(btn_x + 10, btn_y + 26), btn_style).draw(display)?;
    register(
        "now-playing-play-btn",
        "Button",
        (btn_x, btn_y),
        (btn_w, btn_h),
    );

    Ok(())
}
