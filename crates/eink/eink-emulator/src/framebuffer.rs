//! CPU-based framebuffer for e-ink simulation
//!
//! Supports both grayscale (Gray4) and tri-color modes (Spectra 6, Kaleido 3).
//! Uses unified EinkColor type for all pixel operations.

use crate::pixel_color::{EinkColor, SpectraColor};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::GrayColor;

/// Color mode for framebuffer
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ColorMode {
    /// Traditional grayscale (4 levels)
    Grayscale,
    /// Spectra 6 (ACeP) - 6 colors with dual-plane
    Spectra6,
    /// Kaleido 3 - 4096 colors via color filter
    Kaleido3,
}

/// CPU-based framebuffer for e-ink simulation
pub struct Framebuffer {
    pub pixels: Vec<EinkColor>,
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
}

impl Framebuffer {
    /// Create new framebuffer filled with white (backward compatibility)
    pub fn new(width: u32, height: u32) -> Self {
        Self::with_color_mode(width, height, ColorMode::Grayscale)
    }

    /// Create framebuffer with specific color mode
    // SAFETY: width * height is a pixel count bounded by display dimensions (~800×480 max),
    // so width * height fits in u32; cast to usize is always safe on 32-bit+ targets.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn with_color_mode(width: u32, height: u32, mode: ColorMode) -> Self {
        let size = (width * height) as usize;
        let default_pixel = match mode {
            ColorMode::Grayscale => EinkColor::Gray(Gray4::WHITE),
            ColorMode::Spectra6 => EinkColor::Spectra6 {
                bw: Gray4::WHITE,
                color: SpectraColor::None,
            },
            ColorMode::Kaleido3 => EinkColor::Kaleido3 {
                r: 15,
                g: 15,
                b: 15,
            }, // White
        };

