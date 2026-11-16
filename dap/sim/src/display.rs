//! `SimDisplay` -- a host test double for `dap_core::display::Display`: an
//! 800x480 `Gray4` framebuffer backed by a plain `Vec`, matching the
//! GDEM0397T81P/SSD1677 panel this product targets (see
//! `eink_specs::displays::GDEM0397T81P` -- 2c's board adapter is for this
//! exact controller/resolution).
//!
//! `EinkRefresh` is counted, not simulated: a `full_refresh`/
//! `partial_refresh` call just records that it happened (and, for
//! `partial_refresh`, what `area` it was asked to refresh). This is a
//! headless double for asserting what `App::render` drew into the
//! framebuffer -- not a waveform/ghosting emulator (that's the root
//! workspace's `eink-emulator`'s job; `dap/` is a separate workspace from
//! it on purpose -- see `dap/Cargo.toml`'s own comment on why -- so this
//! crate doesn't reach for it).

use dap_core::display::{DisplayError, EinkRefresh, Gray4, Rectangle};
use embedded_graphics::prelude::*;

/// Panel width, pixels -- the GDEM0397T81P/SSD1677 panel's native resolution.
pub const WIDTH: u32 = 800;
/// Panel height, pixels.
pub const HEIGHT: u32 = 480;

/// A host [`Display`](dap_core::display::Display) test double: an in-memory
/// [`WIDTH`]x[`HEIGHT`] [`Gray4`] framebuffer, plus refresh-call counters.
pub struct SimDisplay {
    /// Row-major, `WIDTH * HEIGHT` pixels.
    pixels: Vec<Gray4>,
    full_refresh_count: u32,
    partial_refresh_count: u32,
    /// The `area` passed to the most recent `partial_refresh` call, if any.
    last_partial_refresh_area: Option<Rectangle>,
}

impl SimDisplay {
    /// A blank (all-white) [`WIDTH`]x[`HEIGHT`] framebuffer, with no
    /// refreshes recorded yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pixels: vec![Gray4::WHITE; (WIDTH * HEIGHT) as usize],
            full_refresh_count: 0,
            partial_refresh_count: 0,
            last_partial_refresh_area: None,
        }
    }

    /// The color at `(x, y)`, or `None` if out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<Gray4> {
        if x >= WIDTH || y >= HEIGHT {
            return None;
        }
        self.pixels.get((y * WIDTH + x) as usize).copied()
    }

    /// Number of pixels that are not pure white -- a coarse "something was
    /// drawn" signal for headless assertions.
    #[must_use]
    pub fn non_blank_pixel_count(&self) -> usize {
        self.pixels.iter().filter(|&&p| p != Gray4::WHITE).count()
    }

    /// Fraction (`0.0..=1.0`) of pixels that are not pure white.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn coverage(&self) -> f32 {
        self.non_blank_pixel_count() as f32 / self.pixels.len() as f32
    }

    /// Number of [`EinkRefresh::full_refresh`] calls observed.
    #[must_use]
    pub fn full_refresh_count(&self) -> u32 {
        self.full_refresh_count
    }

    /// Number of [`EinkRefresh::partial_refresh`] calls observed.
    #[must_use]
    pub fn partial_refresh_count(&self) -> u32 {
        self.partial_refresh_count
    }

    /// The `area` passed to the most recent [`EinkRefresh::partial_refresh`]
    /// call, if any (`None` if `partial_refresh` has never been called).
    #[must_use]
    pub fn last_partial_refresh_area(&self) -> Option<Rectangle> {
        self.last_partial_refresh_area
    }

    /// Save the current framebuffer as a PNG at `path` (its parent
    /// directories are created if missing), for eyeballing what a test
    /// actually rendered.
    ///
    /// # Errors
    /// Whatever `image`'s PNG encoder returns for a write failure.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> Result<(), image::ImageError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut img = image::GrayImage::new(WIDTH, HEIGHT);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                // Gray4::luma() is 0..=15 (4-bit); scale up to a full-range
                // 8-bit PNG sample (15 * 17 == 255, so WHITE stays exactly
                // 255 and BLACK stays exactly 0).
                let luma4 = self.pixel(x, y).map_or(0xF, |p| p.luma());
                img.put_pixel(x, y, image::Luma([luma4 * 17]));
            }
        }
        img.save(path)
    }
}

