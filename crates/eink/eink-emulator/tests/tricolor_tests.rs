//! Tri-color display support tests
//!
//! Tests for Spectra 6 and Kaleido 3 color e-ink displays

use eink_emulator::{ColorMode, EinkColor, Framebuffer, PixelState, SpectraColor, WaveformMode};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_eink_color_gray4_conversion() {
    let color = EinkColor::from_gray4(Gray4::BLACK);
    assert!(color.is_grayscale());
    assert!(!color.is_color());

    let rgba = color.to_rgba();
    assert_eq!(rgba, 0xFF000000); // ARGB: A=255, R=0, G=0, B=0 (black)
}

#[test]
fn test_eink_color_spectra6_conversion() {
    let color = EinkColor::Spectra6 {
        bw: Gray4::new(2),
        color: SpectraColor::Red,
    };
    assert!(color.is_color());
    assert!(!color.is_grayscale());

    let rgba = color.to_rgba();
    // Red channel should be 255 (ARGB format: bits 16-23)
    let r = (rgba >> 16) & 0xFF;
    assert_eq!(r, 255);
}

#[test]
fn test_eink_color_kaleido3_conversion() {
    let color = EinkColor::Kaleido3 { r: 15, g: 8, b: 0 };
    assert!(color.is_color());

    let rgba = color.to_rgba();
    // ARGB format: A=24-31, R=16-23, G=8-15, B=0-7
    let r = (rgba >> 16) & 0xFF;
    let g = (rgba >> 8) & 0xFF;
    let b = (rgba >> 0) & 0xFF;

    assert_eq!(r, 255); // 15 * 17 = 255
    assert_eq!(g, 136); // 8 * 17 = 136
    assert_eq!(b, 0); // 0 * 17 = 0
}

#[test]
fn test_spectra6_rgba_output() {
    // Test all Spectra colors
    let test_cases = [
        (SpectraColor::None, "gray"),
        (SpectraColor::Red, "red"),
        (SpectraColor::Yellow, "yellow"),
        (SpectraColor::Blue, "blue"),
        (SpectraColor::Green, "green"),
    ];

    for (spectra_color, name) in test_cases {
        let color = EinkColor::Spectra6 {
            bw: Gray4::new(2), // Mid-gray base
            color: spectra_color,
        };
        let rgba = color.to_rgba();

        // Check that color channels are in expected ranges
        // ARGB format: A=24-31, R=16-23, G=8-15, B=0-7
        let r = (rgba >> 16) & 0xFF;
        let g = (rgba >> 8) & 0xFF;
        let b = (rgba >> 0) & 0xFF;

        match spectra_color {
            SpectraColor::Red => {
                assert_eq!(r, 255, "Red channel should be 255 for {} pigment", name);
            }
            SpectraColor::Blue => {
                assert_eq!(b, 255, "Blue channel should be 255 for {} pigment", name);
            }
            SpectraColor::Yellow => {
                assert_eq!(r, 255, "Red should be 255 for {}", name);
                assert_eq!(g, 255, "Green should be 255 for {}", name);
            }
            SpectraColor::Green => {
                assert_eq!(g, 255, "Green channel should be 255 for {} pigment", name);
            }
            SpectraColor::None => {
                assert_eq!(r, g, "R and G should match for {}", name);
                assert_eq!(g, b, "G and B should match for {}", name);
            }
        }
    }
}

#[test]
fn test_kaleido3_rgba_output() {
    // Test pure colors (ARGB format: 0xAARRGGBB)
    let red = EinkColor::Kaleido3 { r: 15, g: 0, b: 0 };
    assert_eq!(red.to_rgba(), 0xFFFF0000); // A=255, R=255, G=0, B=0

    let green = EinkColor::Kaleido3 { r: 0, g: 15, b: 0 };
    assert_eq!(green.to_rgba(), 0xFF00FF00); // A=255, R=0, G=255, B=0

    let blue = EinkColor::Kaleido3 { r: 0, g: 0, b: 15 };
    assert_eq!(blue.to_rgba(), 0xFF0000FF); // A=255, R=0, G=0, B=255

    let white = EinkColor::Kaleido3 {
        r: 15,
        g: 15,
        b: 15,
    };
    assert_eq!(white.to_rgba(), 0xFFFFFFFF); // A=255, R=255, G=255, B=255

    let black = EinkColor::Kaleido3 { r: 0, g: 0, b: 0 };
    assert_eq!(black.to_rgba(), 0xFF000000); // A=255, R=0, G=0, B=0
}

