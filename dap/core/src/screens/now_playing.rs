//! The Now-Playing screen: a play/pause icon + track title, a progress bar with
//! an elapsed/total readout, and volume/battery — composed with the `eink-ui`
//! toolkit's flexbox layout rather than hand-placed coordinates.
//!
//! Every field on [`NowPlayingView`] is computed fresh each frame from
//! caller-supplied state, so the dynamic strings are formatted into stack
//! `heapless::String` buffers and handed to `eink_ui::Text` (which borrows for
//! the draw only — no per-frame allocation; this crate is no_std/no-alloc on
//! target). The whole screen is one immediate-mode `eink_ui` widget tree built
//! and drawn per frame.

use core::fmt::Write as _;

use eink_ui::{col, row, Glyph, Icon, ProgressBar, Role, Spacer, Text, TextScale, Theme, Widget};
use embedded_graphics::prelude::*;
use heapless::String;

use crate::display::{DisplayError, Gray4};

/// Everything [`render`] needs to draw one frame of the Now-Playing screen.
/// Built fresh by the caller (typically [`crate::app::App::render`]) from the
/// player/power state at that moment — not retained state of its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NowPlayingView<'a> {
    /// Track title/identifier to display.
    pub title: &'a str,
    /// Elapsed playback position, seconds.
    pub elapsed_s: u32,
    /// Track total duration, seconds (`0` if unknown).
    pub total_s: u32,
    /// Output volume, `0.0..=1.0`.
    pub volume: f32,
    /// Battery state of charge, per-mille (`0..=1000`) — see
    /// [`crate::ports::BatteryReading::soc_permille`].
    pub battery_permille: u16,
    /// Whether the deck is actively playing (drives the status icon).
    pub playing: bool,
}

/// Draw `view` onto `d`: a white background, then a play/pause icon beside the
/// title, a progress bar with a time readout, and a volume/battery row — laid
/// out with `eink-ui`'s flexbox and drawn into the panel's full bounding box.
///
/// # Errors
/// [`DisplayError::Bus`] if any draw call into `d` fails. `D::Error` (an
/// associated type with no fixed meaning) is mapped to `Bus` rather than
/// `Timeout`, since [`DisplayError`] has no generic "drawing failed" variant of
/// its own — and a failed in-memory draw is, for every real controller this
/// crate targets, a transfer/bus-level fault rather than a `BUSY`-line timeout.
/// Bound on [`DrawTarget`] rather than [`Display`](crate::display::Display) — it
/// only ever *draws* (the `render_calls_no_refresh_itself` test pins that), and
/// the looser bound is what lets [`App::render`](crate::app::App::render) draw
/// through an `eink_ui::refresh::DirtyTracker` wrapper to measure the dirty
/// region a partial refresh needs. Every `Display` still satisfies it.
pub fn render<D: DrawTarget<Color = Gray4>>(
    d: &mut D,
    view: &NowPlayingView<'_>,
) -> Result<(), DisplayError> {
    d.clear(Gray4::WHITE).map_err(|_| DisplayError::Bus)?;

    let glyph = if view.playing {
        Glyph::Play
    } else {
        Glyph::Pause
    };

    let mut time_buf: String<32> = String::new();
    let _ = write_time(&mut time_buf, view.elapsed_s, view.total_s);

    let mut vol_buf: String<16> = String::new();
    let vol_pct = (view.volume.clamp(0.0, 1.0) * 100.0) as u32;
    let _ = write!(vol_buf, "VOL {vol_pct:>3}%");

    let mut batt_buf: String<16> = String::new();
    let _ = write!(batt_buf, "BATT {:>3}%", view.battery_permille / 10);

    let screen = col((
        // Header: play/pause icon beside the title.
        row((
            Icon::new(glyph, 40),
            Text::new(view.title).scale(TextScale::Title),
        ))
        .gap(eink_ui::Space::Md)
        .align(eink_ui::Align::Center),
        // Progress bar + elapsed/total readout.
        ProgressBar::new(view.elapsed_s, view.total_s).thickness(eink_ui::Space::Sm),
        Text::new(&time_buf)
            .scale(TextScale::Caption)
            .role(Role::Muted),
        // Volume on the left, battery pushed to the right.
        row((
            Text::new(&vol_buf).scale(TextScale::Caption),
            Spacer::new(),
            Text::new(&batt_buf).scale(TextScale::Caption),
        )),
    ))
    .padding(eink_ui::Space::Lg)
    .gap(eink_ui::Space::Lg);

    let area = d.bounding_box();
    let theme = Theme::default();
    screen
        .draw(d, area, &theme)
        .map_err(|_| DisplayError::Bus)?;
    Ok(())
}

