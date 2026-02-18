//! E-Ink Waveform Modes
//!
//! Based on E Ink Corporation's waveform specifications and controller datasheets.
//! Each mode has different characteristics for grayscale levels, speed, and quality.

/// Waveform modes supported by e-ink displays
///
/// These correspond to actual hardware modes used by controllers like
/// SSD1680, UC8151, and IT8951.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum WaveformMode {
    /// GC16 - Grayscale Clearing 16-level
    ///
    /// - **Grayscale**: 16 levels (4-bit)
    /// - **Duration**: ~980ms (varies by controller)
    /// - **Flashing**: Multiple flashes (typically 3-4)
    /// - **Ghosting**: Completely clears ghosting
    /// - **Use case**: Initial page render, high-quality images
    ///
    /// This is the highest quality mode and should be used for page transitions
    /// and periodic cleaning.
    GC16,

    /// GL16 - Grayscale 16-level
    ///
    /// - **Grayscale**: 16 levels (4-bit)
    /// - **Duration**: ~980ms
    /// - **Flashing**: Reduced flash compared to GC16
    /// - **Ghosting**: Minimal ghosting for text
    /// - **Use case**: Anti-aliased text on white background
    ///
    /// Optimized for sparse content updates (text on white).
    GL16,

    /// DU - Direct Update
    ///
    /// - **Grayscale**: 2 levels (1-bit, black & white only)
    /// - **Duration**: ~260ms
    /// - **Flashing**: No flashing
    /// - **Ghosting**: Accumulates ghosting (~20% per refresh)
    /// - **Use case**: Fast page turning, scrolling
    ///
    /// Fastest mode but only supports pure black and white.
    /// Cannot update to intermediate gray levels.
    DU,

    /// DU4 - Direct Update 4-level
    ///
    /// - **Grayscale**: 4 levels (2-bit)
    /// - **Duration**: ~260ms
    /// - **Flashing**: Minimal flashing
    /// - **Ghosting**: Accumulates ghosting (~15% per refresh)
    /// - **Use case**: Anti-aliased text in menus, simple animations
    ///
    /// Good balance between speed and quality for grayscale content.
    DU4,

    /// A2 - Animation Mode
    ///
    /// - **Grayscale**: 2 levels (1-bit, black & white only)
    /// - **Duration**: ~200ms (ultra-fast)
    /// - **Flashing**: Single flash
    /// - **Ghosting**: High accumulation (~25% per refresh)
    /// - **Use case**: Animation, live updates, video playback
    ///
    /// Ultra-fast mode for animations. Requires full refresh cleanup frequently.
    A2,

    /// GCC16 - Grayscale Color Clearing (Spectra 6 full refresh)
    ///
    /// - **Colors**: 6 colors (black, white, red, yellow, blue, green)
    /// - **Duration**: ~15,000ms (15 seconds)
    /// - **Flashing**: Many flashes (30+) for color particle movement
    /// - **Ghosting**: Completely clears both B&W and color ghosting
    /// - **Use case**: Full Spectra 6 color updates
    ///
    /// ACeP (Advanced Color ePaper) full refresh mode.
    GCC16,

    /// GCU - Grayscale Color Update (Kaleido 3 fast update)
    ///
    /// - **Colors**: 4096 colors (4-bit RGB)
    /// - **Duration**: ~500ms
    /// - **Flashing**: Minimal flashing (4 flashes)
    /// - **Ghosting**: Moderate accumulation (~8% per refresh)
    /// - **Use case**: Fast Kaleido3 color updates
    ///
    /// Kaleido 3 optimized color refresh mode.
    GCU,
}

impl WaveformMode {
    /// Get the number of grayscale levels supported by this mode
    pub fn grayscale_levels(&self) -> u8 {
        match self {
            WaveformMode::GC16 | WaveformMode::GL16 => 16,
            WaveformMode::DU4 => 4,
            WaveformMode::DU | WaveformMode::A2 => 2,
            WaveformMode::GCC16 => 6, // 6 distinct colors
            WaveformMode::GCU => 16, // 4-bit per channel = 4096 colors, but report 16 for compatibility
        }
    }

