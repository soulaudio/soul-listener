//! Unified pixel color abstraction for grayscale and tri-color e-ink displays
//!
//! Supports three color modes:
//! - Grayscale: Traditional 4-level B&W displays
//! - Spectra 6 (ACeP): 6-color displays with separate B&W and color planes
//! - Kaleido 3: 4096-color displays with color filter overlay

use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::GrayColor;

/// Unified color type supporting grayscale and tri-color modes
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EinkColor {
    /// Grayscale mode (4 levels, traditional e-ink)
    Gray(Gray4),

    /// Spectra 6 (ACeP - Advanced Color ePaper, 6 colors)
    ///
    /// Uses 4 ink particles (red, blue, yellow, white) to produce 6 colors:
    /// black, white, red, yellow, blue, green
    Spectra6 {
        /// Black/white plane (grayscale level)
        bw: Gray4,
        /// Color pigment state
        color: SpectraColor,
    },

    /// Kaleido 3 (4096 colors via color filter overlay)
    ///
    /// 300ppi B&W panel with color filter overlay
    /// Effective 150ppi color resolution with 4-bit per channel RGB
    Kaleido3 {
        /// 4-bit red (0-15)
        r: u8,
        /// 4-bit green (0-15)
        g: u8,
        /// 4-bit blue (0-15)
        b: u8,
    },
}

/// Spectra 6 color pigment states
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpectraColor {
    /// No color (black/white only)
    None,
    /// Red pigment active
    Red,
    /// Yellow pigment active
    Yellow,
    /// Blue pigment active
    Blue,
    /// Green pigment (yellow + blue combination)
    Green,
}

impl EinkColor {
    /// Create from Gray4 for backward compatibility
    pub fn from_gray4(gray: Gray4) -> Self {
        EinkColor::Gray(gray)
    }

    /// Convert to ARGB for display rendering (0xAARRGGBB format for softbuffer)
    pub fn to_rgba(&self) -> u32 {
        match self {
            EinkColor::Gray(gray) => {
                // Convert grayscale to ARGB
                // Gray4 luma() returns 0-3, scale to 0-255
                let value = (gray.luma() as u32) * 85; // 0,1,2,3 → 0,85,170,255
                let r = value;
                let g = value;
                let b = value;
                let a = 255;
                // ARGB format: 0xAARRGGBB
                (a << 24) | (r << 16) | (g << 8) | b
            }
            EinkColor::Spectra6 { bw, color } => {
                // Combine B&W base with color pigment
                let base_value = (bw.luma() as u32) * 85; // 0-255 range

                match color {
                    SpectraColor::None => {
                        // Pure black/white - ARGB format
                        0xFF000000 | (base_value << 16) | (base_value << 8) | base_value
                    }
                    SpectraColor::Red => {
                        // Red pigment: bright red with gray modulation
                        let r = 255;
                        let g = base_value.min(128); // Muted green/blue
                        let b = base_value.min(128);
                        0xFF000000 | (r << 16) | (g << 8) | b
                    }
                    SpectraColor::Yellow => {
                        // Yellow pigment: bright yellow
                        let r = 255;
                        let g = 255;
                        let b = base_value.min(128);
                        0xFF000000 | (r << 16) | (g << 8) | b
                    }
                    SpectraColor::Blue => {
                        // Blue pigment: bright blue
                        let r = base_value.min(128);
                        let g = base_value.min(128);
                        let b = 255;
                        0xFF000000 | (r << 16) | (g << 8) | b
                    }
                    SpectraColor::Green => {
                        // Green pigment (yellow + blue)
                        let r = base_value.min(128);
                        let g = 255;
                        let b = base_value.min(180); // Slightly less blue
                        0xFF000000 | (r << 16) | (g << 8) | b
                    }
                }
            }
            EinkColor::Kaleido3 { r, g, b } => {
                // 4-bit RGB to 8-bit RGB (scale from 0-15 to 0-255)
                let r8 = ((*r).min(15) as u32) * 17; // 15 * 17 = 255
                let g8 = ((*g).min(15) as u32) * 17;
                let b8 = ((*b).min(15) as u32) * 17;
                // ARGB format: 0xAARRGGBB
                0xFF000000 | (r8 << 16) | (g8 << 8) | b8
            }
        }
    }

    /// Check if this is grayscale mode
    pub fn is_grayscale(&self) -> bool {
        matches!(self, EinkColor::Gray(_))
    }

    /// Check if this is a color mode (Spectra6 or Kaleido3)
    pub fn is_color(&self) -> bool {
        !self.is_grayscale()
    }

    /// Quantize to mode-specific levels (for grayscale)
    pub fn quantize(&self, levels: u8) -> Self {
        match self {
            EinkColor::Gray(gray) => {
                let value = gray.luma();
                let quantized = if levels <= 4 {
                    // Map to nearest supported level
                    let step = 3 / (levels - 1);
                    (value / step) * step
                } else {
                    value
                };
                EinkColor::Gray(Gray4::new(quantized))
            }
            // Color modes are already discrete
            _ => *self,
        }
    }
}