        Self {
            pixels: vec![default_pixel; size],
            width,
            height,
            color_mode: mode,
        }
    }

    /// Create from Gray4 buffer (backward compatibility)
    pub fn from_gray4(width: u32, height: u32, gray_pixels: Vec<Gray4>) -> Self {
        let pixels = gray_pixels.into_iter().map(EinkColor::from).collect();
        Self {
            pixels,
            width,
            height,
            color_mode: ColorMode::Grayscale,
        }
    }

    /// Set pixel at coordinates
    // SAFETY: x < width and y < height are checked before use; y * width + x is bounded
    // by width * height which fits in u32 for display-sized framebuffers.
    #[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    pub fn set_pixel(&mut self, x: u32, y: u32, color: EinkColor) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.pixels[idx] = color;
        }
    }

    /// Get pixel at coordinates
    // SAFETY: x < width and y < height are checked before use; y * width + x is bounded
    // by width * height which fits in u32 for display-sized framebuffers.
    #[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<EinkColor> {
        if x < self.width && y < self.height {
            Some(self.pixels[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    /// Convert to RGBA for display (using EinkColor::to_rgba)
    pub fn to_rgba(&self) -> Vec<u32> {
        self.pixels.iter().map(|color| color.to_rgba()).collect()
    }

    /// Fill entire framebuffer with color
    pub fn fill(&mut self, color: EinkColor) {
        self.pixels.fill(color);
    }

    /// Clear framebuffer (fill with white)
    pub fn clear(&mut self) {
        let white = match self.color_mode {
            ColorMode::Grayscale => EinkColor::Gray(Gray4::WHITE),
            ColorMode::Spectra6 => EinkColor::Spectra6 {
                bw: Gray4::WHITE,
                color: SpectraColor::None,
            },
            ColorMode::Kaleido3 => EinkColor::Kaleido3 {
                r: 15,
                g: 15,
                b: 15,
            },
        };
        self.fill(white);
    }

    /// Convert Gray4 pixel to current color mode's equivalent
    pub fn gray4_to_mode(&self, gray: Gray4) -> EinkColor {
        match self.color_mode {
            ColorMode::Grayscale => EinkColor::Gray(gray),
            ColorMode::Spectra6 => EinkColor::Spectra6 {
                bw: gray,
                color: SpectraColor::None,
            },
            ColorMode::Kaleido3 => {
                // Convert grayscale to RGB
                // SAFETY: luma() returns 0-3; 3 * 5 = 15 which fits in u8.
                #[allow(clippy::arithmetic_side_effects)]
                let value = gray.luma() * 5; // 0-3 → 0-15
                EinkColor::Kaleido3 {
                    r: value,
                    g: value,
                    b: value,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_framebuffer_creation() {
        let fb = Framebuffer::new(100, 50);
        assert_eq!(fb.width, 100);
        assert_eq!(fb.height, 50);
        assert_eq!(fb.pixels.len(), 5000);
        assert_eq!(fb.color_mode, ColorMode::Grayscale);
    }

    #[test]
    fn test_color_mode_creation() {
        let fb_gray = Framebuffer::with_color_mode(10, 10, ColorMode::Grayscale);
        assert_eq!(fb_gray.color_mode, ColorMode::Grayscale);

        let fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);
        assert_eq!(fb_spectra.color_mode, ColorMode::Spectra6);

        let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
        assert_eq!(fb_kaleido.color_mode, ColorMode::Kaleido3);
    }

    #[test]
    fn test_set_get_pixel() {
        let mut fb = Framebuffer::new(10, 10);
        let black = EinkColor::Gray(Gray4::BLACK);
        fb.set_pixel(5, 5, black);
        assert_eq!(fb.get_pixel(5, 5), Some(black));

        let white = EinkColor::Gray(Gray4::WHITE);
        assert_eq!(fb.get_pixel(0, 0), Some(white));
    }

    #[test]
    fn test_bounds_checking() {
        let mut fb = Framebuffer::new(10, 10);
        fb.set_pixel(100, 100, EinkColor::Gray(Gray4::BLACK)); // Should not panic
        assert_eq!(fb.get_pixel(100, 100), None);
    }

    #[test]
    fn test_rgba_conversion_grayscale() {
        let mut fb = Framebuffer::new(2, 2);
        fb.set_pixel(0, 0, EinkColor::Gray(Gray4::new(0))); // Black
        fb.set_pixel(1, 0, EinkColor::Gray(Gray4::new(1)));
        fb.set_pixel(0, 1, EinkColor::Gray(Gray4::new(2)));
        fb.set_pixel(1, 1, EinkColor::Gray(Gray4::new(3))); // White

        let rgba = fb.to_rgba();
        assert_eq!(rgba.len(), 4);

        // Black should be 0xFF000000 (ARGB: A=255, R=0, G=0, B=0)
        assert_eq!(rgba[0], 0xFF000000);

        // White should be 0xFFFFFFFF (ARGB: A=255, R=255, G=255, B=255)
        assert_eq!(rgba[3], 0xFFFFFFFF);
    }

    #[test]
    fn test_rgba_conversion_spectra6() {
        let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Spectra6);

        fb.set_pixel(
            0,
            0,
            EinkColor::Spectra6 {
                bw: Gray4::WHITE,
                color: SpectraColor::Red,
            },
        );

        let rgba = fb.to_rgba();
        // ARGB format: Red is at bits 16-23
        let r = (rgba[0] >> 16) & 0xFF;
        assert_eq!(r, 255, "Red channel should be 255 for Red pigment");
    }

    #[test]
    fn test_rgba_conversion_kaleido3() {
        let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Kaleido3);

        fb.set_pixel(0, 0, EinkColor::Kaleido3 { r: 15, g: 0, b: 0 });

        let rgba = fb.to_rgba();
        assert_eq!(rgba[0], 0xFFFF0000); // Pure red (ARGB: A=255, R=255, G=0, B=0)
    }

    #[test]
    fn test_clear() {
        let mut fb = Framebuffer::new(10, 10);
        fb.set_pixel(5, 5, EinkColor::Gray(Gray4::BLACK));
        fb.clear();

        for y in 0..fb.height {
            for x in 0..fb.width {
                assert_eq!(fb.get_pixel(x, y), Some(EinkColor::Gray(Gray4::WHITE)));
            }
        }
    }

    #[test]
    fn test_from_gray4() {
        let gray_pixels = vec![Gray4::BLACK; 100];
        let fb = Framebuffer::from_gray4(10, 10, gray_pixels);

        assert_eq!(fb.width, 10);
        assert_eq!(fb.height, 10);
        assert_eq!(fb.color_mode, ColorMode::Grayscale);
        assert_eq!(fb.get_pixel(0, 0), Some(EinkColor::Gray(Gray4::BLACK)));
    }

    #[test]
    fn test_gray4_to_mode() {
        let fb_gray = Framebuffer::with_color_mode(10, 10, ColorMode::Grayscale);
        let converted = fb_gray.gray4_to_mode(Gray4::new(2));
        assert_eq!(converted, EinkColor::Gray(Gray4::new(2)));

        let fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);
        let converted = fb_spectra.gray4_to_mode(Gray4::new(2));
        match converted {
            EinkColor::Spectra6 { bw, color } => {
                assert_eq!(bw.luma(), 2);
                assert_eq!(color, SpectraColor::None);
            }
            _ => unreachable!("gray4_to_mode(Spectra6) returned unexpected variant"),
        }

        let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
        let converted = fb_kaleido.gray4_to_mode(Gray4::new(2));
        match converted {
            EinkColor::Kaleido3 { r, g, b } => {
                assert_eq!(r, 10); // 2 * 5 = 10
                assert_eq!(g, 10);
                assert_eq!(b, 10);
            }
            _ => unreachable!("gray4_to_mode(Kaleido3) returned unexpected variant"),
        }
    }
}