#[test]
fn test_color_quantization() {
    let gray = EinkColor::Gray(Gray4::new(2));
    let quantized = gray.quantize(2); // Binary

    match quantized {
        EinkColor::Gray(g) => {
            let luma = g.luma();
            assert!(
                luma == 0 || luma == 3,
                "Should quantize to black or white, got {}",
                luma
            );
        }
        _ => panic!("Should remain grayscale"),
    }
}

#[test]
fn test_framebuffer_color_mode_creation() {
    let fb_gray = Framebuffer::with_color_mode(10, 10, ColorMode::Grayscale);
    assert_eq!(fb_gray.color_mode, ColorMode::Grayscale);

    let fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);
    assert_eq!(fb_spectra.color_mode, ColorMode::Spectra6);

    let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
    assert_eq!(fb_kaleido.color_mode, ColorMode::Kaleido3);
}

#[test]
fn test_framebuffer_rgba_conversion_grayscale() {
    let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Grayscale);
    fb.set_pixel(0, 0, EinkColor::Gray(Gray4::new(0))); // Black
    fb.set_pixel(1, 1, EinkColor::Gray(Gray4::new(3))); // White

    let rgba = fb.to_rgba();
    assert_eq!(rgba.len(), 4);
    assert_eq!(rgba[0], 0xFF000000); // ARGB: Black (A=255, R=0, G=0, B=0)
    assert_eq!(rgba[3], 0xFFFFFFFF); // ARGB: White (A=255, R=255, G=255, B=255)
}

#[test]
fn test_framebuffer_rgba_conversion_spectra6() {
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
fn test_framebuffer_rgba_conversion_kaleido3() {
    let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Kaleido3);

    fb.set_pixel(0, 0, EinkColor::Kaleido3 { r: 15, g: 0, b: 0 });

    let rgba = fb.to_rgba();
    assert_eq!(rgba[0], 0xFFFF0000); // Pure red (ARGB: A=255, R=255, G=0, B=0)
}

#[test]
fn test_gray4_to_mode_conversion() {
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
        _ => panic!("Should be Spectra6 color"),
    }

    let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
    let converted = fb_kaleido.gray4_to_mode(Gray4::new(2));
    match converted {
        EinkColor::Kaleido3 { r, g, b } => {
            assert_eq!(r, 10); // 2 * 5 = 10
            assert_eq!(g, 10);
            assert_eq!(b, 10);
        }
        _ => panic!("Should be Kaleido3 color"),
    }
}

// ============================================================================
// COLOR PIXEL STATE TESTS
// ============================================================================

#[test]
fn test_color_pixel_state_creation() {
    let pixel = PixelState::new_with_color();
    assert!(pixel.color_state.is_some());

    let color_state = pixel.color_state.unwrap();
    assert_eq!(color_state.red_pigment, 0.0);
    assert_eq!(color_state.yellow_pigment, 0.0);
    assert_eq!(color_state.blue_pigment, 0.0);
    assert_eq!(color_state.color_ghosting, 0.0);
}

#[test]
fn test_color_ghosting_accumulates_faster() {
    let mut pixel_color = PixelState::new_with_color();
    let mut pixel_bw = PixelState::new();

    // Same refresh for both
    let ghosting_rate = 0.15;
    pixel_color.partial_refresh(15, ghosting_rate, 25);
    pixel_bw.partial_refresh(15, ghosting_rate, 25);

    // Color pixel should have additional color ghosting
    let color_state = pixel_color.color_state.unwrap();
    assert!(
        color_state.color_ghosting > 0.0,
        "Color ghosting should accumulate"
    );

    // Get the B&W ghosting (which includes direction_factor, etc.)
    let bw_ghosting = pixel_bw.ghosting;

    // Color ghosting should be approximately 2× the B&W content ghosting
    // The actual B&W ghosting includes: rate * transition * direction_factor * temp_factor
    // Color ghosting = content_ghosting * 2.0
    let expected_color_ghosting = bw_ghosting * 2.0;

    assert!(
        (color_state.color_ghosting - expected_color_ghosting).abs() < 0.01,
        "Color ghosting should be ~{}, got {}",
        expected_color_ghosting,
        color_state.color_ghosting
    );
}