impl From<Gray4> for EinkColor {
    fn from(gray: Gray4) -> Self {
        EinkColor::Gray(gray)
    }
}

impl Default for EinkColor {
    fn default() -> Self {
        EinkColor::Gray(Gray4::WHITE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gray4_conversion() {
        let color = EinkColor::from_gray4(Gray4::BLACK);
        assert!(color.is_grayscale());
        assert!(!color.is_color());

        let rgba = color.to_rgba();
        assert_eq!(rgba, 0x000000FF); // Black with alpha
    }

    #[test]
    fn test_spectra6_conversion() {
        let color = EinkColor::Spectra6 {
            bw: Gray4::new(2),
            color: SpectraColor::Red,
        };
        assert!(color.is_color());
        assert!(!color.is_grayscale());

        let rgba = color.to_rgba();
        // Red channel should be 255
        let r = (rgba >> 24) & 0xFF;
        assert_eq!(r, 255);
    }

    #[test]
    fn test_kaleido3_conversion() {
        let color = EinkColor::Kaleido3 { r: 15, g: 8, b: 0 };
        assert!(color.is_color());

        let rgba = color.to_rgba();
        let r = (rgba >> 24) & 0xFF;
        let g = (rgba >> 16) & 0xFF;
        let b = (rgba >> 8) & 0xFF;

        assert_eq!(r, 255); // 15 * 17 = 255
        assert_eq!(g, 136); // 8 * 17 = 136
        assert_eq!(b, 0); // 0 * 17 = 0
    }

    #[test]
    fn test_spectra6_rgba_output() {
        // Test all Spectra colors
        let colors = [
            (SpectraColor::None, 0xAAAAAAFFu32),     // Gray
            (SpectraColor::Red, 0xFF5555FFu32),      // Red with gray
            (SpectraColor::Yellow, 0xFFFF55FFu32),   // Yellow
            (SpectraColor::Blue, 0x5555FFFFu32),     // Blue
            (SpectraColor::Green, 0x55FFB4FFu32),    // Green
        ];

        for (spectra_color, _expected_pattern) in colors {
            let color = EinkColor::Spectra6 {
                bw: Gray4::new(2), // Mid-gray base
                color: spectra_color,
            };
            let rgba = color.to_rgba();

            // Check that color channels are in expected ranges
            match spectra_color {
                SpectraColor::Red => {
                    let r = (rgba >> 24) & 0xFF;
                    assert_eq!(r, 255, "Red channel should be 255 for Red pigment");
                }
                SpectraColor::Blue => {
                    let b = (rgba >> 8) & 0xFF;
                    assert_eq!(b, 255, "Blue channel should be 255 for Blue pigment");
                }
                SpectraColor::Yellow => {
                    let r = (rgba >> 24) & 0xFF;
                    let g = (rgba >> 16) & 0xFF;
                    assert_eq!(r, 255, "Red should be 255 for Yellow");
                    assert_eq!(g, 255, "Green should be 255 for Yellow");
                }
                SpectraColor::Green => {
                    let g = (rgba >> 16) & 0xFF;
                    assert_eq!(g, 255, "Green channel should be 255 for Green pigment");
                }
                SpectraColor::None => {
                    let r = (rgba >> 24) & 0xFF;
                    let g = (rgba >> 16) & 0xFF;
                    let b = (rgba >> 8) & 0xFF;
                    assert_eq!(r, g, "R and G should match for grayscale");
                    assert_eq!(g, b, "G and B should match for grayscale");
                }
            }
        }
    }

    #[test]
    fn test_kaleido3_rgba_output() {
        // Test pure colors
        let red = EinkColor::Kaleido3 { r: 15, g: 0, b: 0 };
        assert_eq!(red.to_rgba(), 0xFF0000FF);

        let green = EinkColor::Kaleido3 { r: 0, g: 15, b: 0 };
        assert_eq!(green.to_rgba(), 0x00FF00FF);

        let blue = EinkColor::Kaleido3 { r: 0, g: 0, b: 15 };
        assert_eq!(blue.to_rgba(), 0x0000FFFF);

        let white = EinkColor::Kaleido3 {
            r: 15,
            g: 15,
            b: 15,
        };
        assert_eq!(white.to_rgba(), 0xFFFFFFFF);
    }

    #[test]
    fn test_color_quantization() {
        let gray = EinkColor::Gray(Gray4::new(2));
        let quantized = gray.quantize(2); // Binary

        match quantized {
            EinkColor::Gray(g) => {
                let luma = g.luma();
                assert!(luma == 0 || luma == 3, "Should quantize to black or white");
            }
            _ => panic!("Should remain grayscale"),
        }
    }

    #[test]
    fn test_from_trait() {
        let gray = Gray4::new(2);
        let color: EinkColor = gray.into();
        assert_eq!(color, EinkColor::Gray(gray));
    }

    #[test]
    fn test_default() {
        let color = EinkColor::default();
        assert_eq!(color, EinkColor::Gray(Gray4::WHITE));
    }
}