    /// Get the bit depth for this mode
    pub fn bit_depth(&self) -> u8 {
        match self {
            WaveformMode::GC16 | WaveformMode::GL16 => 4,
            WaveformMode::DU4 => 2,
            WaveformMode::DU | WaveformMode::A2 => 1,
            WaveformMode::GCC16 => 4, // 6 colors fits in 4 bits
            WaveformMode::GCU => 12,  // 4-bit per RGB channel
        }
    }

    /// Get typical refresh duration in milliseconds
    pub fn base_duration_ms(&self) -> u32 {
        match self {
            WaveformMode::GC16 | WaveformMode::GL16 => 980,
            WaveformMode::DU4 | WaveformMode::DU => 260,
            WaveformMode::A2 => 200,
            WaveformMode::GCC16 => 15000, // 15 seconds for Spectra 6
            WaveformMode::GCU => 500,     // 500ms for Kaleido 3
        }
    }

    /// Get the number of flashes for this mode
    pub fn flash_count(&self) -> u8 {
        match self {
            WaveformMode::GC16 => 4,
            WaveformMode::GL16 => 2,
            WaveformMode::DU4 => 1,
            WaveformMode::DU => 0,
            WaveformMode::A2 => 1,
            WaveformMode::GCC16 => 30, // Many flashes for color particles
            WaveformMode::GCU => 4,    // Moderate flashing for Kaleido
        }
    }

    /// Get ghosting accumulation rate (0.0 - 1.0 per refresh)
    pub fn ghosting_rate(&self) -> f32 {
        match self {
            WaveformMode::GC16 | WaveformMode::GL16 => 0.0, // Clears ghosting
            WaveformMode::DU4 => 0.15,
            WaveformMode::DU => 0.20,
            WaveformMode::A2 => 0.25,
            WaveformMode::GCC16 => 0.0, // Clears ghosting (including color)
            WaveformMode::GCU => 0.08,  // Lower ghosting for Kaleido 3
        }
    }

    /// Check if this mode clears ghosting
    pub fn clears_ghosting(&self) -> bool {
        matches!(
            self,
            WaveformMode::GC16 | WaveformMode::GL16 | WaveformMode::GCC16
        )
    }

    /// Check if this mode supports full 16-level grayscale
    pub fn is_high_quality(&self) -> bool {
        matches!(self, WaveformMode::GC16 | WaveformMode::GL16)
    }

    /// Check if this is a fast update mode
    pub fn is_fast_mode(&self) -> bool {
        matches!(
            self,
            WaveformMode::DU | WaveformMode::DU4 | WaveformMode::A2 | WaveformMode::GCU
        )
    }

    /// Check if this mode supports color
    pub fn supports_color(&self) -> bool {
        matches!(self, WaveformMode::GCC16 | WaveformMode::GCU)
    }

    /// Get color refresh duration in milliseconds
    pub fn color_refresh_duration_ms(&self) -> u32 {
        match self {
            WaveformMode::GCC16 => 15000, // 15 seconds for Spectra 6
            WaveformMode::GCU => 500,     // 500ms for Kaleido 3
            _ => 0,                       // Not a color mode
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            WaveformMode::GC16 => "GC16 (16-level, high quality)",
            WaveformMode::GL16 => "GL16 (16-level, reduced flash)",
            WaveformMode::DU => "DU (1-bit, no flash)",
            WaveformMode::DU4 => "DU4 (4-level, fast)",
            WaveformMode::A2 => "A2 (1-bit, animation)",
            WaveformMode::GCC16 => "GCC16 (Spectra 6 color, 15s)",
            WaveformMode::GCU => "GCU (Kaleido 3 color, 500ms)",
        }
    }