#[test]
fn test_color_full_refresh_clears_ghosting() {
    let mut pixel = PixelState::new_with_color();

    // Accumulate color ghosting
    pixel.partial_refresh(15, 0.15, 25);
    pixel.partial_refresh(0, 0.15, 25);

    let color_state_before = pixel.color_state.unwrap();
    assert!(color_state_before.color_ghosting > 0.0);

    // Full refresh clears color ghosting
    pixel.full_refresh(8);

    let color_state_after = pixel.color_state.unwrap();
    assert_eq!(
        color_state_after.color_ghosting, 0.0,
        "Full refresh should clear color ghosting"
    );
}

#[test]
fn test_color_pigment_update() {
    let mut pixel = PixelState::new_with_color();
    let color_state = pixel.color_state.as_mut().unwrap();

    color_state.update_pigments(0.8, 0.5, 0.3);

    assert_eq!(color_state.red_pigment, 0.8);
    assert_eq!(color_state.yellow_pigment, 0.5);
    assert_eq!(color_state.blue_pigment, 0.3);
}

#[test]
fn test_color_pigment_clamping() {
    let mut pixel = PixelState::new_with_color();
    let color_state = pixel.color_state.as_mut().unwrap();

    // Test clamping to 0.0-1.0 range
    color_state.update_pigments(1.5, -0.3, 0.5);

    assert_eq!(color_state.red_pigment, 1.0, "Should clamp to 1.0");
    assert_eq!(color_state.yellow_pigment, 0.0, "Should clamp to 0.0");
    assert_eq!(color_state.blue_pigment, 0.5);
}

// ============================================================================
// COLOR WAVEFORM MODE TESTS
// ============================================================================

#[test]
fn test_waveform_mode_color_support() {
    assert!(!WaveformMode::GC16.supports_color());
    assert!(!WaveformMode::DU4.supports_color());
    assert!(WaveformMode::GCC16.supports_color());
    assert!(WaveformMode::GCU.supports_color());
}

#[test]
fn test_color_refresh_durations() {
    // Spectra 6 (GCC16) - 15 seconds
    assert_eq!(WaveformMode::GCC16.color_refresh_duration_ms(), 15000);
    assert_eq!(WaveformMode::GCC16.base_duration_ms(), 15000);

    // Kaleido 3 (GCU) - 500ms
    assert_eq!(WaveformMode::GCU.color_refresh_duration_ms(), 500);
    assert_eq!(WaveformMode::GCU.base_duration_ms(), 500);

    // Non-color modes return 0
    assert_eq!(WaveformMode::GC16.color_refresh_duration_ms(), 0);
    assert_eq!(WaveformMode::DU4.color_refresh_duration_ms(), 0);
}

#[test]
fn test_color_flash_counts() {
    // Spectra 6 needs many flashes for color particles
    assert_eq!(WaveformMode::GCC16.flash_count(), 30);

    // Kaleido 3 has moderate flashing
    assert_eq!(WaveformMode::GCU.flash_count(), 4);

    // Compare with B&W modes
    assert_eq!(WaveformMode::GC16.flash_count(), 4);
    assert_eq!(WaveformMode::DU4.flash_count(), 1);
}

#[test]
fn test_color_ghosting_rates() {
    // GCC16 clears ghosting (full refresh)
    assert_eq!(WaveformMode::GCC16.ghosting_rate(), 0.0);

    // GCU has lower ghosting than B&W partial modes
    assert_eq!(WaveformMode::GCU.ghosting_rate(), 0.08);
    assert!(WaveformMode::GCU.ghosting_rate() < WaveformMode::DU4.ghosting_rate());
}

#[test]
fn test_color_mode_characteristics() {
    // GCC16 should clear ghosting
    assert!(WaveformMode::GCC16.clears_ghosting());

    // GCU is a fast mode
    assert!(WaveformMode::GCU.is_fast_mode());

    // GCC16 is not a high-quality B&W mode (it's a color mode)
    assert!(!WaveformMode::GCC16.is_high_quality());
}

