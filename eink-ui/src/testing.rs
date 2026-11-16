//! Host-only test support: an in-memory [`Framebuffer`] `DrawTarget<Color =
//! Gray4>` with PNG export, used by this crate's own golden tests (see
//! `tests/golden.rs`) and available to downstream consumers that want the
//! same golden-image technique for their screens.
//!
//! Gated behind the `std` feature; never compiled into the no-alloc core.

use std::path::Path;
use std::vec::Vec;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size as EgSize};
use embedded_graphics::pixelcolor::{Gray4, GrayColor};
use embedded_graphics::Pixel;

/// An in-memory framebuffer of [`Gray4`] pixels.
///
/// Implements `embedded_graphics::draw_target::DrawTarget<Color = Gray4>`,
/// so any [`crate::widget::Widget`] can draw into it directly. Pixels
/// written outside the framebuffer's bounds are silently dropped (matching
/// how a real display driver clips), rather than panicking.
#[derive(Debug, Clone)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    background: Gray4,
    pixels: Vec<Gray4>,
}

impl Framebuffer {
    /// Create a `width` x `height` framebuffer filled with [`Role::Paper`]'s
    /// canonical value (`Gray4::new(0xF)`, white).
    ///
    /// [`Role::Paper`]: crate::theme::Role::Paper
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self::filled(width, height, Gray4::new(0xF))
    }

    /// Create a `width` x `height` framebuffer filled with `background`.
    #[must_use]
    pub fn filled(width: u32, height: u32, background: Gray4) -> Self {
        let area = (width as usize).saturating_mul(height as usize);
        Framebuffer {
            width,
            height,
            background,
            pixels: std::vec![background; area],
        }
    }

    /// This framebuffer's width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// This framebuffer's height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Read back the pixel at `(x, y)`, or `None` if out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Option<Gray4> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y as usize)
            .saturating_mul(self.width as usize)
            .saturating_add(x as usize);
        self.pixels.get(idx).copied()
    }

    /// The fraction (`0.0..=1.0`) of pixels that differ from the
    /// framebuffer's initial background fill — i.e. how much "ink" has
    /// been drawn. Useful for a golden test's sanity assertion that a
    /// widget actually drew *something*.
    #[must_use]
    pub fn coverage(&self) -> f32 {
        if self.pixels.is_empty() {
            return 0.0;
        }
        let changed = self
            .pixels
            .iter()
            .filter(|&&p| p != self.background)
            .count();
        #[allow(clippy::cast_precision_loss)]
        let ratio = changed as f32 / self.pixels.len() as f32;
        ratio
    }

    /// Save this framebuffer as an 8-bit grayscale PNG (the 4-bit
    /// [`Gray4`] value is scaled `0..=15 -> 0..=255`), creating any missing
    /// parent directories first.
    ///
    /// # Errors
    ///
    /// Returns an error if the parent directory cannot be created or the
    /// PNG cannot be written/encoded.
    pub fn save_png(&self, path: impl AsRef<Path>) -> image::ImageResult<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(image::ImageError::IoError)?;
            }
        }

        let mut img = image::GrayImage::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                // Safe: x < width, y < height by construction of the loop bounds.
                let color = self.pixel(x, y).unwrap_or(self.background);
                let gray8 = color.luma().saturating_mul(17); // 15 * 17 == 255
                img.put_pixel(x, y, image::Luma([gray8]));
            }
        }
        img.save(path)
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> EgSize {
        EgSize::new(self.width, self.height)
    }
}

impl DrawTarget for Framebuffer {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let (Ok(x), Ok(y)) = (u32::try_from(point.x), u32::try_from(point.y)) {
                if x < self.width && y < self.height {
                    let idx = (y as usize)
                        .saturating_mul(self.width as usize)
                        .saturating_add(x as usize);
                    if let Some(slot) = self.pixels.get_mut(idx) {
                        *slot = color;
                    }
                }
            }
        }
        Ok(())
    }
}