    /// Quantize a 4-bit gray value (0-15) to this mode's supported levels
    ///
    /// This simulates the hardware limitation of different modes.
    pub fn quantize_gray4(&self, value: u8) -> u8 {
        let value = value.min(15); // Ensure 0-15 range

        match self {
            // 16-level modes: no quantization needed
            WaveformMode::GC16 | WaveformMode::GL16 => value,

            // 4-level mode: map 0-15 to 0,5,10,15
            WaveformMode::DU4 => {
                // Map to nearest value using proper thresholds
                // 0-3 → 0, 4-7 → 5, 8-12 → 10, 13-15 → 15
                match value {
                    0..=3 => 0,
                    4..=7 => 5,
                    8..=12 => 10,
                    _ => 15,
                }
            }

            // 2-level mode: map 0-15 to 0 or 15 (black or white)
            WaveformMode::DU | WaveformMode::A2 => {
                if value < 8 {
                    0
                } else {
                    15
                }
            }

            // Color modes: for grayscale, use same rules as GC16
            // (actual color quantization happens in color-specific code)
            WaveformMode::GCC16 | WaveformMode::GCU => value,
        }
    }
}

impl Default for WaveformMode {
    fn default() -> Self {
        WaveformMode::GC16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grayscale_levels() {
        assert_eq!(WaveformMode::GC16.grayscale_levels(), 16);
        assert_eq!(WaveformMode::GL16.grayscale_levels(), 16);
        assert_eq!(WaveformMode::DU4.grayscale_levels(), 4);
        assert_eq!(WaveformMode::DU.grayscale_levels(), 2);
        assert_eq!(WaveformMode::A2.grayscale_levels(), 2);
    }

    #[test]
    fn test_quantization_gc16() {
        let mode = WaveformMode::GC16;
        assert_eq!(mode.quantize_gray4(0), 0);
        assert_eq!(mode.quantize_gray4(7), 7);
        assert_eq!(mode.quantize_gray4(15), 15);
    }

    #[test]
    fn test_quantization_du4() {
        let mode = WaveformMode::DU4;
        // Should map to 0, 5, 10, 15
        assert_eq!(mode.quantize_gray4(0), 0); // 0 → 0
        assert_eq!(mode.quantize_gray4(3), 0); // 3 → 0
        assert_eq!(mode.quantize_gray4(5), 5); // 5 → 5
        assert_eq!(mode.quantize_gray4(8), 10); // 8 → 10
        assert_eq!(mode.quantize_gray4(13), 15); // 13 → 15
        assert_eq!(mode.quantize_gray4(15), 15); // 15 → 15
    }

    #[test]
    fn test_quantization_du() {
        let mode = WaveformMode::DU;
        // Should map to 0 or 15 only
        assert_eq!(mode.quantize_gray4(0), 0);
        assert_eq!(mode.quantize_gray4(5), 0);
        assert_eq!(mode.quantize_gray4(7), 0);
        assert_eq!(mode.quantize_gray4(8), 15);
        assert_eq!(mode.quantize_gray4(10), 15);
        assert_eq!(mode.quantize_gray4(15), 15);
    }

    #[test]
    fn test_ghosting_behavior() {
        assert!(WaveformMode::GC16.clears_ghosting());
        assert!(WaveformMode::GL16.clears_ghosting());
        assert!(!WaveformMode::DU4.clears_ghosting());
        assert!(!WaveformMode::DU.clears_ghosting());
        assert!(!WaveformMode::A2.clears_ghosting());

        assert_eq!(WaveformMode::GC16.ghosting_rate(), 0.0);
        assert!(WaveformMode::A2.ghosting_rate() > WaveformMode::DU4.ghosting_rate());
    }

    #[test]
    fn test_mode_characteristics() {
        assert!(WaveformMode::GC16.is_high_quality());
        assert!(!WaveformMode::DU.is_high_quality());

        assert!(WaveformMode::A2.is_fast_mode());
        assert!(!WaveformMode::GC16.is_fast_mode());

        assert!(WaveformMode::A2.base_duration_ms() < WaveformMode::GC16.base_duration_ms());
    }
}
