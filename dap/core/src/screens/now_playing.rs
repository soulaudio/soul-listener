//! The Now-Playing screen: track title, transport status, a progress bar,
//! volume, and battery — the one screen Phase 2a renders (see
//! [`crate::app::App::render`]). The layout here is deliberately
//! minimal/functional; Phase 2b is expected to polish it by rendering real
//! PNGs and picking, per this project's design convention.
//!
//! ## Why most of this draws with raw `Text`, not [`crate::display::Label`]
//! [`crate::display::Label`] stores its text as `&'static str` — it's built
//! for compile-time UI chrome (button captions, static headings), not text
//! computed fresh every frame from caller-supplied state. Every field on
//! [`NowPlayingView`] *is* computed fresh every frame, so none of it can
//! satisfy `Label`'s bound without leaking a new allocation per frame (this
//! crate is `no_std`/no-alloc on-target — not an option). `embedded_graphics`'s
//! own [`Text`] has no such restriction (it borrows for the draw call only),
//! so that's what every dynamic string here uses. [`crate::display::ProgressBar`]
//! and [`crate::display::Icon`] carry no text, so they're used as-is.

use core::fmt::Write as _;

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X10, MonoTextStyle},
    prelude::*,
    text::Text,
};
use heapless::String;

use crate::display::{Display, DisplayError, Gray4, Icon, IconType, ProgressBar};

/// Left margin shared by every row this screen draws.
const MARGIN_X: i32 = 40;

/// Everything [`render`] needs to draw one frame of the Now-Playing screen.
/// Built fresh by the caller (typically [`crate::app::App::render`]) from
/// the player/power state at that moment — not retained state of its own.
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

/// Draw `view` onto `d`: a white background, a play/pause status icon,
/// the track title, a progress bar, and volume/battery readouts.
///
/// # Errors
/// [`DisplayError::Bus`] if any draw call into `d` fails. `D::Error` (an
/// associated type with no fixed meaning) is mapped to `Bus` rather than
/// `Timeout`, since [`DisplayError`] has no generic "drawing failed"
/// variant of its own — see its docs — and a failed in-memory draw is, for
/// every real controller this crate targets, a transfer/bus-level fault
/// rather than a `BUSY`-line timeout.
pub fn render<D: Display>(d: &mut D, view: &NowPlayingView<'_>) -> Result<(), DisplayError> {
    d.clear(Gray4::WHITE).map_err(|_| DisplayError::Bus)?;

    // Status icon + title.
    let icon_type = if view.playing {
        IconType::Play
    } else {
        IconType::Pause
    };
    Icon::new(icon_type, 40)
        .color(Gray4::BLACK)
        .render(d, Point::new(MARGIN_X, 40))
        .map_err(|_| DisplayError::Bus)?;

    let title_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    Text::new(view.title, Point::new(MARGIN_X + 60, 68), title_style)
        .draw(d)
        .map_err(|_| DisplayError::Bus)?;

    // Progress bar, with an elapsed/total readout underneath.
    let progress = if view.total_s > 0 {
        view.elapsed_s as f32 / view.total_s as f32
    } else {
        0.0
    };
    ProgressBar::new(720, 28)
        .progress(progress)
        .colors(Gray4::WHITE, Gray4::BLACK)
        .render(d, Point::new(MARGIN_X, 220))
        .map_err(|_| DisplayError::Bus)?;

    let small_style = MonoTextStyle::new(&FONT_6X10, Gray4::BLACK);

    let mut time_buf: String<32> = String::new();
    let _ = write_time(&mut time_buf, view.elapsed_s, view.total_s);
    Text::new(&time_buf, Point::new(MARGIN_X, 270), small_style)
        .draw(d)
        .map_err(|_| DisplayError::Bus)?;

    // Volume / battery readouts.
    let mut vol_buf: String<16> = String::new();
    let vol_pct = (view.volume.clamp(0.0, 1.0) * 100.0) as u32;
    let _ = write!(vol_buf, "VOL {vol_pct:>3}%");
    Text::new(&vol_buf, Point::new(MARGIN_X, 340), small_style)
        .draw(d)
        .map_err(|_| DisplayError::Bus)?;

    let mut batt_buf: String<16> = String::new();
    let _ = write!(batt_buf, "BATT {:>3}%", view.battery_permille / 10);
    Text::new(&batt_buf, Point::new(MARGIN_X, 370), small_style)
        .draw(d)
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

    /// A tiny in-memory `Display`: a `Vec<Gray4>` big enough for these
    /// tests' assertions (not [`crate::screens`]'s concern to provide the
    /// real one — that's `dap-sim`'s `SimDisplay`, exercised end-to-end by
    /// `dap-sim`'s `ui_headless` test). Panics on out-of-bounds writes --
    /// fine here since every draw call in `render` targets coordinates well
    /// within this test display's bounds.
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

        fn pixel(&self, x: u32, y: u32) -> Gray4 {
            self.pixels[(y * self.width + x) as usize]
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

        let view_a = NowPlayingView {
            title: "Alpha",
            ..sample_view()
        };
        let view_b = NowPlayingView {
            title: "Zulu Zulu Zulu",
            ..sample_view()
        };

        render(&mut a, &view_a).expect("render a");
        render(&mut b, &view_b).expect("render b");

        // Scan a band around the title's baseline (y=68) rather than one
        // exact row: the alphabetic baseline a glyph's ink touches depends
        // on the font's internal metrics (descenders, flat-bottomed
        // letters, …), not something this test should hard-code.
        let title_band_differs = (0..800).any(|x| (50..75).any(|y| a.pixel(x, y) != b.pixel(x, y)));
        assert!(
            title_band_differs,
            "two different titles must draw different pixels in the title band"
        );
    }

    #[test]
    fn render_calls_no_refresh_itself() {
        // `render` only draws; committing the frame (`full_refresh`) is the
        // caller's job (`App::render` calls both) -- see the module docs.
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