impl Default for SimDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl embedded_graphics::draw_target::DrawTarget for SimDisplay {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        for embedded_graphics::Pixel(point, color) in pixels {
            // `DrawTarget` implementors must silently discard out-of-bounds
            // pixels rather than erroring or panicking -- see its trait
            // docs' example.
            if point.x < 0 || point.y < 0 {
                continue;
            }
            let (x, y) = (point.x as u32, point.y as u32);
            if x < WIDTH && y < HEIGHT {
                if let Some(slot) = self.pixels.get_mut((y * WIDTH + x) as usize) {
                    *slot = color;
                }
            }
        }
        Ok(())
    }
}

impl OriginDimensions for SimDisplay {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

impl EinkRefresh for SimDisplay {
    fn full_refresh(&mut self) -> Result<(), DisplayError> {
        self.full_refresh_count += 1;
        Ok(())
    }

    fn partial_refresh(&mut self, area: Rectangle) -> Result<(), DisplayError> {
        self.partial_refresh_count += 1;
        self.last_partial_refresh_area = Some(area);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_blank() {
        let d = SimDisplay::new();
        assert_eq!(d.non_blank_pixel_count(), 0);
        assert_eq!(d.coverage(), 0.0);
        assert_eq!(d.full_refresh_count(), 0);
        assert_eq!(d.partial_refresh_count(), 0);
        assert_eq!(d.last_partial_refresh_area(), None);
    }

    #[test]
    fn size_matches_the_target_panel() {
        let d = SimDisplay::new();
        assert_eq!(d.size(), Size::new(800, 480));
    }

    #[test]
    fn draw_iter_sets_in_bounds_pixels_and_drops_out_of_bounds_ones() {
        let mut d = SimDisplay::new();
        d.draw_iter([
            embedded_graphics::Pixel(Point::new(0, 0), Gray4::BLACK),
            embedded_graphics::Pixel(Point::new(799, 479), Gray4::BLACK),
            // Out of bounds on every side -- must be silently dropped, not
            // panic (per `DrawTarget`'s contract) and not corrupt the
            // in-bounds pixel count.
            embedded_graphics::Pixel(Point::new(-1, 10), Gray4::BLACK),
            embedded_graphics::Pixel(Point::new(10, -1), Gray4::BLACK),
            embedded_graphics::Pixel(Point::new(800, 10), Gray4::BLACK),
            embedded_graphics::Pixel(Point::new(10, 480), Gray4::BLACK),
        ])
        .expect("draw_iter is infallible");

        assert_eq!(d.pixel(0, 0), Some(Gray4::BLACK));
        assert_eq!(d.pixel(799, 479), Some(Gray4::BLACK));
        assert_eq!(d.non_blank_pixel_count(), 2);
        assert_eq!(d.pixel(800, 10), None, "out of bounds stays None");
    }

    #[test]
    fn refresh_calls_are_counted() {
        let mut d = SimDisplay::new();
        d.full_refresh().expect("full_refresh");
        d.full_refresh().expect("full_refresh");
        let area = Rectangle::new(Point::new(1, 2), Size::new(3, 4));
        d.partial_refresh(area).expect("partial_refresh");

        assert_eq!(d.full_refresh_count(), 2);
        assert_eq!(d.partial_refresh_count(), 1);
        assert_eq!(d.last_partial_refresh_area(), Some(area));
    }

    #[test]
    fn save_png_round_trips_black_and_white() {
        let mut d = SimDisplay::new();
        d.draw_iter([embedded_graphics::Pixel(Point::new(5, 5), Gray4::BLACK)])
            .expect("draw_iter is infallible");

        let path = std::env::temp_dir().join(format!(
            "sim_display_test_{}_{:?}.png",
            std::process::id(),
            std::thread::current().id()
        ));
        d.save_png(&path).expect("save_png");

        let img = image::open(&path).expect("open saved png").to_luma8();
        assert_eq!(
            img.get_pixel(5, 5).0[0],
            0,
            "black pixel must round-trip as luma 0"
        );
        assert_eq!(
            img.get_pixel(0, 0).0[0],
            255,
            "untouched pixel must round-trip as luma 255 (white)"
        );
        let _ = std::fs::remove_file(&path);
    }
}