#[test]
fn test_spectra6_grayscale_levels() {
    // Spectra 6 reports 6 distinct colors
    assert_eq!(WaveformMode::GCC16.grayscale_levels(), 6);

    // Kaleido 3 reports 16 for compatibility (4-bit per channel)
    assert_eq!(WaveformMode::GCU.grayscale_levels(), 16);
}

#[test]
fn test_color_mode_bit_depth() {
    // Spectra 6: 4 bits for 6 colors
    assert_eq!(WaveformMode::GCC16.bit_depth(), 4);

    // Kaleido 3: 12 bits (4-bit per RGB channel)
    assert_eq!(WaveformMode::GCU.bit_depth(), 12);
}

// ============================================================================
// ADDITIONAL COLOR INTEGRATION TESTS
// ============================================================================

#[test]
fn test_spectra6_color_mixing() {
    // Test that Spectra6 colors render correctly with B&W base
    let colors_to_test = [
        (SpectraColor::Red, Gray4::BLACK, "Red on Black"),
        (SpectraColor::Yellow, Gray4::WHITE, "Yellow on White"),
        (SpectraColor::Blue, Gray4::new(2), "Blue on Mid-gray"),
        (SpectraColor::Green, Gray4::new(1), "Green on Light-gray"),
    ];

    for (color, bw, name) in colors_to_test {
        let eink_color = EinkColor::Spectra6 { bw, color };
        let rgba = eink_color.to_rgba();

        // Verify ARGB has alpha=255 (ARGB format: A=24-31)
        let a = (rgba >> 24) & 0xFF;
        assert_eq!(a, 255, "{} should have full alpha", name);

        // Verify color channels are reasonable (ARGB: R=16-23, G=8-15, B=0-7)
        let r = (rgba >> 16) & 0xFF;
        let g = (rgba >> 8) & 0xFF;
        let b = (rgba >> 0) & 0xFF;

        // Each color should have at least one channel at a reasonable level
        match color {
            SpectraColor::Red => assert!(r > 200, "{} red channel too low", name),
            SpectraColor::Yellow => {
                assert!(r > 200 && g > 200, "{} yellow channels too low", name)
            }
            SpectraColor::Blue => assert!(b > 200, "{} blue channel too low", name),
            SpectraColor::Green => {
                assert!(g > 200, "{} green channel too low", name)
            }
            SpectraColor::None => {
                assert_eq!(r, g, "{} grayscale R!=G", name);
                assert_eq!(g, b, "{} grayscale G!=B", name);
            }
        }
    }
}

#[test]
fn test_kaleido3_4bit_precision() {
    // Test that 4-bit precision is maintained
    let test_colors = [
        (0, 0, 0, "Black"),
        (15, 15, 15, "White"),
        (7, 7, 7, "Mid-gray"),
        (15, 0, 0, "Pure red"),
        (0, 15, 0, "Pure green"),
        (0, 0, 15, "Pure blue"),
        (8, 4, 2, "Mixed color"),
    ];

    for (r, g, b, name) in test_colors {
        let color = EinkColor::Kaleido3 { r, g, b };
        let rgba = color.to_rgba();

        // Extract channels (ARGB format: A=24-31, R=16-23, G=8-15, B=0-7)
        let r8 = (rgba >> 16) & 0xFF;
        let g8 = (rgba >> 8) & 0xFF;
        let b8 = (rgba >> 0) & 0xFF;

        // Verify 4-bit to 8-bit conversion (multiply by 17)
        assert_eq!(r8, (r * 17) as u32, "{} red conversion failed", name);
        assert_eq!(g8, (g * 17) as u32, "{} green conversion failed", name);
        assert_eq!(b8, (b * 17) as u32, "{} blue conversion failed", name);
    }
}