/// Format `elapsed_s`/`total_s` (seconds) as `"MM:SS / MM:SS"` into `buf`.
/// Silently truncates (never panics) if `buf` is too small — see
/// [`heapless::String`]'s `fmt::Write` impl.
fn write_time(buf: &mut String<32>, elapsed_s: u32, total_s: u32) -> core::fmt::Result {
    write!(
        buf,
        "{:02}:{:02} / {:02}:{:02}",
        elapsed_s / 60,
        elapsed_s % 60,
        total_s / 60,
        total_s % 60,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny in-memory `Display`: a `Vec<Gray4>` big enough for these tests'
    /// assertions. Panics on out-of-bounds writes — fine here since the screen
    /// is drawn into this display's full bounds.
    struct TinyDisplay {
        width: u32,
        height: u32,
        pixels: std::vec::Vec<Gray4>,
        full_refresh_count: u32,
    }

    impl TinyDisplay {
        fn new(width: u32, height: u32) -> Self {
            Self {
                width,
                height,
                pixels: std::vec![Gray4::WHITE; (width * height) as usize],
                full_refresh_count: 0,
            }
        }
    }

    impl embedded_graphics::draw_target::DrawTarget for TinyDisplay {
        type Color = Gray4;
        type Error = core::convert::Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
        {
            for embedded_graphics::Pixel(point, color) in pixels {
                if point.x >= 0
                    && point.y >= 0
                    && (point.x as u32) < self.width
                    && (point.y as u32) < self.height
                {
                    let idx = point.y as u32 * self.width + point.x as u32;
                    self.pixels[idx as usize] = color;
                }
            }
            Ok(())
        }
    }

    impl embedded_graphics::geometry::OriginDimensions for TinyDisplay {
        fn size(&self) -> embedded_graphics::geometry::Size {
            embedded_graphics::geometry::Size::new(self.width, self.height)
        }
    }

    impl crate::display::EinkRefresh for TinyDisplay {
        fn full_refresh(&mut self) -> Result<(), DisplayError> {
            self.full_refresh_count += 1;
            Ok(())
        }
        fn partial_refresh(
            &mut self,
            _area: crate::display::Rectangle,
        ) -> Result<(), DisplayError> {
            Ok(())
        }
    }

    fn sample_view() -> NowPlayingView<'static> {
        NowPlayingView {
            title: "Test Track",
            elapsed_s: 65,
            total_s: 200,
            volume: 0.75,
            battery_permille: 823,
            playing: true,
        }
    }

    #[test]
    fn render_draws_a_non_blank_frame() {
        let mut d = TinyDisplay::new(800, 480);
        render(&mut d, &sample_view()).expect("render must succeed on a healthy display");

        let non_white = d.pixels.iter().filter(|&&p| p != Gray4::WHITE).count();
        assert!(
            non_white > 0,
            "render must leave at least one non-white pixel"
        );
    }

    #[test]
    fn render_differs_between_distinct_titles() {
        let mut a = TinyDisplay::new(800, 480);
        let mut b = TinyDisplay::new(800, 480);

        render(
            &mut a,
            &NowPlayingView {
                title: "Alpha",
                ..sample_view()
            },
        )
        .expect("render a");
        render(
            &mut b,
            &NowPlayingView {
                title: "Zulu Zulu Zulu",
                ..sample_view()
            },
        )
        .expect("render b");

        // Two different titles must produce different frames somewhere (the
        // exact title position is the layout engine's business, so compare the
        // whole frame rather than a hard-coded band).
        assert_ne!(a.pixels, b.pixels, "distinct titles must draw differently");
    }

    #[test]
    fn render_calls_no_refresh_itself() {
        // `render` only draws; committing the frame (`full_refresh`) is the
        // caller's job (`App::render` calls both).
        let mut d = TinyDisplay::new(800, 480);
        render(&mut d, &sample_view()).expect("render");
        assert_eq!(d.full_refresh_count, 0);
    }

    #[test]
    fn write_time_formats_minutes_and_seconds() {
        let mut buf: String<32> = String::new();
        write_time(&mut buf, 65, 200).expect("fits in 32 bytes");
        assert_eq!(buf.as_str(), "01:05 / 03:20");
    }

    #[test]
    fn write_time_zero_total_is_unknown_duration() {
        let mut buf: String<32> = String::new();
        write_time(&mut buf, 5, 0).expect("fits in 32 bytes");
        assert_eq!(buf.as_str(), "00:05 / 00:00");
    }
}