#[test]
fn test_color_mode_default_pixels() {
    // Test that default pixels are white for each color mode
    let fb_gray = Framebuffer::with_color_mode(10, 10, ColorMode::Grayscale);
    let first_pixel = fb_gray.get_pixel(0, 0).unwrap();
    assert_eq!(first_pixel, EinkColor::Gray(Gray4::WHITE));

    let fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);
    let first_pixel = fb_spectra.get_pixel(0, 0).unwrap();
    match first_pixel {
        EinkColor::Spectra6 { bw, color } => {
            assert_eq!(bw, Gray4::WHITE);
            assert_eq!(color, SpectraColor::None);
        }
        _ => panic!("Should be Spectra6"),
    }

    let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
    let first_pixel = fb_kaleido.get_pixel(0, 0).unwrap();
    match first_pixel {
        EinkColor::Kaleido3 { r, g, b } => {
            assert_eq!(r, 15);
            assert_eq!(g, 15);
            assert_eq!(b, 15);
        }
        _ => panic!("Should be Kaleido3"),
    }
}

#[test]
fn test_color_mode_clear() {
    // Test that clear() sets all pixels to white in each mode
    let mut fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);

    // Set a pixel to black
    fb_spectra.set_pixel(
        5,
        5,
        EinkColor::Spectra6 {
            bw: Gray4::BLACK,
            color: SpectraColor::Red,
        },
    );

    // Clear
    fb_spectra.clear();

    // All pixels should be white
    for y in 0..10 {
        for x in 0..10 {
            let pixel = fb_spectra.get_pixel(x, y).unwrap();
            match pixel {
                EinkColor::Spectra6 { bw, color } => {
                    assert_eq!(bw, Gray4::WHITE);
                    assert_eq!(color, SpectraColor::None);
                }
                _ => panic!("Should be Spectra6"),
            }
        }
    }
}

#[test]
fn test_color_refresh_is_slower() {
    // Verify that color refresh modes are significantly slower than B&W
    let gcc16_duration = WaveformMode::GCC16.base_duration_ms();
    let gcu_duration = WaveformMode::GCU.base_duration_ms();
    let gc16_duration = WaveformMode::GC16.base_duration_ms();
    let du4_duration = WaveformMode::DU4.base_duration_ms();

    // Spectra 6 (GCC16) should be much slower than any B&W mode
    assert!(
        gcc16_duration > gc16_duration * 5,
        "GCC16 ({}) should be >5× slower than GC16 ({})",
        gcc16_duration,
        gc16_duration
    );

    // Kaleido 3 (GCU) should be faster than Spectra 6 but slower than fast B&W modes
    assert!(
        gcu_duration > du4_duration,
        "GCU ({}) should be slower than DU4 ({})",
        gcu_duration,
        du4_duration
    );
    assert!(
        gcu_duration < gcc16_duration,
        "GCU ({}) should be faster than GCC16 ({})",
        gcu_duration,
        gcc16_duration
    );
}

#[test]
fn test_color_pixel_state_independence() {
    // Test that color state is independent from B&W state
    let mut pixel = PixelState::new_with_color();

    // Update B&W state
    pixel.partial_refresh(15, 0.15, 25);

    // Update color state
    if let Some(ref mut color) = pixel.color_state {
        color.update_pigments(0.8, 0.5, 0.3);
    }

    // B&W and color should be independent
    assert_eq!(pixel.current, 15);
    assert!(pixel.ghosting > 0.0);

    let color_state = pixel.color_state.unwrap();
    assert_eq!(color_state.red_pigment, 0.8);
    assert_eq!(color_state.yellow_pigment, 0.5);
    assert_eq!(color_state.blue_pigment, 0.3);
}

#[test]
fn test_spectra6_all_colors_unique() {
    // Verify that all Spectra6 colors produce unique RGBA values
    let colors = [
        SpectraColor::None,
        SpectraColor::Red,
        SpectraColor::Yellow,
        SpectraColor::Blue,
        SpectraColor::Green,
    ];

    let mut rgba_values = std::collections::HashSet::new();

    for color in colors {
        let eink_color = EinkColor::Spectra6 {
            bw: Gray4::new(2), // Same B&W base for all
            color,
        };
        let rgba = eink_color.to_rgba();
        rgba_values.insert(rgba);
    }

    // All 5 colors should produce unique RGBA values
    assert_eq!(
        rgba_values.len(),
        5,
        "All Spectra6 colors should be visually distinct"
    );
}
